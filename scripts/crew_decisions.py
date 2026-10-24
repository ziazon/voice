#!/usr/bin/env python3
"""Mine, split, tally and score a person's decision history: the corpus behind their crew assistant.

A crew assistant answers a decision the way its principal would. This script is the deterministic
half: no LLM, no network. It reads the principal's Claude Code transcripts, which hold their
verbatim words, so everything it writes belongs in the member's state dir (`state_dir` in
member.json), never in git.

Every command reads a member config: `crew_decisions.py --member crew/<name>/member.json <cmd>`
(or $CREW_MEMBER); a gitignored private.json beside it adds filters or replaces keys. It names the
member and the principal, the state dir, the gate mark word (`(Recommended · <Name>)`), the
principal's pronouns, and the privacy filters (excluded folders, personal topics, secrets, pasted prompts).

  mine  --projects DIR --out decisions.jsonl [--holdout 0.15 | --cutoff TS]
      Every answered AskUserQuestion question (options, the recommended label and the member's
      pick, the principal's picks, own words and notes) and every message they typed, with the end
      of the reply it answered. Drops excluded folders, and records on a personal topic or holding a
      secret. The newest share of questions, and every message from that point on, is split
      "heldout" for the backtest.
  documents --dir FOLDER --out-dir DIR [--size 6000] [--pdftotext PATH]
      Read the principal's documents into filtered private lens inputs and an inventory.
  round --name NAME --questions Q.json --answers A.json [--heldout] [--out PATH]
      Import an off-Claude question round, training by default or held out as a whole round.
  merge [--in FILE...] [--train FILE...] [--rounds FILE...] --out decisions.jsonl
      Merge history and training inputs; question rounds keep their imported split and never
      move the history cutoff.
  tally --corpus decisions.jsonl --labels labels.json [--json]
      Outcomes per decision category over the training split. Every training record must
      carry one label; a held-out record may carry a category but never a reason.
  split --corpus decisions.jsonl --questions q.jsonl --answers a.jsonl
      The held-out questions without their answers (what a blind predictor sees), and the
      answer key.
  score --answers a.jsonl --labels labels.json --predictions p.json [--json]
      Accuracy per confidence tier and per category, against the baseline of always taking
      the recommended option. Every held-out question needs a prediction.
  similar [--corpus decisions.jsonl] --query TEXT [--k 6] [--json]
      Past answers closest to a new question (shared rare words), training split only, so
      a backtest never sees the answers it is scored on.
  record [--log live.jsonl] --work PATH --kind decided|escalated --category C --tier high|medium|low
         --question Q --<member>-pick P [--rule R] [--<principal>-answer A]
      Appends one live decision: what the member decided alone, or escalated with their lean.
  pending [--projects DIR] [--queues DIR] [--seen FILE] [--since-hours 24] [--exclude ID]
      One JSON line per question waiting on the principal that it has not printed before: an
      unanswered AskUserQuestion, a closing wrap_up ask nothing has replied to, an open CONTROL-Q
      (only when the member config names `queues`), or an answer to an escalation, written by the
      config's `queue_answerer`, for relay to its asking session. Same privacy filters as mine.
      A member's watcher runs pending with no model.
"""
import argparse
import hashlib
import json
import math
import os
import re
import sys
import subprocess
import uuid
import zipfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import crew_documents  # noqa: E402

NOISE = {"try again", "continue", "retry"}
REMINDER = re.compile(r"<system-reminder>.*?</system-reminder>", re.S)
CONTEXT_CHARS = 300
MAX_MESSAGE_CHARS = 4000
PART_WORDS = 500
QUEUE_TAIL_BYTES = 512 * 1024
TIERS = ("high", "medium", "low")
REQUIRED = ("name", "slug", "principal", "principal_slug", "state_dir", "mark_word", "filters")
REQUIRED_FILTERS = ("excluded_folders", "personal", "secret", "pasted_prefixes")


class Failed(Exception):
    pass


@dataclass(frozen=True)
class Member:
    """One crew member's config, compiled once: the filters, the gate mark and the derived names."""
    name: str
    slug: str
    principal: str
    principal_slug: str
    state_dir: Path
    mark_word: str
    excluded: re.Pattern
    personal: re.Pattern
    secret: re.Pattern
    pasted: tuple
    mark: re.Pattern
    queues: str
    queue_answerer: str
    codebook: dict
    principal_subject: str

    @property
    def pick_field(self):
        return f"{self.slug}_pick"

    @property
    def answer_field(self):
        return f"{self.principal_slug}_answer"

    def state(self, name):
        return str(self.state_dir / name)


def compile_pattern(key, pattern):
    try:
        return re.compile(pattern, re.I)
    except (re.error, TypeError) as exc:
        raise Failed(f"member config: {key} is not a valid regex ({exc})")


def validate_config(config, source):
    if not isinstance(config, dict):
        raise Failed(f"member config: {source} must be a JSON object")
    filters = config.get("filters", {})
    if not isinstance(filters, dict):
        raise Failed(f"member config: {source}: filters must be a JSON object")
    if "pasted_prefixes" in filters:
        prefixes = filters["pasted_prefixes"]
        if not isinstance(prefixes, list) or not all(isinstance(v, str) for v in prefixes):
            raise Failed(f"member config: {source}: filters.pasted_prefixes must be a list of strings")
    if "queue_answerer" in config and not (isinstance(config["queue_answerer"], str) and config["queue_answerer"]):
        raise Failed(f"member config: {source}: queue_answerer must be a non-empty string")


def load_member(config):
    validate_config(config, "member.json")
    missing = [k for k in REQUIRED if k not in config]
    missing += [f"filters.{k}" for k in REQUIRED_FILTERS if k not in (config.get("filters") or {})]
    if missing:
        raise Failed(f"member config: missing {', '.join(missing)}")
    f = config["filters"]
    word = re.escape(config["mark_word"])
    # Gate labels use (Recommended), (Recommended · <Name>), or (<Name>).
    mark = re.compile(rf"\s*\(\s*(recommended|{word})(?:\s*[·•/+,&-]\s*(recommended|{word}))?\s*\)", re.I)
    return Member(name=config["name"], slug=config["slug"], principal=config["principal"],
                  principal_slug=config["principal_slug"], state_dir=Path(config["state_dir"]).expanduser(),
                  mark_word=config["mark_word"],
                  excluded=compile_pattern("filters.excluded_folders", f["excluded_folders"]),
                  personal=compile_pattern("filters.personal", f["personal"]),
                  secret=compile_pattern("filters.secret", f["secret"]),
                  pasted=tuple(f["pasted_prefixes"]), mark=mark, queues=config.get("queues"),
                  queue_answerer=config.get("queue_answerer"),
                  codebook=config.get("codebook") or {},
                  principal_subject=config.get("principal_pronouns", "they/them").split("/")[0])


def read_json(path):
    try:
        return json.loads(path.read_text())
    except ValueError as exc:
        raise Failed(f"member config: {path} is not JSON ({exc})")


def read_member(path):
    """member.json, with the gitignored private.json beside it laid over: its filters extend the
    member's (a record is dropped if either pattern matches), and any other key replaces the member's."""
    path = Path(path).expanduser()
    config = read_json(path)
    validate_config(config, path)
    private = path.with_name("private.json")
    if not private.is_file():
        return config
    extra = read_json(private)
    validate_config(extra, private)
    merged = {**config, **{k: v for k, v in extra.items() if k != "filters"}}
    filters = dict(config.get("filters") or {})
    for key, value in (extra.get("filters") or {}).items():
        if key != "pasted_prefixes" and not isinstance(value, str):
            raise Failed(f"member config: private.json: filters.{key} must be a string (a regex)")
        if key == "pasted_prefixes":
            filters[key] = [*filters.get(key, []), *value]
        elif filters.get(key):
            filters[key] = f"(?:{filters[key]})|(?:{value})"
        else:
            filters[key] = value
    merged["filters"] = filters
    return merged


def project_name(folder):
    name = re.sub(r"--claude-worktrees-.*", "", folder)
    # Claude Code names a project folder by its path with every non-alphanumeric character as "-".
    prefix = re.sub(r"[^A-Za-z0-9]", "-", str(Path.home())) + "-"
    if name.startswith(prefix):
        name = name[len(prefix):]
    else:
        # A folder mined on another Mac: drop that home too, as far as its encoding allows.
        name = re.sub(r"^-(Users|home)-[A-Za-z0-9]+-", "", name)
    name = re.sub(r"^projects-", "", name)
    return "." + name[1:] if name.startswith("-") else name


