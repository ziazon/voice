"""Mine a local Slack JSON export into samples and candidate decision episodes.

Usage: python3 -I scripts/slack_mine.py --config CFG <export dir> <work dir>

Expected input under <export dir>:
  users.json                       JSON array of users with id and optional names
  conversations.json               JSON array with id, name and kind (channel/im/mpim)
  messages/<conversation id>.jsonl  one JSON object per line with user, text, ts,
                                   and optional thread_ts
Timestamps are Slack timestamp strings; every message file needs a conversation entry.

The config may supply filters.personal and filters.secret as case-insensitive regex
strings. Matching messages are dropped before analysis or output; stats count drops
per configured person. Without filters, a built-in secret pattern is used.

Writes (0600) under <work dir>, which must be outside any git work tree:
  stats.json                      per-person counts by year, kind, conversation, length, hour
  samples/<person>/<batch>.txt    stratified samples by quarter, each message with its context
  episodes/<year>/<batch>.txt     candidate decision/disagreement episodes, scored, as transcripts
  episodes/index.json             every candidate episode's id, link, participants, score
Raw text never leaves the work dir.
"""
from __future__ import annotations

import argparse
import collections
import datetime as dt
import json
import os
import random
import re
import sys
import uuid
from pathlib import Path
from urllib.parse import urlsplit

DISAGREE = re.compile(
    r"\b(disagree|don'?t agree|doesn'?t make sense|makes? (zero|no) sense|not at all|i don'?t think|"
    r"push(ing)? back|concern(ed|s)?|that'?s not|why would|why are we|why not|i'?d rather|"
    r"nope|but (why|that|we|i|the)|instead of|problem with|not the page|not on the same page|"
    r"frustrat|annoy|wrong|bad idea|not a good idea|shouldn'?t|should not|can'?t agree|objection)\b|\bno,",
    re.I,
)
DECIDE = re.compile(
    r"\b(decided|decision|let'?s go with|we'?ll go with|agreed|we agree|going forward|from now on|"
    r"final(ly)?|settled|the plan is|we'?re going to|approved|sign(ed)? off|ruling|consensus|"
    r"we should|we will|we won'?t|moving forward|go ahead)\b",
    re.I,
)


DEFAULT_SECRET = (
    r"\bsk-[A-Za-z0-9]{20,}|\bsk_(live|test)_[A-Za-z0-9]{16,}|\bgh[pousr]_[A-Za-z0-9]{20,}|"
    r"\bxox[abpr]-|\bxapp-|AKIA[0-9A-Z]{16}|\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}|"
    r"\bAIza[0-9A-Za-z_-]{30,}|\bhf_[A-Za-z0-9]{30,}|\bglpat-[A-Za-z0-9_-]{20,}|"
    r"\bnpm_[A-Za-z0-9]{30,}|-----BEGIN|passw(or)?d|\b\d{9,}\b"
)


def load_config(path: Path):
    """Validate both tools' shared config before reading or writing history."""
    config = json.loads(path.read_text())
    defaults = {"focus": [], "seed": 20261022, "sample_per_quarter": 45,
                "batch_chars": 60000, "gap_minutes": 45, "select": {}, "filters": {"secret": DEFAULT_SECRET}}
    required = {"workspace_url", "principal", "people"}
    if not isinstance(config, dict):
        raise ValueError("config must be an object")
    if config.keys() - required - defaults.keys():
        raise ValueError("unknown config keys")
    missing = required - config.keys()
    if missing:
        raise ValueError("missing config keys: " + ", ".join(sorted(missing)))
    config = defaults | config
    url = config["workspace_url"]
    if not isinstance(url, str) or urlsplit(url).scheme not in {"http", "https"} or not urlsplit(url).netloc:
        raise ValueError("workspace_url must be an HTTP URL")
    if not isinstance(config["principal"], str) or not config["principal"]:
        raise ValueError("principal must be a nonempty id")
    people = config["people"]
    if not isinstance(people, dict) or not people or any(
        not isinstance(uid, str) or not uid or not isinstance(label, str)
        or not re.fullmatch(r"[A-Za-z0-9_-]+", label) for uid, label in people.items()
    ) or len(set(people.values())) != len(people):
        raise ValueError("people must map ids to unique folder labels (letters, digits, _ or -)")
    if not isinstance(config["focus"], list) or any(not isinstance(uid, str) for uid in config["focus"]):
        raise ValueError("focus must be a list of ids")
    for key in ("seed", "sample_per_quarter", "batch_chars", "gap_minutes"):
        if type(config[key]) is not int or (key != "seed" and config[key] <= 0):
            raise ValueError(key + " must be an integer" + (" greater than zero" if key != "seed" else ""))
    filters = config["filters"]
    if not isinstance(filters, dict) or filters.keys() - {"personal", "secret"}:
        raise ValueError("filters must be an object with personal or secret regex strings")
    for pattern in filters.values():
        if not isinstance(pattern, str):
            raise ValueError("filters must be regex strings")
        try:
            re.compile(pattern, re.I)
        except re.error:
            raise ValueError("filters must be valid regex strings") from None
    rule = config["select"]
    if not isinstance(rule, dict) or rule.keys() - {"min_score", "pairs"}:
        raise ValueError("select has unknown keys or is not an object")
    rule = {"min_score": 10, "pairs": []} | rule

    def valid_score(score):
        return type(score) in (int, float) and float('-inf') < score < float('inf')

    if not valid_score(rule["min_score"]) or not isinstance(rule["pairs"], list):
        raise ValueError("select requires a numeric min_score and a pairs list")
    for pair in rule["pairs"]:
        if (not isinstance(pair, dict) or set(pair) != {"ids", "min_score"}
                or not isinstance(pair["ids"], list) or not pair["ids"]
                or any(not isinstance(uid, str) or not uid for uid in pair["ids"])
                or not valid_score(pair["min_score"])):
            raise ValueError("select pairs require ids and a numeric min_score")
    config["select"] = rule
    return config


