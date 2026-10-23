#!/usr/bin/env python3
"""Decision cards: per-person decision models, blind backtests, and a panel of personas.

A team's recorded arguments become cards: one per decision, with a neutral question, the 2 to 4
options people argued, each person's stance and the outcome. Cards are paraphrase written by a
model pass (briefs in crew/_template/briefs/); this script is the deterministic half: no LLM, no
network. One card feeds every participant's model, so the models learn from each other only
through what really happened.

  check      --cards C [--categories CATS]   every card well formed (exit 1 lists the problems)
  split      --cards C --person P [--holdout 0.25] [--min 10] [--before YYYY-MM-DD]
             P's cards by date; the newest share (at least --min) held out, plus everything from
             --before on. Exit 3 when P has too few cards to measure.
  questions  --cards C --split S             the held-out cards as blind questions
  distil-in  --cards C --split S [--evidence FILE ...]
             P's training cards (with P's stance) and extra evidence, for a distilling pass
  score      --cards C --split S --predictions PRED [--json]
             hits per tier and category, a 95% interval, and two baselines
  similar    --cards C --person P --query TEXT [--split S] [--k 3] [--json]
  panel-check --answers A --cards C --question ID    seat answers well formed
  panel-sheet --answers A --cards C --question ID    the predicted meeting, per round
  panel-score --answers A --cards C --question ID [--json]   against the question's real card

Ground truth only: training reads cards and evidence whose source is in GROUND_TRUTH. A panel
seat's answer is source "simulated": a hypothesis, refused as evidence. A hypothesis counts only
once a real card confirms it, and then the card is the evidence.
"""
import argparse
import itertools
import json
import math
import re
import sys
from collections import defaultdict
from pathlib import Path

# Isolated Python omits the script directory; import only this sibling tool.
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_decisions import MIN_SHARED_WORDS, TIERS, share, words  # noqa: E402

GROUND_TRUTH = ("card", "pr-review", "principal-read", "principal-correction")
NO_SIDE = ("own", "asked-only")
OPEN_OUTCOMES = ("unresolved", "other")
LIVE_MIN_HIGH, LIVE_ACCURACY = 5, 0.9
Z95 = 1.96
DATE = re.compile(r"(\d{4})-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])")


class Failed(Exception):
    pass


def load_cards(path):
    data = json.loads(Path(path).read_text())
    if not isinstance(data, dict) or not isinstance(data.get("cards"), list) \
            or not all(isinstance(c, dict) for c in data["cards"]):
        raise Failed(f"{path}: expected an object with a cards list")
    return data["cards"]


def option_ids(c):
    return [o.get("id") for o in c.get("options") or [] if isinstance(o, dict)]


def option_text(c, oid):
    return next((o["text"] for o in c["options"] if o.get("id") == oid), oid)


def problems(c, categories, seen):
    cid = c.get("id")
    if not isinstance(cid, str) or not cid:
        yield "card without an id"
        return
    if cid in seen:
        yield "duplicate id"
    seen.add(cid)
    if c.get("source") not in GROUND_TRUTH:
        yield f"source {c.get('source')} is not ground truth"
    if not isinstance(c.get("date"), str) or not DATE.fullmatch(c["date"]):
        yield f"date {c.get('date')} is not YYYY-MM-DD"
    if categories is not None and c.get("category") not in categories:
        yield f"unknown category {c.get('category')}"
    for key in ("question", "context"):
        if not isinstance(c.get(key), str) or (key == "question" and not c[key].strip()):
            yield f"{key} must be text"
    options = c.get("options")
    ids = option_ids(c)
    if not isinstance(options, list) or not 2 <= len(options) <= 4 or len(ids) != len(options) \
            or len(set(ids)) != len(ids) or not all(isinstance(o.get("text"), str) and o["text"].strip() for o in options):
        yield "2 to 4 options, each with a unique id and text"
    stances = c.get("stances")
    if not isinstance(stances, dict) or not stances:
        yield "stances must name at least one person"
        stances = {}
    for person, stance in stances.items():
        if stance not in ids and stance not in NO_SIDE:
            yield f"{person} has no option {stance}"
    if c.get("outcome") not in ids and c.get("outcome") not in OPEN_OUTCOMES:
        yield f"outcome {c.get('outcome')} is not an option, unresolved or other"
    context = (c.get("context") or "").lower() if isinstance(c.get("context"), str) else ""
    for o in options if isinstance(options, list) else []:
        text = o.get("text") if isinstance(o, dict) else None
        if isinstance(text, str) and len(text.split()) >= 3 and text.lower() in context:
            yield f"context repeats option {o.get('id')}"
    for ask in c.get("asked") or []:
        if not isinstance(ask, dict) or ask.get("by") not in stances or not isinstance(ask.get("about"), str):
            yield f"{ask.get('by') if isinstance(ask, dict) else ask} asked but has no stance"
    for person in c.get("agent_drafted") or []:
        if person not in stances:
            yield f"{person} is agent_drafted but has no stance"