def record_id(kind, session, when, text):
    digest = hashlib.sha1(f"{session}\n{when}\n{text}".encode()).hexdigest()[:12]
    return f"{kind}:{digest}"


def marks(m, label):
    return {word.lower() for match in m.mark.findall(label) for word in match if word}


def recommended_label(m, labels):
    return next((label for label in labels if "recommended" in marks(m, label)), None)


def member_label(m, labels):
    return next((label for label in labels if m.mark_word.lower() in marks(m, label)), None)


def parse_answer(answer, labels, multi):
    """Their picks among the labels, and whatever they wrote in their own words."""
    if not multi:
        return ([answer], "") if answer in labels else ([], answer.strip())
    picks, rest = [], answer
    for label in sorted(labels, key=len, reverse=True):
        if label in rest:
            picks.append(label)
            rest = rest.replace(label, "", 1)
    return picks, re.sub(r"\s*,(\s*,)+\s*", ", ", rest).strip(" ,")


def outcome(picks, own_words, recommended):
    if own_words:
        return "own_words"
    if recommended is None:
        return "no_rec"
    return "took_rec" if recommended in picks else "other_option"


def questions_in(m, record, session, project):
    result = record.get("toolUseResult")
    if not (isinstance(result, dict) and "questions" in result and "answers" in result):
        return []
    out = []
    for q in result["questions"]:
        answer = result["answers"].get(q.get("question"))
        if answer is None:
            continue
        options = [{"label": o.get("label", ""), "description": o.get("description", "")}
                   for o in q.get("options", [])]
        labels = [o["label"] for o in options]
        multi = bool(q.get("multiSelect"))
        picks, own_words = parse_answer(str(answer), labels, multi)
        recommended = recommended_label(m, labels)
        notes = ((result.get("annotations") or {}).get(q["question"]) or {}).get("notes") or ""
        out.append({"id": record_id("question", session, record["timestamp"], q["question"]),
                    "kind": "question", "timestamp": record["timestamp"], "project": project,
                    "header": q.get("header", ""), "question": q["question"], "multiSelect": multi,
                    "options": options, "recommended": recommended, m.pick_field: member_label(m, labels),
                    "picks": picks, "own_words": own_words, "notes": notes,
                    "outcome": outcome(picks, own_words, recommended)})
    return out


def text_blocks(content):
    """Text strings from a message's string content or typed block list."""
    if isinstance(content, str):
        return [content]
    return [b.get("text", "") for b in content or []
            if isinstance(b, dict) and b.get("type") == "text"]


def typed_text(m, record):
    """Their own words in a user record, or None for anything a tool or another session wrote."""
    if record.get("isMeta") or record.get("isCompactSummary"):
        return None
    if (record.get("origin") or {}).get("kind") != "human":
        return None
    content = record.get("message", {}).get("content")
    blocks = text_blocks(content)
    text = "\n".join(t for t in (REMINDER.sub("", b).strip() for b in blocks) if t and not t.startswith("<"))
    return own_message(m, text)


def own_message(m, text):
    """The text, unless it is empty, pasted, a one-word nudge or too long to be typed."""
    if not text or text.startswith(m.pasted) or text.lower() in NOISE or len(text) > MAX_MESSAGE_CHARS:
        return None
    return text


def assistant_text(record):
    content = record.get("message", {}).get("content")
    return "\n".join(text_blocks(content))


def searchable(record):
    if record["kind"] == "message":
        return record["text"]
    labels = " ".join(o["label"] for o in record["options"])
    return " ".join((record["question"], labels, " ".join(record["picks"]), record["own_words"], record["notes"]))


def filtered(record):
    """Everything a record keeps, including the reply it answered: what the privacy filters read."""
    text = " ".join((searchable(record), record.get("context", ""), record.get("project", "")))
    if record["kind"] == "question":
        text += " " + record["header"] + " " + " ".join(o["description"] for o in record["options"])
    return text


def read_session(path):
    records = []
    for line in path.read_text(errors="ignore").splitlines():
        try:
            records.append(json.loads(line))
        except ValueError:
            continue
    return records


def harvest(m, path, project):
    harvested, context = [], ""
    for r in read_session(path):
        if r.get("type") == "assistant":
            context = assistant_text(r).strip() or context
        if r.get("type") != "user" or not r.get("timestamp"):
            continue
        harvested += questions_in(m, r, path.stem, project)
        text = typed_text(m, r)
        if text is not None:
            harvested.append(message_record(path.stem, r["timestamp"], project, text, context))
    return harvested


def message_record(session, when, project, text, context):
    return {"id": record_id("message", session, when, text), "kind": "message", "timestamp": when,
            "project": project, "text": text, "context": context[-CONTEXT_CHARS:]}


def privacy(m, record):
    """Which filter drops the record: "secret", "personal", or None to keep it."""
    text = filtered(record)
    if m.secret.search(text):
        return "secret"
    if m.personal.search(text):
        return "personal"
    return None


def mark_split(records, holdout, cutoff):
    questions = sorted(r["timestamp"] for r in records if r["kind"] == "question")
    if cutoff is None and questions:
        held = max(1, round(len(questions) * holdout))
        cutoff = questions[len(questions) - held]
    for r in records:
        r["split"] = "heldout" if cutoff is not None and r["timestamp"] >= cutoff else "train"
    return cutoff


def private_directory(path):
    """Create missing directories with mode 0700 and return an open directory descriptor."""
    path = Path(path).expanduser().resolve()
    directory_flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW
    parent_fd = os.open(path.anchor, directory_flags)
    try:
        for part in path.parts[1:]:
            try:
                os.mkdir(part, 0o700, dir_fd=parent_fd)
            except FileExistsError:
                pass
            child_fd = os.open(part, directory_flags, dir_fd=parent_fd)
            os.close(parent_fd)
            parent_fd = child_fd
        return parent_fd
    except OSError:
        os.close(parent_fd)
        raise


def private_write(path, text):
    """Atomically replace private output through exclusive, owner-only temporary files."""
    path = Path(path).expanduser()
    parent_fd = private_directory(path.parent)
    temporary = None
    try:
        temporary_name = "." + path.name + "." + uuid.uuid4().hex + ".tmp"
        fd = os.open(temporary_name, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                     0o600, dir_fd=parent_fd)
        temporary = temporary_name
        with os.fdopen(fd, "w", encoding="utf-8") as output:
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