def validate_work(work: Path) -> None:
    resolved = work.resolve()
    if any((parent / ".git").exists() or (parent / ".git").is_symlink()
           for parent in (resolved, *resolved.parents)):
        raise ValueError("work directory must not be inside a git work tree")


def private_write(work: Path, path: Path, text: str) -> None:
    """Keep raw output private and reject paths that escape the work directory."""
    validate_work(work)
    if not path.resolve().is_relative_to(work.resolve()):
        raise ValueError("output path escapes work directory")
    work.mkdir(parents=True, exist_ok=True, mode=0o700)
    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    parent_fd = os.open(work, directory_flags)
    temporary = None
    try:
        os.fchmod(parent_fd, 0o700)
        for part in path.relative_to(work).parts[:-1]:
            try:
                os.mkdir(part, 0o700, dir_fd=parent_fd)
            except FileExistsError:
                pass
            child_fd = os.open(part, directory_flags, dir_fd=parent_fd)
            os.close(parent_fd)
            parent_fd = child_fd
            os.fchmod(parent_fd, 0o700)
        temporary_name = "." + path.name + "." + uuid.uuid4().hex + ".tmp"
        fd = os.open(temporary_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                     0o600, dir_fd=parent_fd)
        temporary = temporary_name
        with os.fdopen(fd, "w") as output:
            os.fchmod(output.fileno(), 0o600)
            output.write(text)
        os.replace(temporary, path.name, src_dir_fd=parent_fd, dst_dir_fd=parent_fd)
        temporary = None
    finally:
        if temporary is not None:
            try:
                os.unlink(temporary, dir_fd=parent_fd)
            except FileNotFoundError:
                pass
        os.close(parent_fd)


def when(ts: str) -> dt.datetime:
    return dt.datetime.fromtimestamp(float(ts), dt.timezone.utc)


def link(config, cid: str, ts: str, thread_ts: str | None) -> str:
    url = f"{config['workspace_url'].rstrip('/')}/archives/{cid}/p{ts.replace('.', '')}"
    if thread_ts and thread_ts != ts:
        url += f"?thread_ts={thread_ts}&cid={cid}"
    return url


def load(export: Path):
    users = {u["id"]: u for u in json.loads((export / "users.json").read_text())}
    convs = {c["id"]: c for c in json.loads((export / "conversations.json").read_text())}
    messages = {}
    for path in sorted((export / "messages").glob("*.jsonl")):
        if path.stem not in convs:
            raise ValueError(f"{path.stem} missing from conversations.json")
        # split on \n only: str.splitlines() also breaks on U+2028 and friends inside messages
        rows = [json.loads(line) for line in path.read_text().split("\n") if line.strip()]
        messages[path.stem] = [m for m in rows if m.get("user") and m.get("text")]
    return users, convs, messages


def flatten(text):
    return text.replace("\r\n", "\n").replace("\r", "\n").replace("\n", " ⏎ ")


def name_of(config, users, uid):
    if uid in config["people"]:
        return config["people"][uid].capitalize()
    u = users.get(uid) or {}
    return flatten(u.get("real_name") or u.get("display_name") or u.get("name") or uid)


def conv_label(config, users, conv):
    if conv["kind"] == "im":
        return f"DM with {name_of(config, users, conv['name'])}"
    if conv["kind"] == "mpim":
        return "group DM"
    return "#" + flatten(conv["name"])