def cmd_check(args):
    cards = load_cards(args.cards)
    categories = json.loads(Path(args.categories).read_text()) if args.categories else None
    seen, found = set(), 0
    for c in cards:
        for problem in problems(c, categories, seen):
            print(f"{c.get('id')}: {problem}")
            found += 1
    print(f"{len(cards)} cards, {found} problems")
    return 1 if found else 0


def ground_truth(cards):
    for c in cards:
        if c.get("source") not in GROUND_TRUTH:
            raise Failed(f"{c.get('id')}: source {c.get('source')} is not ground truth")
    return cards


def takes_part(c, person):
    stance = (c.get("stances") or {}).get(person)
    return stance is not None and stance != "asked-only" and person not in (c.get("agent_drafted") or [])


def cmd_split(args):
    cards = ground_truth(load_cards(args.cards))
    if args.before and not DATE.fullmatch(args.before):
        raise Failed("--before must be YYYY-MM-DD")
    mine = sorted((c for c in cards if takes_part(c, args.person)), key=lambda c: (c["date"], c["id"]))
    if len(mine) < 2 * args.min:
        print(f"crew_personas: too few cards for {args.person}: {len(mine)} (need {2 * args.min})", file=sys.stderr)
        return 3
    held = max(math.ceil(args.holdout * len(mine)), args.min)
    cut = len(mine) - held
    heldout = {c["id"] for c in mine[cut:]} | {c["id"] for c in mine if args.before and c["date"] >= args.before}
    print(json.dumps({"person": args.person, "train": [c["id"] for c in mine if c["id"] not in heldout],
                      "heldout": [c["id"] for c in mine if c["id"] in heldout]}, indent=2))
    return 0


def by_split(args, part):
    cards = {c["id"]: c for c in ground_truth(load_cards(args.cards))}
    split = json.loads(Path(args.split).read_text())
    missing = [i for i in split[part] if i not in cards]
    if missing:
        raise Failed(f"split names cards not in {args.cards}: {', '.join(missing)}")
    return split["person"], [cards[i] for i in split[part]]


def cmd_questions(args):
    _, cards = by_split(args, "heldout")
    for c in cards:
        print(json.dumps({k: c[k] for k in ("id", "date", "category", "question", "context", "options")},
                         ensure_ascii=False))
    return 0


def cmd_distil_in(args):
    person, cards = by_split(args, "train")
    extra = []
    for path in args.evidence or []:
        for number, line in enumerate(Path(path).read_text().splitlines(), 1):
            if not line.strip():
                continue
            row = json.loads(line)
            if not isinstance(row, dict) or row.get("source") not in GROUND_TRUTH:
                source = row.get("source") if isinstance(row, dict) else None
                raise Failed(f"{Path(path).name} line {number}: source {source} is not ground truth")
            extra.append(row)
    for c in cards:
        stance = c["stances"][person]
        print(json.dumps(dict(c, person=person, stance=stance, stance_text=option_text(c, stance),
                              outcome_text=option_text(c, c["outcome"])), ensure_ascii=False))
    for row in extra:
        print(json.dumps(row, ensure_ascii=False))
    return 0