def write_jsonl(path, records):
    private_write(path, "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in records))


def read_jsonl(path):
    return [json.loads(line) for line in Path(path).read_text().split("\n") if line.strip()]


def cmd_mine(m, args):
    root = Path(args.projects).expanduser()
    kept, excluded, dropped = [], 0, Counter()
    for folder in sorted(p for p in root.iterdir() if p.is_dir()):
        if m.excluded.search(folder.name):
            excluded += 1
            continue
        project = project_name(folder.name)
        for path in sorted(folder.glob("*.jsonl")):
            for h in harvest(m, path, project):
                reason = privacy(m, h)
                if reason:
                    dropped[reason] += 1
                else:
                    kept.append(h)
    kept.sort(key=lambda r: (r["timestamp"], r["id"]))
    cutoff = mark_split(kept, args.holdout, args.cutoff)
    write_jsonl(args.out, kept)
    questions = [r for r in kept if r["kind"] == "question"]
    held = sum(r["split"] == "heldout" for r in questions)
    span = f"{kept[0]['timestamp'][:10]} → {kept[-1]['timestamp'][:10]}" if kept else "none"
    print(f"mined {len(questions)} questions ({held} held out) and {len(kept) - len(questions)} messages, "
          f"{span}; held out from {cutoff or 'nothing'}")
    print(f"dropped {excluded} excluded folders, {dropped['personal']} personal records, "
          f"{dropped['secret']} secret records")
    return 0


def check_labels(corpus, labels):
    ids = {r["id"]: r for r in corpus}
    missing = sorted(i for i, r in ids.items() if r["split"] == "train" and i not in labels)
    unknown = sorted(labels.keys() - ids.keys())
    peeked = sorted(i for i, label in labels.items() if i in ids and ids[i]["split"] == "heldout" and "reason" in label)
    problems = []
    if missing:
        problems.append(f"unlabelled {', '.join(missing)}")
    if unknown:
        problems.append(f"unknown {', '.join(unknown)}")
    if peeked:
        problems.append(f"held-out records carry a reason: {', '.join(peeked)}")
    if problems:
        raise Failed("labels do not match the corpus: " + "; ".join(problems))


def share(n, total):
    return round(n / total, 4) if total else 0.0


def tally(corpus, labels):
    check_labels(corpus, labels)
    out = defaultdict(lambda: {"questions": 0, "took_rec": 0, "other_option": 0, "own_words": 0,
                               "no_rec": 0, "messages": 0, "rec_share": 0.0, "last_seen": ""})
    for r in corpus:
        if r["split"] != "train":
            continue
        c = out[labels[r["id"]]["category"]]
        if r["kind"] == "question":
            c["questions"] += 1
            c[r["outcome"]] += 1
        else:
            c["messages"] += 1
        c["last_seen"] = max(c["last_seen"], r["timestamp"][:10])
    for c in out.values():
        c["rec_share"] = share(c["took_rec"], c["questions"] - c["no_rec"])
    return dict(sorted(out.items(), key=lambda kv: (-kv[1]["questions"] - kv[1]["messages"], kv[0])))


def tally_markdown(result):
    lines = ["| Category | Questions | Took rec | Other option | Own words | No rec | Rec share | Messages | Last seen |",
             "|---|---|---|---|---|---|---|---|---|"]
    for name, c in result.items():
        lines.append(f"| {name} | {c['questions']} | {c['took_rec']} | {c['other_option']} | {c['own_words']} | "
                     f"{c['no_rec']} | {c['rec_share']:.0%} | {c['messages']} | {c['last_seen']} |")
    return "\n".join(lines)


def cmd_tally(m, args):
    result = tally(read_jsonl(args.corpus), json.loads(Path(args.labels).read_text()))
    print(json.dumps(result, indent=2, ensure_ascii=False) if args.json else tally_markdown(result))
    return 0


QUESTION_FIELDS = ("id", "timestamp", "project", "header", "question", "multiSelect", "options", "recommended")


def answer_fields(m):
    return ("id", "recommended", m.pick_field, "picks", "own_words", "notes", "outcome", "multiSelect")


def cmd_split(m, args):
    held = [r for r in read_jsonl(args.corpus) if r["kind"] == "question" and r["split"] == "heldout"]
    write_jsonl(args.questions, [{k: r[k] for k in QUESTION_FIELDS} for r in held])
    write_jsonl(args.answers, [{k: r[k] for k in answer_fields(m)} for r in held])
    print(f"split {len(held)} held-out questions")
    return 0


def bare(m, label):
    return m.mark.sub("", label).strip()


def is_hit(m, answer, prediction):
    pick = prediction.get("pick")
    if answer["outcome"] == "own_words":
        return pick == "own"
    if answer["multiSelect"]:
        picks = pick if isinstance(pick, list) else [pick]
        return {bare(m, p) for p in picks if isinstance(p, str)} == {bare(m, p) for p in answer["picks"]}
    return bool(answer["picks"]) and isinstance(pick, str) and bare(m, pick) == bare(m, answer["picks"][0])


def cell(n, hits):
    return {"n": n, "hits": hits, "accuracy": share(hits, n)}


def score(m, answers, labels, predictions):
    missing = sorted(a["id"] for a in answers if a["id"] not in predictions)
    if missing:
        raise Failed(f"no prediction for {', '.join(missing)}")
    tiers, categories = defaultdict(lambda: [0, 0]), defaultdict(lambda: defaultdict(lambda: [0, 0]))
    hits = baseline = 0
    led_n = led_hits = took_member = 0
    for a in answers:
        p = predictions[a["id"]]
        tier = p.get("confidence")
        if tier not in TIERS:
            raise Failed(f"{a['id']}: confidence must be one of {', '.join(TIERS)}")
        hit = is_hit(m, a, p)
        hits += hit
        baseline += a["outcome"] == "took_rec"
        if a.get(m.pick_field):
            led_n += 1
            led_hits += hit
            took_member += bare(m, a[m.pick_field]) in {bare(m, p) for p in a["picks"]}
        for bucket in (tiers[tier], categories[(labels.get(a["id"]) or {}).get("category", "unlabelled")][tier]):
            bucket[0] += 1
            bucket[1] += hit
    n = len(answers)
    return {"overall": dict(cell(n, hits), baseline=share(baseline, n)),
            f"{m.slug}_led": dict(cell(led_n, led_hits), **{f"took_{m.slug}": share(took_member, led_n)}),
            "tiers": {t: cell(*tiers[t]) for t in TIERS if t in tiers},
            "categories": {c: {t: cell(*v[t]) for t in TIERS if t in v} for c, v in sorted(categories.items())}}


def score_markdown(m, result):
    o = result["overall"]
    lines = [f"Overall: {o['hits']} of {o['n']} ({o['accuracy']:.0%}); always-take-the-recommendation "
             f"baseline {o['baseline']:.0%}"]
    c = result[f"{m.slug}_led"]
    if c["n"]:
        lines.append(f"{m.name}-led questions: {c['hits']} of {c['n']} ({c['accuracy']:.0%}); "
                     f"{m.principal} took {m.name}'s pick {c[f'took_{m.slug}']:.0%}")
    lines += ["", "| Tier | N | Hits | Accuracy |", "|---|---|---|---|"]
    lines += [f"| {t} | {c['n']} | {c['hits']} | {c['accuracy']:.0%} |" for t, c in result["tiers"].items()]
    lines += ["", "| Category | " + " | ".join(TIERS) + " |", "|---|" + "---|" * len(TIERS)]
    for name, tiers in result["categories"].items():
        cells = [f"{tiers[t]['hits']}/{tiers[t]['n']}" if t in tiers else "–" for t in TIERS]
        lines.append(f"| {name} | " + " | ".join(cells) + " |")
    return "\n".join(lines)


def cmd_score(m, args):
    result = score(m, read_jsonl(args.answers), json.loads(Path(args.labels).read_text()),
                   json.loads(Path(args.predictions).read_text()))
    print(json.dumps(result, indent=2) if args.json else score_markdown(m, result))
    return 0


STOPWORDS = set("the and for that this with what should which from into when where does would there their "
                "they them then than have has had will can are was were you your our its it's not but all any "
                "now one two how who why use".split())


def words(text):
    return {w for w in re.findall(r"[a-z][a-z0-9-]{2,}", text.lower()) if w not in STOPWORDS}


MIN_SHARED_WORDS = 2  # one shared word matched "review that PR" to any question about reviews
MIN_MESSAGE_WORDS = 8  # a one-line status check or go-ahead is no precedent for a decision


def similar(m, corpus, query, k):
    train = [r for r in corpus if r["split"] == "train"
             and (r["kind"] == "question" or len(r["text"].split()) >= MIN_MESSAGE_WORDS)]
    docs = [(r, words(searchable(r) + " " + r.get("header", ""))) for r in train]
    df = Counter(w for _, ws in docs for w in ws)
    wanted = words(query)
    scored = []
    for r, ws in docs:
        shared = wanted & ws
        if len(shared) >= MIN_SHARED_WORDS:
            weight = sum(math.log(len(docs) / df[w]) + 1 for w in shared) / math.sqrt(len(ws))
            scored.append((weight, r["timestamp"], r))
    scored.sort(key=lambda t: (-t[0], t[1]))
    return [precedent(m, r) for _, _, r in scored[:k]]


def precedent(m, r):
    if r["kind"] == "message":
        return {"kind": "message", "date": r["timestamp"][:10], "project": r["project"], "text": r["text"][:400]}
    return {"kind": "question", "date": r["timestamp"][:10], "project": r["project"], "header": r["header"],
            "question": r["question"], "options": [o["label"] for o in r["options"]],
            "recommended": r["recommended"], m.pick_field: r.get(m.pick_field),
            "picks": r["picks"], "own_words": r["own_words"][:400],
            "notes": r["notes"][:400], "outcome": r["outcome"]}


def cmd_similar(m, args):
    hits = similar(m, read_jsonl(args.corpus or m.state("decisions.jsonl")), args.query, args.k)
    if args.json:
        print(json.dumps(hits, indent=2, ensure_ascii=False))
        return 0
    for h in hits:
        if h["kind"] == "message":
            print(f"- {h['date']} {h['project']} · {m.principal_subject} said: {h['text']}")
        else:
            answer = ", ".join(h["picks"]) + (f" + own words: {h['own_words']}" if h["own_words"] else "")
            print(f"- {h['date']} {h['project']} · {h['question']}\n    options: {' / '.join(h['options'])}"
                  f"\n    {m.principal_subject} chose: {answer} ({h['outcome']})" + (f"\n    notes: {h['notes']}" if h["notes"] else ""))
    return 0


def cmd_record(m, args):
    line = {"timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "work": args.work,
            "kind": args.kind, "category": args.category, "tier": args.tier, "question": args.question,
            m.pick_field: args.member_pick, "rule": args.rule, m.answer_field: args.principal_answer}
    log = Path(args.log or m.state("live.jsonl")).expanduser()
    previous = log.read_text() if log.exists() else ""
    private_write(log, previous + json.dumps(line, ensure_ascii=False) + "\n")
    print(f"recorded a {args.kind} {args.category} decision ({args.tier}) for {args.work}")
    return 0


def opens_a_turn(record):
    """A message from the principal or another session: a user record with text left once the app's
    system reminders are removed (those arrive as user records too, and answer nothing)."""
    if record.get("type") != "user" or record.get("isMeta"):
        return False
    content = record.get("message", {}).get("content")
    blocks = text_blocks(content)
    return any(REMINDER.sub("", b).strip() for b in blocks)


def open_asks(records):
    """AskUserQuestion calls with no tool result yet, and a closing wrap_up ask nothing has replied to."""
    asks, results = [], set()
    for i, r in enumerate(records):
        content = r.get("message", {}).get("content")
        for b in content if isinstance(content, list) else []:
            if not isinstance(b, dict):
                continue
            if b.get("type") == "tool_result":
                results.add(b.get("tool_use_id"))
            elif b.get("type") == "tool_use" and b.get("name") == "AskUserQuestion":
                for q in (b.get("input") or {}).get("questions") or []:
                    asks.append({"kind": "ask", "tool_id": b.get("id"), "when": r.get("timestamp"),
                                 "question": q.get("question", ""),
                                 "options": [o.get("label", "") for o in q.get("options") or []]})
            elif b.get("type") == "tool_use" and b.get("name") == "mcp__ccd_turn__wrap_up":
                ask = (b.get("input") or {}).get("ask")
                if ask and not any(opens_a_turn(later) for later in records[i + 1:]):
                    asks.append({"kind": "wrap_up", "tool_id": b.get("id"), "when": r.get("timestamp"),
                                 "question": ask,
                                 "options": [a.get("label", "") for a in (b.get("input") or {}).get("actions") or []]})
    return [a for a in asks if a["kind"] == "wrap_up" or a["tool_id"] not in results]


def control_header(line):
    """`(Q|A, key)` for a queue question or answer header, plain or as a heading; None for prose."""
    row = line.lstrip()
    text = row.lstrip("#")
    if 1 <= len(row) - len(text) <= 6 and text.startswith(" "):
        row = text.lstrip()
    if not row.startswith("🎛"):
        return None
    m = re.search(r"CONTROL-([QA]) ([A-Za-z0-9._-]+)(?: · |$)", re.sub(r"[*`]", "", row))
    return (m.group(1), m.group(2)) if m else None


def open_control_qs(m, queues):
    """Questions waiting in a queue file: keys whose newest header is a question.
    Read the last 512 KiB (QUEUE_TAIL_BYTES)."""
    found = []
    for path in sorted(Path(queues).glob("*/queue.md")):
        data = path.read_bytes()
        tail = data[-QUEUE_TAIL_BYTES:]
        first = data[:len(data) - len(tail)].count(b"\n") + 1
        open_by_key = {}
        for n, line in enumerate(tail.decode(errors="ignore").splitlines(), first):
            header = control_header(line)
            if not header:
                continue
            if header[0] == "Q":
                open_by_key[header[1]] = (n, line.strip())
            else:
                open_by_key.pop(header[1], None)
        for key, (n, line) in open_by_key.items():
            if not any(p.search(line) for p in (m.personal, m.secret)):
                found.append({"kind": "control-q", "project": path.parent.name, "key": key, "line": n,
                              "question": line, "options": []})
    return found


def answered_escalations(m, queues):
    """Answers after each key's newest escalation, with the asking session.
    Only headers written by the member's queue_answerer count, so a human ruling is never confused
    with an agent's answer; with no queue_answerer configured, none do.
    Read the same 512 KiB tail as open_control_qs."""
    found = []
    if not m.queue_answerer:
        return found
    for path in sorted(Path(queues).glob("*/queue.md")):
        data = path.read_bytes()
        tail = data[-QUEUE_TAIL_BYTES:]
        first = data[:len(data) - len(tail)].count(b"\n") + 1
        sessions, answers = {}, {}
        for n, line in enumerate(tail.decode(errors="ignore").splitlines(), first):
            header = control_header(line)
            if not header:
                continue
            kind, key = header
            if kind == "Q":
                sessions.pop(key, None)
                answers.pop(key, None)
                session = re.search(r"session (\S+) \(Autonomous on\)", line)
                if session:
                    sessions[key] = session.group(1)
            elif key in sessions:
                parts = re.sub(r"[*`]", "", line).split(" · ")
                if len(parts) < 4 or parts[2] != m.queue_answerer or not parts[3].startswith("answered by "):
                    continue
                name = parts[3][len("answered by "):].strip()
                if not name or name == m.name or any(p.search(line) for p in (m.personal, m.secret)):
                    continue
                answers[key] = {"kind": "control-a", "project": path.parent.name, "key": key,
                                "line": n, "answer": line.strip(), "session": sessions[key]}
        found.extend(answers.values())
    return found


def cmd_pending(m, args):
    seen_path = Path(args.seen or m.state("autonomous/seen.json")).expanduser()
    seen = set(json.loads(seen_path.read_text())) if seen_path.exists() else set()
    cutoff = datetime.now(timezone.utc).timestamp() - args.since_hours * 3600
    items, projects = [], Path(args.projects).expanduser()
    for folder in sorted(projects.iterdir()) if projects.is_dir() else []:
        if not folder.is_dir() or m.excluded.search(folder.name):
            continue
        for path in sorted(folder.glob("*.jsonl")):
            if path.stem in args.exclude or path.stat().st_mtime < cutoff:
                continue
            for a in open_asks(read_session(path)):
                text = a["question"] + " " + " ".join(a["options"])
                if m.personal.search(text) or m.secret.search(text):
                    continue
                items.append({**a, "session": path.stem, "project": project_name(folder.name),
                              "id": f"{path.stem}:{a['tool_id']}"})
    queues = args.queues or m.queues
    if queues and Path(queues).expanduser().is_dir():
        items += [{**q, "id": f"{q['project']}:{q['key']}:{q['line']}"}
                  for q in open_control_qs(m, Path(queues).expanduser())]
        items += [{**a, "id": f"{a['project']}:{a['key']}:A:{a['line']}"}
                  for a in answered_escalations(m, Path(queues).expanduser())]
    new = [i for i in items if i["id"] not in seen]
    for i in new:
        print(json.dumps(i, ensure_ascii=False))
    if new:
        private_write(seen_path, json.dumps(sorted(seen | {i["id"] for i in new})))
    return 0


# The claude.ai export. Anthropic documents how to request it but not its fields; these are the
# fields community documentation describes, and the only ones the adapter reads. `claudeai inspect`
# checks a real export has them before anything is mined.
CONVERSATION_FIELDS = ("uuid", "name", "created_at", "chat_messages")
MESSAGE_FIELDS = ("sender", "created_at")
MINE_MARKS = {"y", "yes", "mine"}
REVIEW_COLUMNS = ("mine", "created", "updated", "messages", "project", "title", "uuid")


def export_files(path):
    """The export's file names and its parsed conversations.json, from a zip, a folder or the file itself.
    Read in memory: nothing from the export is unpacked to disk."""
    path = Path(path).expanduser()
    if path.is_dir():
        names = sorted(p.name for p in path.iterdir() if p.is_file())
        return names, json.loads((path / "conversations.json").read_text())
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as z:
            names = sorted(n for n in z.namelist() if not n.endswith("/"))
            found = [n for n in names if Path(n).name == "conversations.json"]
            if len(found) != 1:
                raise Failed(f"{path}: expected one conversations.json, found {len(found)}")
            info = z.getinfo(found[0])
            if info.file_size > 2 * 1024**3:
                raise Failed("export member exceeds the 2 GB size cap")
            if info.file_size > 100 * info.compress_size:
                raise Failed("export member compression ratio exceeds 100:1")
            return [Path(n).name for n in names], json.loads(z.read(info))
    return [path.name], json.loads(path.read_text())


def type_name(value):
    return {dict: "object", list: "list", str: "str", bool: "bool", int: "int", float: "float",
            type(None): "null"}[type(value)]


def key_tree(items, prefix, tree):
    """Every key under prefix: the types seen and how many of the objects carry it."""
    objects = [i for i in items if isinstance(i, dict)]
    keys = sorted({k for o in objects for k in o})
    for k in keys:
        values = [o[k] for o in objects if k in o]
        entry = tree.setdefault(f"{prefix}.{k}", {"types": set(), "count": 0, "of": 0, "values": set()})
        entry["types"] |= {type_name(v) for v in values}
        entry["count"] += len(values)
        entry["of"] += len(objects)
        if k == "sender":
            entry["values"] |= {v for v in values if isinstance(v, str)}
        children = [c for v in values if isinstance(v, list) for c in v]
        if children:
            key_tree(children, f"{prefix}.{k}[]", tree)
        nested = [v for v in values if isinstance(v, dict)]
        if nested:
            key_tree(nested, f"{prefix}.{k}", tree)
    return tree


def missing_fields(tree):
    wanted = [f".{k}" for k in CONVERSATION_FIELDS] + [f".chat_messages[].{k}" for k in MESSAGE_FIELDS]
    missing = [w for w in wanted if w not in tree]
    if ".chat_messages[].text" not in tree and ".chat_messages[].content[].text" not in tree:
        missing.append(".chat_messages[].text or .chat_messages[].content[].text")
    return missing


def cmd_claudeai_inspect(m, args):
    names, conversations = export_files(args.export)
    if not isinstance(conversations, list):
        raise Failed(f"conversations.json is a {type_name(conversations)}, not a list")
    tree = key_tree(conversations, "", {})
    print(f"files: {len(names)}")
    print(f"conversations.json: list of {len(conversations)}")
    for key, e in tree.items():
        values = f" values: {', '.join(sorted(e['values']))}" if e["values"] else ""
        print(f"  {key}: {'|'.join(sorted(e['types']))} ({e['count']}/{e['of']}){values}")
    missing = missing_fields(tree)
    if missing:
        raise Failed(f"export lacks fields the adapter reads; missing: {', '.join(missing)}")
    print("ok: every field the adapter reads is present")
    return 0


def utc(when):
    """An export timestamp as UTC with milliseconds and a Z, the shape Claude Code transcripts use."""
    try:
        parsed = datetime.fromisoformat(when.replace("Z", "+00:00"))
    except (AttributeError, TypeError, ValueError):
        raise Failed(f"{when!r} is not a timestamp: the export's shape changed, run claudeai inspect")
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def dotted(value, path):
    for part in path.split("."):
        value = value.get(part) if isinstance(value, dict) else None
    return value if isinstance(value, str) else ""


def flat(text):
    return re.sub(r"\s+", " ", text or "").strip()


def chat_text(message):
    text = message.get("text")
    if isinstance(text, str) and text.strip():
        return text
    return "\n".join(text_blocks(message.get("content")))


def read_review(path):
    path = Path(path)
    if not path.exists():
        return []
    lines = path.read_text().splitlines()
    if not lines or tuple(lines[0].split("\t")) != REVIEW_COLUMNS:
        raise Failed(f"{path}: not a review file (header must be {' '.join(REVIEW_COLUMNS)})")
    return [dict(zip(REVIEW_COLUMNS, line.split("\t"))) for line in lines[1:] if line.strip()]


def write_review(path, rows):
    private_write(path, "\t".join(REVIEW_COLUMNS) + "\n" +
                  "".join("\t".join(r[c] for c in REVIEW_COLUMNS) + "\n" for r in rows))


OPENING_WORDS, LONGEST_WORD = 8, 24


def opening_title(m, humans):
    """An untitled conversation's stand-in title: its first human words, or [filtered] when that
    whole message trips a privacy filter, so the principal can tell their own from another's. A long
    word (a link, an ID) shows as …, since the filters do not catch every identifier."""
    text = next((t for t in (chat_text(x) for x in humans) if flat(t)), "")
    if not text:
        return ""
    if m.personal.search(text) or m.secret.search(text):
        return "[filtered]"
    words = flat(text).split(" ")
    shown = [w if len(w) <= LONGEST_WORD else "…" for w in words[:OPENING_WORDS]]
    return "opens: " + " ".join(shown) + ("…" if len(words) > OPENING_WORDS else "")


def cmd_claudeai_titles(m, args):
    _, conversations = export_files(args.export)
    marks_by_uuid = {r["uuid"]: r["mine"] for r in read_review(args.out)}
    rows, untitled = [], Counter()
    for c in sorted(conversations, key=lambda c: (utc(c["created_at"]), c["uuid"])):
        humans = [x for x in c["chat_messages"] if x.get("sender") == "human"]
        title = flat(c.get("name"))
        if not title:
            title = opening_title(m, humans)
            untitled["all"] += 1
            untitled["filtered" if title == "[filtered]" else "opens" if title else "empty"] += 1
        rows.append({"mine": marks_by_uuid.get(c["uuid"], ""), "created": utc(c["created_at"])[:10],
                     "updated": utc(c.get("updated_at") or c["created_at"])[:10], "messages": str(len(humans)),
                     "project": flat(dotted(c, args.project_field)) if args.project_field else "",
                     "title": title, "uuid": c["uuid"]})
    write_review(args.out, rows)
    kept = sum(1 for r in rows if r["mine"])
    shown = (f"; {untitled['all']} untitled: {untitled['opens']} by opening words, {untitled['filtered']} filtered"
             if untitled["all"] else "")
    print(f"listed {len(rows)} conversations in {args.out} ({kept} already marked{shown})")
    return 0


def cmd_claudeai_mark(m, args):
    if not (args.uuid or args.project or args.title or args.before or args.after or args.unmarked or args.all):
        raise Failed("mark needs a selector: --uuid, --project, --title, --before, --after, --unmarked or --all")
    try:
        title = re.compile(args.title) if args.title else None
    except re.error as exc:
        raise Failed(f"--title is not a valid regex ({exc})")
    rows, changed = read_review(args.review), 0
    for r in rows:
        if ((not args.uuid or r["uuid"] in args.uuid) and (not args.project or r["project"] == args.project)
                and (not title or title.search(r["title"])) and (not args.before or r["created"] <= args.before)
                and (not args.after or r["created"] >= args.after) and (not args.unmarked or not r["mine"])):
            changed += r["mine"] != args.mark
            r["mine"] = args.mark
    write_review(args.review, rows)
    print(f"marked {changed} conversations {args.mark}")
    return 0


def cmd_claudeai_mine(m, args):
    _, conversations = export_files(args.export)
    review = {r["uuid"]: r for r in read_review(args.review)}
    kept, dropped, read, not_mine, unmarked = [], Counter(), 0, 0, 0
    for c in conversations:
        row = review.get(c["uuid"])
        mark = (row or {}).get("mine", "").strip().lower()
        if mark not in MINE_MARKS:
            if mark:
                not_mine += 1
            else:
                unmarked += 1
            continue
        read += 1
        project = "claude.ai" + (f"/{row['project']}" if row["project"] else "")
        context = ""
        for x in c["chat_messages"]:
            if x.get("sender") == "assistant":
                context = chat_text(x).strip() or context
                continue
            if x.get("sender") != "human":
                continue
            text = own_message(m, chat_text(x).strip())
            if text is None:
                continue
            record = message_record(c["uuid"], utc(x["created_at"]), project, text, context)
            reason = privacy(m, record)
            if reason:
                dropped[reason] += 1
            else:
                kept.append(record)
    kept.sort(key=lambda r: (r["timestamp"], r["id"]))
    mark_split(kept, args.holdout, args.cutoff)
    write_jsonl(args.out, kept)
    print(f"read {read} conversations marked mine; skipped {not_mine} not mine and {unmarked} unmarked")
    print(f"mined {len(kept)} messages; dropped {dropped['personal']} personal records, "
          f"{dropped['secret']} secret records")
    return 0


def pack_words(items, limit, text_of):
    """Pack whole items in order; an oversized item stays whole."""
    current, words = [], 0
    for item in items:
        count = len(text_of(item).split())
        if current and words + count > limit:
            yield current
            current, words = [], 0
        current.append(item)
        words += count
    if current:
        yield current


def document_parts(m, source, paragraphs):
    kept, dropped = [], Counter(personal=0, secret=0)
    for paragraph in paragraphs:
        text = flat(paragraph)
        if not text:
            continue
        reason = "secret" if m.secret.search(text) else "personal" if m.personal.search(text) else None
        if reason:
            dropped[reason] += 1
        else:
            kept.append(text)
    parts = []
    for part, paragraphs in enumerate(pack_words(kept, PART_WORDS, lambda text: text), 1):
        text = "\n\n".join(paragraphs)
        parts.append({"id": record_id("document", source, str(part), text), "kind": "document",
                      "source": source, "part": part, "text": text})
    return parts, dropped


def cmd_documents(m, args):
    root, output = Path(args.dir).expanduser(), Path(args.out_dir).expanduser()
    resolved = output.resolve()
    if any(os.path.lexists(parent / ".git") for parent in (resolved, *resolved.parents)):
        raise Failed("out dir must not be inside a git work tree")
    if args.size <= 0:
        raise Failed("documents: size must be positive")
    if root.is_symlink() or not root.is_dir():
        raise Failed("documents: dir must be a directory, not a symlink")
    inventory, parts, totals, dropped = [], [], Counter(), Counter()
    for relative in crew_documents.walk(root):
        source = relative.as_posix()
        if m.personal.search(source) or m.secret.search(source):
            inventory.append({"source": "[filtered]", "kind": "skipped", "reason": "filtered"})
            totals["skipped"] += 1
            continue
        try:
            kind, paragraphs = crew_documents.read(root / relative, args.pdftotext)
        except crew_documents.Unreadable as exc:
            inventory.append({"source": source, "kind": "skipped", "reason": str(exc)})
            totals["skipped"] += 1
            continue
        if kind == "image":
            inventory.append({"source": source, "kind": kind})
            totals["images"] += 1
            continue
        document, counts = document_parts(m, source, paragraphs)
        words = sum(len(part["text"].split()) for part in document)
        inventory.append({"source": source, "kind": kind, "words": words,
                          "parts": len(document), "dropped": dict(counts)})
        parts.extend(document)
        dropped.update(counts)
        totals["documents"] += 1
        totals["words"] += words
    os.close(private_directory(output))
    for path in output.glob("documents-*.jsonl"):
        path.unlink()
    chunks = 0
    for chunks, records in enumerate(pack_words(parts, args.size, lambda r: r["text"]), 1):
        write_jsonl(output / f"documents-{chunks:02d}.jsonl", records)
    write_jsonl(output / "inventory.jsonl", inventory)
    print(f"read {totals['documents']} documents into {len(parts)} parts ({totals['words']} words) "
          f"in {chunks} chunks; listed {totals['images']} images; skipped {totals['skipped']} files")
    print(f"dropped {dropped['personal']} personal and {dropped['secret']} secret paragraphs")
    return 0


def round_error(number, message):
    raise Failed(f"round: question {number}: {message}")


def round_string(value, field, number, nonempty=False):
    if not isinstance(value, str) or (nonempty and not value.strip()):
        round_error(number, f"{field} must be {'a non-empty string' if nonempty else 'a string'}")
    return value


def round_question(question, seen):
    if not isinstance(question, dict):
        round_error("?", "question must be an object")
    number = question.get("n")
    if type(number) is not int or number <= 0 or number in seen:
        round_error(number, "n must be a distinct positive integer")
    seen.add(number)
    text = round_string(question.get("question"), "question", number, nonempty=True)
    header = round_string(question.get("header", ""), "header", number)
    multi = question.get("multiSelect", False)
    if not isinstance(multi, bool):
        round_error(number, "multiSelect must be a bool")
    options = question.get("options")
    if not isinstance(options, list) or not 2 <= len(options) <= 4:
        round_error(number, "options must contain 2 to 4 options")
    labels, normalized = set(), []
    for option in options:
        if not isinstance(option, dict):
            round_error(number, "options must be objects with a label")
        label = round_string(option.get("label"), "label", number, nonempty=True)
        if label in labels:
            round_error(number, "label must be distinct")
        labels.add(label)
        description = round_string(option.get("description", ""), "description", number)
        normalized.append({"label": label, "description": description})
    recommended = question.get("recommended")
    if recommended is not None and recommended not in list("ABCD"[:len(options)]):
        round_error(number, "recommended must be an option letter")
    for letter, option in zip("ABCD", normalized):
        if letter == recommended:
            option["label"] += " (Recommended)"
    recommendation = next((o["label"] for letter, o in zip("ABCD", normalized) if letter == recommended), None)
    return {"n": number, "question": text, "header": header, "multiSelect": multi,
            "options": normalized, "recommended": recommendation}


def round_answer(question, answer):
    number = question["n"]
    if not isinstance(answer, dict):
        round_error(number, "answer must be an object")
    own_words = round_string(answer.get("own_words", ""), "own_words", number)
    notes = round_string(answer.get("notes", ""), "notes", number)
    pick = answer.get("pick", [])
    picks = [pick] if isinstance(pick, str) else pick
    if not isinstance(picks, list) or any(letter not in list("ABCD"[:len(question["options"])]) for letter in picks):
        round_error(number, "pick is not an option")
    if not question["multiSelect"] and len(picks) > 1:
        round_error(number, "pick must have one option for a single-select question")
    labels = [o["label"] for letter, o in zip("ABCD", question["options"]) if letter in picks]
    return labels, own_words, notes


def read_round_file(path, field):
    try:
        value = json.loads(Path(path).expanduser().read_text())
    except (OSError, ValueError, RecursionError) as exc:
        raise Failed(f"round: {field}: {exc}")
    if not isinstance(value, dict):
        raise Failed(f"round: {field} must be an object")
    return value


def cmd_round(m, args):
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,39}", args.name):
        raise Failed("round: name must be 1 to 40 lowercase letters, digits or hyphens, starting with a letter or digit")
    questions = read_round_file(args.questions, "questions").get("questions")
    answers_file = read_round_file(args.answers, "answers")
    if not isinstance(questions, list):
        raise Failed("round: questions must be a list")
    seen = set()
    questions = [round_question(q, seen) for q in questions]
    answered = answers_file.get("answered")
    if not isinstance(answered, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}", answered):
        raise Failed("round: answered must be YYYY-MM-DD")
    try:
        timestamp = datetime.strptime(answered, "%Y-%m-%d").replace(hour=12, tzinfo=timezone.utc)
    except ValueError:
        raise Failed("round: answered must be a valid date")
    answers = answers_file.get("answers")
    if not isinstance(answers, dict):
        raise Failed("round: answers must be an object")
    numbers = {str(q["n"]) for q in questions}
    for number in answers:
        if number not in numbers:
            round_error(number, "answer has no question")
    kept, unanswered, dropped = [], 0, Counter()
    split = "heldout" if args.heldout else "train"
    for question in questions:
        number = question["n"]
        picks, own_words, notes = round_answer(question, answers.get(str(number), {}))
        if not picks and not own_words.strip():
            unanswered += 1
            continue
        try:
            when = (timestamp + timedelta(seconds=number)).isoformat(timespec="milliseconds").replace("+00:00", "Z")
        except OverflowError:
            round_error(number, "n exceeds the timestamp range")
        record = {k: v for k, v in question.items() if k != "n"}
        record.update(id=f"question:{args.name}-{number:02d}", kind="question", timestamp=when,
                      project=f"round-{args.name}", picks=picks, own_words=own_words, notes=notes,
                      outcome=outcome(picks, own_words, question["recommended"]), split=split)
        record[m.pick_field] = None
        reason = privacy(m, record)
        if reason:
            dropped[reason] += 1
        else:
            kept.append(record)
    write_jsonl(args.out or m.state(f"decisions-round-{args.name}.jsonl"), kept)
    print(f"imported {len(kept)} answers from round {args.name} ({split}); {unanswered} unanswered")
    print(f"dropped {dropped['personal']} personal records, {dropped['secret']} secret records")
    return 0


