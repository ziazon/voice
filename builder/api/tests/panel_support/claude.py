"""Return the same harmless proposal for each invented assistant."""
import json
import re
import sys

args = sys.argv[1:]
if args[:1] == ["--version"]:
    print("2.1.292 (Claude Code)")
elif args[:1] == ["auth"]:
    print(json.dumps({"loggedIn": True}))
else:
    prompt = sys.stdin.read()
    found = re.match(r"Round (\d+)\.", prompt)
    round_no = int(found.group(1)) if found else 1
    position = "propose" if round_no == 1 else "agree"
    post = 'Try a short trial.\n```deliberation\nposition: ' + json.dumps(position) + '\nitems: ["A short trial"]\nobjections: ""\n```'
    print(json.dumps({"type": "result", "subtype": "success", "is_error": False,
                      "result": "", "total_cost_usd": 0.01,
                      "usage": {"input_tokens": 80, "output_tokens": 40},
                      "structured_output": {"post": post}}))
