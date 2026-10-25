"""A stub `claude` for the build's tests: it answers each step from the records in its prompt.

The test copies it into a folder of its own, with a `#!` line naming the Python to run. Each call
keeps its argv (a JSON list) and stdin there (`argv<N>`, `stdin<N>`); `plan.json` there can make call N answer
`bad` (a reply that fails the build's check), `quote` (a distilled point that repeats the record's
own words) or `error` (a failed run).
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
ARGS = sys.argv[1:]


def main():
    if ARGS[:1] == ["--version"]:
        print("2.1.292 (Claude Code)")
        return 0
    if ARGS[:1] == ["auth"]:
        print(json.dumps({"loggedIn": True}))
        return 0
    n = len([f for f in os.listdir(HERE) if f.startswith("stdin")])
    prompt = sys.stdin.read()
    with open(os.path.join(HERE, "stdin%d" % n), "w") as f:
        f.write(prompt)
    with open(os.path.join(HERE, "argv%d" % n), "w") as f:
        json.dump(ARGS, f)
    plan_file = os.path.join(HERE, "plan.json")
    plan = json.load(open(plan_file)) if os.path.exists(plan_file) else {}
    how = plan.get(str(n), "good")
    if how == "error":
        return 1
    first = prompt.split("\n", 1)[0]
    if first.startswith("Label"):
        out = label(prompt, how)
    elif first.startswith("Write how"):
        out = distil(prompt, how)
    else:
        out = backtest(prompt, how)
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False, "result": "",
                      "total_cost_usd": 0.02, "usage": {"input_tokens": 900, "output_tokens": 200},
                      "structured_output": out}))
    return 0


def first_listed(prompt, key):
    line = [l for l in prompt.splitlines() if l.startswith("- %s: " % key)][0]
    return line.split(": ", 1)[1].split(", ")[0]


def label(prompt, how):
    train = "\nMode: train\n" in prompt
    domain, signal = first_listed(prompt, "domain"), first_listed(prompt, "signal")
    lines = prompt.split("one JSON object per line:\n\n", 1)[1].splitlines()
    labels = []
    for line in lines:
        if not line.startswith("{"):
            continue
        r = json.loads(line)
        question = r["kind"] == "question"
        labels.append({
            "id": r["id"],
            "category": r["header"].lower() if question else "tooling",
            "domain": domain,
            "escalates": [],
            "signal": None if question else signal,
            "reason": "It kept the work small and easy to undo." if train else None,
        })
    if how == "bad":
        labels[0]["category"] = "zebracorn"
    return {"labels": labels}


def distil(prompt, how):
    named = prompt.split("The kinds of decision:\n", 1)[1].split("\n\n", 1)[0]
    categories = [line[2:].split(":")[0] for line in named.splitlines()]
    sections = [{
        "category": c,
        "lean": ["They pick the smaller, reversible choice in %s." % c],
        "flips": [],
        "own_answer": [],
        "rules": [],
        "precedents": ["2026-09-01 · kit · chose the smaller option"],
    } for c in categories]
    if how == "bad":
        sections = sections[1:]
    if how == "quote":
        record = json.loads(prompt.split("one JSON object per line:\n\n", 1)[1].splitlines()[0])
        sections[0]["lean"] = [record.get("q") or record.get("msg")]
    return {"sections": sections}


def backtest(prompt, how):
    questions = [json.loads(line) for line in prompt.splitlines() if line.startswith("{")]
    predictions = [{"id": q["id"], "pick": q["options"][0]["label"], "confidence": "high"}
                   for q in questions]
    if how == "bad":
        predictions = predictions[1:]
    return {"predictions": predictions}


sys.exit(main())