def wilson(hits, n):
    if not n:
        return [0.0, 0.0]
    p, z2 = hits / n, Z95 * Z95
    centre = (p + z2 / (2 * n)) / (1 + z2 / n)
    margin = Z95 * math.sqrt(p * (1 - p) / n + z2 / (4 * n * n)) / (1 + z2 / n)
    return [round(max(0.0, centre - margin), 4), round(min(1.0, centre + margin), 4)]


def cell(n, hits):
    return {"n": n, "hits": hits, "accuracy": share(hits, n)}


def score(person, cards, predictions):
    missing = [c["id"] for c in cards if c["id"] not in predictions]
    if missing:
        raise Failed(f"no prediction for {', '.join(missing)}")
    tiers, categories = defaultdict(lambda: [0, 0]), defaultdict(lambda: defaultdict(lambda: [0, 0]))
    hits = outcome_hits = 0
    uniform = 0.0
    for c in cards:
        p = predictions[c["id"]]
        if p.get("confidence") not in TIERS:
            raise Failed(f"{c['id']}: confidence must be one of {', '.join(TIERS)}")
        stance = c["stances"][person]
        hit = p.get("pick") == stance
        hits += hit
        outcome_hits += c["outcome"] == stance
        uniform += 1 / len(c["options"])
        for bucket in (tiers[p["confidence"]], categories[c["category"]][p["confidence"]]):
            bucket[0] += 1
            bucket[1] += hit
    n = len(cards)
    result = {"person": person,
              "overall": dict(cell(n, hits), interval=wilson(hits, n), uniform=round(uniform / n, 4) if n else 0.0,
                              sides_with_outcome=share(outcome_hits, n)),
              "tiers": {t: cell(*tiers[t]) for t in TIERS if t in tiers},
              "categories": {k: {t: cell(*v[t]) for t in TIERS if t in v} for k, v in sorted(categories.items())}}
    result["live"] = [k for k, v in result["categories"].items()
                      if v.get("high", {}).get("n", 0) >= LIVE_MIN_HIGH and v["high"]["accuracy"] >= LIVE_ACCURACY]
    return result


def score_markdown(result):
    o = result["overall"]
    low, high = o["interval"]
    lines = [f"{result['person']}. Overall: {o['hits']} of {o['n']} ({o['accuracy']:.0%}, 95% interval "
             f"{low * 100:.0f}–{high:.0%}); baselines: a random option {o['uniform']:.0%}, "
             f"sides with the outcome {o['sides_with_outcome']:.0%}",
             "", "| Tier | N | Hits | Accuracy |", "|---|---|---|---|"]
    lines += [f"| {t} | {c['n']} | {c['hits']} | {c['accuracy']:.0%} |" for t, c in result["tiers"].items()]
    lines += ["", "| Category | " + " | ".join(TIERS) + " | Live |", "|---|" + "---|" * (len(TIERS) + 1)]
    for name, tiers in result["categories"].items():
        cells = [f"{tiers[t]['hits']}/{tiers[t]['n']}" if t in tiers else "–" for t in TIERS]
        lines.append(f"| {name} | " + " | ".join(cells) + f" | {'yes' if name in result['live'] else 'no'} |")
    return "\n".join(lines)


def cmd_score(args):
    person, cards = by_split(args, "heldout")
    result = score(person, cards, json.loads(Path(args.predictions).read_text()))
    print(json.dumps(result, indent=2) if args.json else score_markdown(result))
    return 0


def searchable(c):
    return " ".join([c["question"], c["context"]] + [o["text"] for o in c["options"]])