def cmd_merge(m, args):
    """Every input's records once, re-split as one corpus. The privacy filters run again, so a
    filter tightened since another Mac mined its corpus still applies. Records from a --train input
    (the calibration interview) always train; --rounds keeps imported splits. Neither moves the cutoff."""
    inputs = ([(p, "history") for p in args.inputs] + [(p, "train") for p in args.train] +
              [(p, "round") for p in args.rounds])
    if not inputs:
        raise Failed("merge needs at least one input")
    by_id, seen, fixed_splits, duplicates, dropped = {}, set(), {}, 0, Counter()
    for path, mode in inputs:
        for r in read_jsonl(path):
            if not isinstance(r, dict):
                raise Failed(f"{path}: a line is not a JSON object")
            if mode == "round" and r.get("split") not in ("train", "heldout"):
                raise Failed(f"{path}: round split must be train or heldout")
            if r["id"] in seen:
                duplicates += 1
                continue
            seen.add(r["id"])
            reason = privacy(m, r)
            if reason:
                dropped[reason] += 1
                continue
            by_id[r["id"]] = r
            if mode != "history":
                fixed_splits[r["id"]] = "train" if mode == "train" else r["split"]
    merged = sorted(by_id.values(), key=lambda r: (r["timestamp"], r["id"]))
    cutoff = mark_split([r for r in merged if r["id"] not in fixed_splits], args.holdout, args.cutoff)
    for r in merged:
        if r["id"] in fixed_splits:
            r["split"] = fixed_splits[r["id"]]
    write_jsonl(args.out, merged)
    questions = [r for r in merged if r["kind"] == "question"]
    held = sum(r["split"] == "heldout" for r in questions)
    print(f"merged {len(questions)} questions ({held} held out) and {len(merged) - len(questions)} messages "
          f"from {len(inputs)} files; {duplicates} duplicates dropped; "
          f"held out from {cutoff or 'nothing'}")
    print(f"dropped {dropped['personal']} personal records, {dropped['secret']} secret records")
    return 0


