"""Tool-free Claude Code stub; plan.json controls each numbered response."""
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
    n = len([name for name in os.listdir(HERE) if name.startswith("stdin")])
    prompt = sys.stdin.read()
    with open(os.path.join(HERE, "stdin%d" % n), "w") as stream:
        stream.write(prompt)
    with open(os.path.join(HERE, "argv%d" % n), "w") as stream:
        json.dump(ARGS, stream)
    plan_path = os.path.join(HERE, "plan.json")
    plan = json.load(open(plan_path)) if os.path.exists(plan_path) else {}
    mode = plan.get(str(n), "good")
    if mode == "error":
        return 1
    cases = []
    for line in prompt.splitlines():
        if not line.startswith("{"):
            continue
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if all(key in row for key in ["id", "area", "wanted"]):
            cases.append(row)
    edits = [{"area": row["area"], "kind": "add", "old": None,
              "text": "In %s, begin with a reversible step, lesson %d." % (row["area"], i + 1),
              "why": "A smaller first step leaves room to adjust.", "cases": [row["id"]]}
             for i, row in enumerate(cases[:10])]
    if mode == "bad":
        edits[0]["area"] = "zebracorn"
    if mode == "quote":
        edits[0]["text"] = " ".join(cases[0]["wanted"].split()[:6])
    if mode == "private":
        edits[0]["text"] = "Ask at invented@example.invalid before deciding."
    if mode == "hidden":
        edits[0]["text"] += "\u200b"
    if mode == "move":
        with open(plan["decisions_path"], "a") as stream:
            stream.write("\nChanged during the call.\n")
    out = {"summary": "Start with smaller steps and leave room to adjust.", "edits": edits}
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False,
                      "result": "", "total_cost_usd": 0.02,
                      "usage": {"input_tokens": 900, "output_tokens": 200},
                      "structured_output": out}))
    return 0


sys.exit(main())