def similar_cards(cards, person, query, k, train=None):
    """The person's cards nearest a query (shared rare words); only `train` ids when given."""
    cards = [c for c in ground_truth(cards) if takes_part(c, person) and (train is None or c["id"] in train)]
    docs = [(c, words(searchable(c))) for c in cards]
    df = defaultdict(int)
    for _, ws in docs:
        for w in ws:
            df[w] += 1
    wanted = words(query)
    scored = []
    for c, ws in docs:
        shared = wanted & ws
        if len(shared) >= MIN_SHARED_WORDS:
            scored.append((sum(math.log(len(docs) / df[w]) + 1 for w in shared) / math.sqrt(len(ws)), c["date"], c))
    scored.sort(key=lambda t: (-t[0], t[1]))
    return [{"id": c["id"], "date": c["date"], "category": c["category"], "question": c["question"],
             "argued": option_text(c, c["stances"][person]), "went": option_text(c, c["outcome"]),
             "link": c.get("link", "")} for _, _, c in scored[:k]]


def cmd_similar(args):
    train = set(json.loads(Path(args.split).read_text())["train"]) if args.split else None
    hits = similar_cards(load_cards(args.cards), args.person, args.query, args.k, train)
    if args.json:
        print(json.dumps(hits, indent=2, ensure_ascii=False))
    else:
        for h in hits:
            print(f"- {h['id']} · {h['date']} · {h['question']} · argued: {h['argued']} · went: {h['went']} · {h['link']}")
    return 0


def panel_inputs(args):
    cards = {c["id"]: c for c in load_cards(args.cards)}
    if args.question not in cards:
        raise Failed(f"question {args.question} is not a card in {args.cards}")
    answers = json.loads(Path(args.answers).read_text())
    if not isinstance(answers, list) or not all(isinstance(a, dict) for a in answers):
        raise Failed(f"{args.answers}: expected a list of seat answers")
    return cards, cards[args.question], answers


def answer_problems(a, cards, question):
    who = f"{a.get('seat')} round {a.get('round')}"
    if a.get("source") != "simulated":
        yield f"{who}: source must be simulated"
    if a.get("pick") not in option_ids(question) and a.get("pick") != "own":
        yield f"{who}: pick {a.get('pick')} is not an option or own"
    if a.get("confidence") not in TIERS:
        yield f"{who}: confidence {a.get('confidence')} is not one of {', '.join(TIERS)}"
    for reason in a.get("reasons") or []:
        for evidence in reason.get("evidence") or []:
            if evidence == question["id"]:
                yield f"{who}: evidence {evidence} is the question itself"
            elif evidence not in cards:
                yield f"{who}: evidence {evidence} is not a card"
    if not isinstance(a.get("asks"), list):
        yield f"{who}: asks must be a list"


def cmd_panel_check(args):
    cards, question, answers = panel_inputs(args)
    found = []
    rounds = defaultdict(list)
    for a in answers:
        rounds[a.get("round")].append(a.get("seat"))
        found += answer_problems(a, cards, question)
    seats = rounds.get(1, [])
    for number, present in sorted(rounds.items(), key=lambda kv: str(kv[0])):
        if number not in (1, 2):
            found.append(f"round {number} is not 1 or 2")
        for seat in seats:
            if seat not in present:
                found.append(f"round {number} is missing {seat}")
        if len(set(present)) != len(present):
            found.append(f"round {number} has a seat twice")
    for line in found:
        print(line)
    print(f"{len(answers)} answers, {len(found)} problems")
    return 1 if found else 0


def cmd_panel_sheet(args):
    _, question, answers = panel_inputs(args)
    print(f"Panel on {question['id']} ({question['date']}): {question['question']}")
    print("Every answer below is simulated: a prediction from each seat's model, never evidence.")
    for number in sorted({a["round"] for a in answers}):
        rows = [a for a in answers if a["round"] == number]
        print(f"\nRound {number}")
        for oid in option_ids(question) + ["own"]:
            seats = [a["seat"] for a in rows if a["pick"] == oid]
            if seats:
                print(f"- {oid}) {option_text(question, oid) if oid != 'own' else 'own words'}: {', '.join(seats)}")
        print("Likely questions:")
        for a in rows:
            for ask in a.get("asks") or []:
                print(f"- {a['seat']}: {ask}")
    return 0