def cmd_folders(m, args):
    root = Path(args.projects).expanduser()
    totals = {"mined": [0, 0], "excluded": [0, 0]}
    for folder in sorted((p for p in root.iterdir() if p.is_dir()), key=lambda p: (project_name(p.name), p.name)):
        state = "excluded" if m.excluded.search(folder.name) else "mined"
        sessions = len(list(folder.glob("*.jsonl")))
        totals[state][0] += 1
        totals[state][1] += sessions
        print(f"{state}\t{sessions}\t{project_name(folder.name)}\t{folder.name}")
    print(f"{totals['mined'][0]} mined folders ({totals['mined'][1]} sessions), "
          f"{totals['excluded'][0]} excluded ({totals['excluded'][1]} sessions)")
    return 0


def cmd_claudeai(m, args):
    return {"inspect": cmd_claudeai_inspect, "titles": cmd_claudeai_titles, "mark": cmd_claudeai_mark,
            "mine": cmd_claudeai_mine}[args.step](m, args)



# Labelling. The member's codebook names the categories, domains, escalation classes and message
# signals a labeller may use, and the category groups distillation works in.
MAX_REASON_WORDS = 25
DESCRIPTION_CHARS = 150


def codebook(m):
    book = m.codebook
    if not book:
        raise Failed("member config: missing codebook")
    missing = [f"codebook.{k}" for k in ("categories", "escalates", "signals", "groups") if k not in book]
    if missing:
        raise Failed(f"member config: missing {', '.join(missing)}")
    return book