MENTION = re.compile(r"<@([A-Z0-9]+)>")


def clean(config, users, text):
    return flatten(MENTION.sub(lambda m: "@" + name_of(config, users, m.group(1)), text))


def episodes_of(config, msgs):
    """Threads are episodes; unthreaded messages split on the configured pause."""
    threads = collections.defaultdict(list)
    loose = []
    for m in msgs:
        root = m.get("thread_ts")
        if root:
            threads[root].append(m)
        elif m.get("thread"):
            threads[m["ts"]].append(m)
        else:
            loose.append(m)
    for root, rows in threads.items():
        yield root, sorted(rows, key=lambda m: float(m["ts"]))
    current = []
    for m in loose:
        if current and float(m["ts"]) - float(current[-1]["ts"]) > config["gap_minutes"] * 60:
            yield None, current
            current = []
        current.append(m)
    if current:
        yield None, current


def transcript(config, users, rows):
    return "\n".join(
        f"[{when(m['ts']).strftime('%Y-%m-%d %H:%M')}] {name_of(config, users, m['user'])}: {clean(config, users, m['text'])[:1200]}"
        for m in rows
    )


def main(config, export: Path, work: Path) -> None:
    validate_work(work)
    users, convs, messages = load(export)
    for uid in config["people"]:
        if uid not in users:
            raise ValueError(f"{uid} missing from users.json")
    dropped = collections.Counter()
    patterns = [re.compile(pattern, re.I) for pattern in config["filters"].values()]
    for cid, msgs in messages.items():
        kept = []
        for message in msgs:
            if any(pattern.search(message["text"]) for pattern in patterns):
                dropped[message["user"]] += 1
            else:
                kept.append(message)
        messages[cid] = kept
    rng = random.Random(config["seed"])

    # 1. stats
    stats = {p: {"messages": 0, "by_year": collections.Counter(), "by_kind": collections.Counter(),
                 "by_conv": collections.Counter(), "lengths": [], "questions": 0,
                 "hours_utc": collections.Counter()} for p in config["people"].values()}
    for cid, msgs in messages.items():
        conv = convs[cid]
        for m in msgs:
            p = config["people"].get(m["user"])
            if not p:
                continue
            s = stats[p]
            t = when(m["ts"])
            s["messages"] += 1
            s["by_year"][t.year] += 1
            s["by_kind"][conv["kind"]] += 1
            s["by_conv"][conv_label(config, users, conv)] += 1
            s["lengths"].append(len(m["text"]))
            s["questions"] += "?" in m["text"]
            s["hours_utc"][t.hour] += 1
    out = {}
    for p, s in stats.items():
        lengths = sorted(s["lengths"]) or [0]
        out[p] = {
            "messages": s["messages"],
            "dropped": sum(dropped[uid] for uid, label in config["people"].items() if label == p),
            "by_year": dict(sorted(s["by_year"].items())),
            "by_kind": dict(s["by_kind"]),
            "top_conversations": s["by_conv"].most_common(25),
            "median_chars": lengths[len(lengths) // 2],
            "question_share": round(s["questions"] / max(1, s["messages"]), 3),
            "hours_utc": dict(sorted(s["hours_utc"].items())),
        }
    private_write(work, work / "stats.json", json.dumps(out, indent=1))

    # 1b. everyone else: persona candidates, ranked by volume and by exchanges with configured people
    people = collections.defaultdict(lambda: {"messages": 0, "years": set(), "convs": set(),
                                              "with_principal": 0, "with_focus": 0})
    for cid, msgs in messages.items():
        for root, rows in episodes_of(config, msgs):
            humans = {m["user"] for m in rows if not m.get("bot_id")}
            for m in rows:
                if m.get("bot_id") or m["user"] in config["people"]:
                    continue
                p = people[m["user"]]
                p["messages"] += 1
                p["years"].add(when(m["ts"]).year)
                p["convs"].add(conv_label(config, users, convs[cid]))
                p["with_principal"] += config["principal"] in humans
                p["with_focus"] += bool(humans & set(config["people"]))
    ranked = sorted(
        ({"id": uid, "name": name_of(config, users, uid), "deleted": (users.get(uid) or {}).get("deleted"),
          "is_bot": (users.get(uid) or {}).get("is_bot"), "messages": p["messages"],
          "with_principal": p["with_principal"], "with_focus": p["with_focus"],
          "years": f"{min(p['years'])}-{max(p['years'])}",
          "top_conversations": sorted(p["convs"])[:12]}
         for uid, p in people.items() if not (users.get(uid) or {}).get("is_bot")),
        key=lambda r: -r["with_focus"])
    private_write(work, work / "people.json", json.dumps(ranked[:60], indent=1))

    # 2. stratified samples with context
    by_person_quarter = collections.defaultdict(list)
    for cid, msgs in messages.items():
        for i, m in enumerate(msgs):
            p = config["people"].get(m["user"])
            if p and len(m["text"]) >= 25:
                t = when(m["ts"])
                by_person_quarter[(p, f"{t.year}Q{(t.month - 1) // 3 + 1}")].append((cid, i))
    for p in config["people"].values():
        chunks, size, batch = [], 0, 0
        quarters = sorted(q for (pp, q) in by_person_quarter if pp == p)
        for q in quarters:
            pool = by_person_quarter[(p, q)]
            for cid, i in rng.sample(pool, min(config["sample_per_quarter"], len(pool))):
                msgs = messages[cid]
                m = msgs[i]
                same = [x for x in msgs[max(0, i - 6):i] if x.get("thread_ts") == m.get("thread_ts")][-3:]
                block = (f"### {q} · {conv_label(config, users, convs[cid])} · {link(config, cid, m['ts'], m.get('thread_ts'))}\n"
                         + (transcript(config, users, same) + "\n" if same else "")
                         + ">>> " + transcript(config, users, [m]) + "\n")
                chunks.append(block)
                size += len(block)
                if size > config["batch_chars"]:
                    private_write(work, work / "samples" / p / f"{batch:03d}.txt", "\n".join(chunks))
                    chunks, size, batch = [], 0, batch + 1
        if chunks:
            private_write(work, work / "samples" / p / f"{batch:03d}.txt", "\n".join(chunks))

    # 3. candidate episodes
    focus = set(config["people"]) | set(config["focus"])
    candidates = []
    for cid, msgs in messages.items():
        conv = convs[cid]
        for root, rows in episodes_of(config, msgs):
            humans = {m["user"] for m in rows if not m.get("bot_id")}
            core = humans & focus
            principal_debate = config["principal"] in humans and len(humans) >= 2 and len(rows) >= 6
            if not ((len(core) >= 2 and len(rows) >= 4) or principal_debate):
                continue
            text = " ".join(m["text"] for m in rows)
            dis = len(DISAGREE.findall(text))
            dec = len(DECIDE.findall(text))
            chars = len(text)
            score = dis * 2 + dec + min(chars / 1500, 6) + (2 if len(core) >= 3 else 0)
            if dis + dec == 0:
                continue
            first = rows[0]
            candidates.append({
                "id": f"{cid}:{first['ts']}",
                "link": link(config, cid, first["ts"], root if root and root != first["ts"] else None),
                "where": conv_label(config, users, conv),
                "date": when(first["ts"]).strftime("%Y-%m-%d"),
                "year": when(first["ts"]).year,
                "people": sorted(name_of(config, users, u) for u in humans),
                "ids": sorted(humans),
                "messages": len(rows),
                "disagree": dis, "decide": dec, "score": round(score, 2),
                "_rows": rows,
            })
    candidates.sort(key=lambda c: -c["score"])
    index = [{k: v for k, v in c.items() if k != "_rows"} for c in candidates]
    private_write(work, work / "episodes" / "index.json", json.dumps(index, indent=1))
    by_year = collections.defaultdict(list)
    for c in candidates:
        by_year[c["year"]].append(c)
    for year, cs in sorted(by_year.items()):
        chunks, size, batch = [], 0, 0
        for c in cs:
            block = (f"=== EPISODE {c['id']} · {c['date']} · {c['where']} · {c['link']} · "
                     f"people: {', '.join(c['people'])} · score {c['score']}\n"
                     + transcript(config, users, c["_rows"][:60]) + "\n")
            chunks.append(block)
            size += len(block)
            if size > config["batch_chars"]:
                private_write(work, work / "episodes" / str(year) / f"{batch:03d}.txt", "\n".join(chunks))
                chunks, size, batch = [], 0, batch + 1
        if chunks:
            private_write(work, work / "episodes" / str(year) / f"{batch:03d}.txt", "\n".join(chunks))
    print(json.dumps({p: s["messages"] for p, s in out.items()}),
          "candidates", len(candidates),
          json.dumps({y: len(cs) for y, cs in sorted(by_year.items())}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", required=True, type=Path)
    parser.add_argument("export", type=Path)
    parser.add_argument("work", type=Path)
    args = parser.parse_args()
    try:
        main(load_config(args.config.expanduser()), args.export.expanduser(), args.work.expanduser())
    except (ValueError, OSError) as error:
        print("error: " + " ".join(str(error).splitlines()), file=sys.stderr)
        sys.exit(2)
