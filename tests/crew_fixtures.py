"""Fictional transcript records shared by the crew CLI tests."""

HUMAN = {"kind": "human"}


def ask(when, question, options, answer, recommended=0, notes=None, multi=False, header="Scope", descriptions=None,
        member_pick=None):
    """One AskUserQuestion tool result, the shape Claude Code records."""
    labels = [f"{label} (Recommended · Ada)" if i == recommended and i == member_pick else
              f"{label} (Recommended)" if i == recommended else
              f"{label} (Ada)" if i == member_pick else label for i, label in enumerate(options)]
    if descriptions is None:
        descriptions = [f"about {label}" for label in labels]
    result = {"questions": [{"question": question, "header": header, "multiSelect": multi,
                             "options": [{"label": l, "description": d} for l, d in zip(labels, descriptions)]}],
              "answers": {question: answer}}
    if notes:
        result["annotations"] = {question: {"notes": notes}}
    return {"type": "user", "timestamp": when, "cwd": "/repo", "gitBranch": "main", "userType": "external",
            "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": "ok"}]},
            "toolUseResult": result}


def said(when, text, origin=HUMAN, **extra):
    record = {"type": "user", "timestamp": when, "cwd": "/repo", "gitBranch": "main", "userType": "external",
              "message": {"role": "user", "content": text}}
    if origin is not None:
        record["origin"] = origin
    record.update(extra)
    return record


def reply(when, text):
    return {"type": "assistant", "timestamp": when,
            "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}}


def assert_cli_error(test, stderr, prefix):
    """Allow interpreter diagnostics, but require one tool error and no script frames."""
    lines = stderr.splitlines()
    test.assertEqual(sum(line.startswith(prefix) for line in lines), 1, stderr)
    test.assertNotRegex(stderr, r'scripts/[^"\n]+\.py", line')