def cmd_chunk(m, args):
    corpus = read_jsonl(args.corpus)
    train = [r for r in corpus if r["split"] == "train"]
    held = [{"kind": "question", **{k: r[k] for k in QUESTION_FIELDS}}
            for r in corpus if r["split"] == "heldout" and r["kind"] == "question"]
    out = Path(args.out_dir)
    os.close(private_directory(out))
    counts = {}
    for name, records in (("train", train), ("heldout", held)):
        parts = [records[i:i + args.size] for i in range(0, len(records), args.size)]
        for n, part in enumerate(parts, 1):
            write_jsonl(out / f"{name}-{n:02d}.jsonl", part)
        counts[name] = len(parts)
    print(f"{len(train)} train records in {counts['train']} chunks, "
          f"{len(held)} held-out questions in {counts['heldout']} chunks")
    return 0


def label_problems(book, records, labels, mode):
    problems = []
    if set(labels) != set(records):
        problems.append(f"ids differ: missing {len(set(records) - set(labels))}, extra {len(set(labels) - set(records))}")
    for i, label in labels.items():
        r = records.get(i)
        if not r:
            continue
        if label.get("category") not in book["categories"]:
            problems.append(f"{i} category")
        if book.get("domains") and label.get("domain") not in book["domains"]:
            problems.append(f"{i} domain")
        if r["kind"] == "question" and not set(label.get("escalates", [])) <= set(book["escalates"]):
            problems.append(f"{i} escalates")
        if r["kind"] == "message" and label.get("signal") not in book["signals"]:
            problems.append(f"{i} signal")
        reason = label.get("reason")
        if mode == "train" and not (isinstance(reason, str) and 0 < len(reason.split()) <= MAX_REASON_WORDS):
            problems.append(f"{i} reason")
        if mode == "heldout" and "reason" in label:
            problems.append(f"{i} has reason")
    return problems