def cmd_panel_score(args):
    _, question, answers = panel_inputs(args)
    gold = question["stances"]
    result = {}
    for number in sorted({a["round"] for a in answers}):
        picks = {a["seat"]: a["pick"] for a in answers if a["round"] == number}
        scored = [s for s in picks if gold.get(s) in option_ids(question) or gold.get(s) == "own"]
        hits = sum(picks[s] == gold[s] for s in scored)
        pairs = list(itertools.combinations(scored, 2))
        pair_hits = sum((picks[x] == picks[y]) == (gold[x] == gold[y]) for x, y in pairs)
        result[str(number)] = {"stances": cell(len(scored), hits), "pairs": cell(len(pairs), pair_hits),
                               "unscored": sorted(s for s in picks if s not in scored)}
    result["asked"] = question.get("asked") or []
    if args.json:
        print(json.dumps(result, indent=2, ensure_ascii=False))
        return 0
    for number, r in result.items():
        if number == "asked":
            continue
        print(f"Round {number}: stances {r['stances']['hits']}/{r['stances']['n']}, "
              f"agreement pairs {r['pairs']['hits']}/{r['pairs']['n']}; unscored: {', '.join(r['unscored']) or 'none'}")
    print("Questions really asked (score the seats' asks against these by hand or with a judge):")
    for ask in result["asked"]:
        print(f"- {ask['by']}: {ask['about']}")
    return 0


def parser():
    p = argparse.ArgumentParser(prog="crew_personas", description=__doc__.split("\n\n")[0])
    sub = p.add_subparsers(dest="command", required=True)
    c = sub.add_parser("check")
    c.add_argument("--cards", required=True)
    c.add_argument("--categories")
    c = sub.add_parser("split")
    c.add_argument("--cards", required=True)
    c.add_argument("--person", required=True)
    c.add_argument("--holdout", type=float, default=0.25)
    c.add_argument("--min", type=int, default=10)
    c.add_argument("--before")
    for name in ("questions", "distil-in", "score"):
        c = sub.add_parser(name)
        c.add_argument("--cards", required=True)
        c.add_argument("--split", required=True)
        if name == "distil-in":
            c.add_argument("--evidence", action="append")
        if name == "score":
            c.add_argument("--predictions", required=True)
            c.add_argument("--json", action="store_true")
    c = sub.add_parser("similar")
    c.add_argument("--cards", required=True)
    c.add_argument("--person", required=True)
    c.add_argument("--query", required=True)
    c.add_argument("--split")
    c.add_argument("--k", type=int, default=3)
    c.add_argument("--json", action="store_true")
    for name in ("panel-check", "panel-sheet", "panel-score"):
        c = sub.add_parser(name)
        c.add_argument("--answers", required=True)
        c.add_argument("--cards", required=True)
        c.add_argument("--question", required=True)
        if name == "panel-score":
            c.add_argument("--json", action="store_true")
    return p


COMMANDS = {"check": cmd_check, "split": cmd_split, "questions": cmd_questions, "distil-in": cmd_distil_in,
            "score": cmd_score, "similar": cmd_similar, "panel-check": cmd_panel_check,
            "panel-sheet": cmd_panel_sheet, "panel-score": cmd_panel_score}


def main(argv):
    args = parser().parse_args(argv)
    if getattr(args, "holdout", 0.25) is not None and not 0 < getattr(args, "holdout", 0.25) < 1:
        print("crew_personas: --holdout must be between 0 and 1", file=sys.stderr)
        return 2
    try:
        return COMMANDS[args.command](args)
    except (Failed, ValueError, KeyError, OSError) as error:
        print("crew_personas: " + " ".join(str(error).splitlines()), file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
