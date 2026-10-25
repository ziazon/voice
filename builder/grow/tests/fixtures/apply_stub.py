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
    rows = []
    for line in prompt.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict) and "id" in row:
            rows.append(row)
    if prompt.startswith("Answer each test"):
        marker = plan.get("right_when", "ADJUST")
        decisions = prompt.split("The tests, one JSON object per line:")[0]
        answers = []
        for row in rows:
            options = row.get("options")
            answer = "Offer a smaller first step."
            if options:
                answer = next((o for o in options if marker in o), options[0]) if marker in decisions else options[0]
            answers.append({"id": row["id"], "answer": answer})
        if mode == "bad":
            answers[0]["answer"] = "not an offered option"
        if mode == "missing":
            answers.pop()
        if mode == "hidden":
            next(a for a, r in zip(answers, rows) if "options" not in r)["answer"] = "private sentinel\u200b"
        out = {"answers": answers}
    else:
        out = {"grades": [{"id": r["id"], "grade": "pass" if "smaller" in r["answer"] else "fail"} for r in rows]}
    if mode == "move":
        with open(plan["decisions_path"], "a") as stream:
            stream.write("\nChanged during the call.\n")
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False,
                      "result": "", "total_cost_usd": 0.02,
                      "usage": {"input_tokens": 900, "output_tokens": 200},
                      "structured_output": out}))
    return 0


sys.exit(main())