def cmd_check_labels(m, args):
    book = codebook(m)
    records = {r["id"]: r for r in read_jsonl(args.chunk)}
    problems = label_problems(book, records, json.loads(Path(args.labels).read_text()), args.mode)
    print("OK" if not problems else "PROBLEMS:\n" + "\n".join(problems[:40]))
    return 1 if problems else 0


def distil_row(r, label):
    row = {"date": r["timestamp"][:10], "project": r["project"], "category": label["category"]}
    if "domain" in label:
        row["domain"] = label["domain"]
    if r["kind"] == "question":
        row.update(q=r["question"],
                   options=[o["label"] + (" — " + o["description"][:DESCRIPTION_CHARS] if o["description"] else "")
                            for o in r["options"]],
                   picks=r["picks"], own=r["own_words"], notes=r["notes"], outcome=r["outcome"],
                   reason=label["reason"])
    else:
        row.update(msg=r["text"], signal=label["signal"], reason=label["reason"])
    return row


def cmd_distil_in(m, args):
    groups = codebook(m)["groups"]
    group_of = {c: g for g, cs in groups.items() for c in cs}
    labels = json.loads(Path(args.labels).read_text())
    out = {g: [] for g in groups}
    for r in read_jsonl(args.corpus):
        if r["split"] != "train":
            continue
        label = labels[r["id"]]
        if label["category"] not in group_of:
            raise Failed(f"category {label['category']} is in no codebook group")
        out[group_of[label["category"]]].append(distil_row(r, label))
    folder = Path(args.out_dir)
    os.close(private_directory(folder))
    for g, rows in out.items():
        write_jsonl(folder / f"{g}.jsonl", rows)
        print(g, len(rows))
    return 0


def tokens(text):
    return re.findall(r"[a-z0-9']+", text.lower())


def shingles(text, n):
    t = tokens(text)
    return {" ".join(t[i:i + n]) for i in range(len(t) - n + 1)}


def cmd_shingle(m, args):
    raw = set()
    for path in args.corpora:
        for r in read_jsonl(path):
            for key in ("text", "own_words", "notes", "question", "context"):
                if r.get(key):
                    raw |= shingles(r[key], args.n)
            for o in r.get("options", []):
                raw |= shingles(o["label"] + " " + o["description"], args.n)
    hits = sorted(shingles(Path(args.doc).read_text(), args.n) & raw)
    print(f"{len(hits)} overlaps")
    for h in hits[:60]:
        print("  ", h)
    return 1 if hits else 0


def string_paths(value, path=""):
    """Every string in a JSON value as (key path, string, is a key). A key comes with its parent's
    path, so a key that trips a filter is never printed itself."""
    if isinstance(value, dict):
        for k, v in value.items():
            yield path or "(top level)", k, True
            yield from string_paths(v, f"{path}.{k}" if path else k)
    elif isinstance(value, list):
        for i, v in enumerate(value):
            yield from string_paths(v, f"{path}[{i}]")
    elif isinstance(value, str):
        yield path, value, False


def privacy_lines(m, text):
    """The 1-based lines a privacy filter matches in text, each with its kind (secret wins)."""
    found = {}
    for kind, pattern in (("personal", m.personal), ("secret", m.secret)):
        for match in pattern.finditer(text):
            found[text.count("\n", 0, match.start()) + 1] = kind
    return sorted(found.items())


def privacy_files(folder):
    """Tracked paths in a clone; otherwise walk files, excluding ignored paths and OS junk."""
    folder = folder.resolve()
    tracked = subprocess.run(["git", "-C", str(folder), "ls-files", "-z", "--", "."],
                             capture_output=True)
    if tracked.returncode == 0:
        paths = [folder / os.fsdecode(name) for name in tracked.stdout.split(b"\0") if name]
    else:
        paths = list(folder.rglob("*"))
        ignored = subprocess.run(["git", "-C", str(folder), "check-ignore", "--no-index", "-z", "--stdin"],
                                 input=b"".join(os.fsencode(p) + b"\0" for p in paths), capture_output=True)
        ignored_paths = set(ignored.stdout.split(b"\0")) if ignored.returncode in (0, 1) else set()
        paths = [p for p in paths if os.fsencode(p) not in ignored_paths
                 and not any(part in {".git", ".DS_Store", "__MACOSX", "Thumbs.db", "desktop.ini"}
                             or part.startswith("._") for part in p.relative_to(folder).parts)]
    return sorted(p for p in paths if p.is_file() and not p.is_symlink())


def cmd_privacy_check(m, args):
    """Step 11's gate over every committed file of a crew folder, at any depth (a kit may add any).
    The member's own filters are the patterns, so they are compared with the template's instead;
    the gitignored private.json is skipped."""
    folder = Path(args.dir).expanduser().resolve()
    paths = privacy_files(folder)
    member = folder / "member.json"
    if not member.is_file():
        raise Failed(f"{member}: no member.json to check")
    config = read_json(member)
    if not isinstance(config, dict):
        raise Failed(f"{member}: member.json must be a JSON object")
    hits = []
    for path in (p for p in paths if p not in (member, folder / "private.json")):
        name = path.relative_to(folder)
        try:
            text = path.read_bytes().decode("utf-8")
        except UnicodeDecodeError:
            hits.append(f"{name} (not text)")
            continue
        hits += [f"{name}:{line} ({kind})" for line, kind in privacy_lines(m, text)]
    config = config if member in paths else {}
    for key, text, is_key in string_paths({k: v for k, v in config.items() if k != "filters"}):
        hits += [f"member.json:{key} ({'a key, ' if is_key else ''}{kind})" for _, kind in privacy_lines(m, text)[:1]]
    template = read_json(Path(__file__).resolve().parent.parent / "crew" / "_template" / "member.json")["filters"]
    own = config.get("filters") or {}
    differ = [k for k in sorted({*template, *own}) if template.get(k) != own.get(k)] if config else []
    hits += [f"member.json:filters.{k} (differs from crew/_template)" for k in differ]
    for hit in hits:
        print(hit)
    if differ:
        print("move member-specific filter patterns to the gitignored private.json and copy filters from "
              "crew/_template/member.json")
    print(f"{len(hits)} hits" if hits else "clean")
    return 1 if hits else 0


COMMANDS = {"mine": cmd_mine, "tally": cmd_tally, "split": cmd_split, "score": cmd_score,
            "similar": cmd_similar, "record": cmd_record, "pending": cmd_pending, "claudeai": cmd_claudeai,
            "documents": cmd_documents, "round": cmd_round, "merge": cmd_merge, "folders": cmd_folders,
            "chunk": cmd_chunk, "check-labels": cmd_check_labels, "distil-in": cmd_distil_in,
            "shingle": cmd_shingle, "privacy-check": cmd_privacy_check}


def parser_for(m, description):
    parser = argparse.ArgumentParser(description=description)
    parser.add_argument("--member", help="the member config (default: $CREW_MEMBER)")
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("mine")
    p.add_argument("--projects", default="~/.claude/projects")
    p.add_argument("--out", required=True)
    p.add_argument("--holdout", type=float, default=0.15)
    p.add_argument("--cutoff", help="hold out from this timestamp instead of a share")
    p = sub.add_parser("tally")
    p.add_argument("--corpus", required=True)
    p.add_argument("--labels", required=True)
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("split")
    p.add_argument("--corpus", required=True)
    p.add_argument("--questions", required=True)
    p.add_argument("--answers", required=True)
    p = sub.add_parser("score")
    p.add_argument("--answers", required=True)
    p.add_argument("--labels", required=True)
    p.add_argument("--predictions", required=True)
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("similar")
    p.add_argument("--corpus", help="default: <state_dir>/decisions.jsonl")
    p.add_argument("--query", required=True)
    p.add_argument("--k", type=int, default=6)
    p.add_argument("--json", action="store_true")
    p = sub.add_parser("record")
    p.add_argument("--log", help="default: <state_dir>/live.jsonl")
    p.add_argument("--work", required=True)
    p.add_argument("--kind", required=True, choices=("decided", "escalated"))
    p.add_argument("--category", required=True)
    p.add_argument("--tier", required=True, choices=TIERS)
    p.add_argument("--question", required=True)
    p.add_argument(f"--{m.slug}-pick" if m else "--member-pick", dest="member_pick", required=True)
    p.add_argument("--rule")
    p.add_argument(f"--{m.principal_slug}-answer" if m else "--principal-answer", dest="principal_answer")
    p = sub.add_parser("pending")
    p.add_argument("--projects", default="~/.claude/projects")
    p.add_argument("--queues", help="default: the member config's queues, if any")
    p.add_argument("--seen", help="default: <state_dir>/autonomous/seen.json")
    p.add_argument("--since-hours", type=float, default=24)
    p.add_argument("--exclude", action="append", default=[], help="a session id to skip (the watcher's own)")
    claudeai = sub.add_parser("claudeai", help="the claude.ai export: inspect, review, mine")
    steps = claudeai.add_subparsers(dest="step", required=True)
    p = steps.add_parser("inspect")
    p.add_argument("--export", required=True, help="the export zip, its folder, or conversations.json")
    p = steps.add_parser("titles")
    p.add_argument("--export", required=True)
    p.add_argument("--out", required=True, help="the review file (TSV); marks already in it are kept")
    p.add_argument("--project-field", help="dotted path to a project name inspect showed, e.g. project.name")
    p = steps.add_parser("mark")
    p.add_argument("--review", required=True)
    p.add_argument("--as", dest="mark", required=True, choices=("y", "n"))
    p.add_argument("--uuid", action="append", default=[])
    p.add_argument("--project")
    p.add_argument("--title", help="a regex on the title")
    p.add_argument("--before", help="created on or before this date (YYYY-MM-DD)")
    p.add_argument("--after", help="created on or after this date (YYYY-MM-DD)")
    p.add_argument("--unmarked", action="store_true")
    p.add_argument("--all", action="store_true")
    p = steps.add_parser("mine")
    p.add_argument("--export", required=True)
    p.add_argument("--review", required=True)
    p.add_argument("--out", required=True)
    p.add_argument("--holdout", type=float, default=0.15)
    p.add_argument("--cutoff")
    p = sub.add_parser("merge")
    p.add_argument("--in", dest="inputs", nargs="*", default=[])
    p.add_argument("--rounds", nargs="*", default=[], help="question-round files: each record keeps its imported split")
    p.add_argument("--train", nargs="*", default=[], help="inputs whose records always train (the interview)")
    p.add_argument("--out", required=True)
    p.add_argument("--holdout", type=float, default=0.15)
    p.add_argument("--cutoff")
    p = sub.add_parser("documents")
    p.add_argument("--dir", required=True)
    p.add_argument("--out-dir", required=True)
    p.add_argument("--size", type=int, default=6000)
    p.add_argument("--pdftotext")
    p = sub.add_parser("round")
    p.add_argument("--name", required=True)
    p.add_argument("--questions", required=True)
    p.add_argument("--answers", required=True)
    p.add_argument("--heldout", action="store_true")
    p.add_argument("--out")
    p = sub.add_parser("folders")
    p.add_argument("--projects", default="~/.claude/projects")
    p = sub.add_parser("chunk")
    p.add_argument("--corpus", required=True)
    p.add_argument("--out-dir", required=True)
    p.add_argument("--size", type=int, default=300)
    p = sub.add_parser("check-labels")
    p.add_argument("--chunk", required=True)
    p.add_argument("--labels", required=True)
    p.add_argument("--mode", required=True, choices=("train", "heldout"))
    p = sub.add_parser("distil-in")
    p.add_argument("--corpus", required=True)
    p.add_argument("--labels", required=True)
    p.add_argument("--out-dir", required=True)
    p = sub.add_parser("shingle", help="exit 1 when the doc shares n words in a row with the corpus")
    p.add_argument("--doc", required=True)
    p.add_argument("--corpus", dest="corpora", nargs="+", required=True)
    p.add_argument("--n", type=int, default=6)
    p = sub.add_parser("privacy-check", help="exit 1 when a committed file in the crew folder trips a filter")
    p.add_argument("--dir", required=True, help="the crew folder, crew/<slug>")
    return parser


def resolve_member(argv, member):
    """The member dict passed in, else the file --member or $CREW_MEMBER names. The member must be
    known before parsing, because `record`'s option names come from it."""
    if member is not None:
        return member
    pre = argparse.ArgumentParser(add_help=False)
    pre.add_argument("--member")
    known, _ = pre.parse_known_args(argv)
    path = known.member or os.environ.get("CREW_MEMBER")
    if not path:
        raise Failed("no member config (--member or CREW_MEMBER)")
    return read_member(path)


def main(argv=None, member=None, prog="crew_decisions", description=None):
    argv = sys.argv[1:] if argv is None else argv
    description = description or __doc__.split("\n\n")[0]
    try:
        m = load_member(resolve_member(argv, member))
    except (Failed, OSError) as exc:
        if "-h" in argv or "--help" in argv:
            parser_for(None, description).print_help()
            return 0
        print(f"{prog}: " + " ".join(str(exc).splitlines()), file=sys.stderr)
        return 1
    args = parser_for(m, description).parse_args(argv)
    for name in ("corpus", "answers", "labels", "predictions"):
        if getattr(args, name, None):
            setattr(args, name, str(Path(getattr(args, name)).expanduser()))
    try:
        return COMMANDS[args.cmd](m, args)
    except (Failed, OSError, ValueError, KeyError, RecursionError) as exc:
        print(f"{prog}: " + " ".join(str(exc).splitlines()), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
