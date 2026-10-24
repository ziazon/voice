"""Invented claude.ai exports for the real-Python/Rust parity checks of `claudeai`.

`python3 -I tests/crew_claudeai_fixtures.py <case> <dir>` writes one case into <dir> and prints what
to run on it as one JSON object: the export's name, a hand-written review file (or null), the extra
`titles` arguments (null: only `inspect` runs), the `mark` argument lists in order, the `mine`
argument lists, the member (`ada` or `ari`) and an export `inspect` must refuse (or null). Every
name, word and id is invented.
"""
import json
import sys
import zipfile
from pathlib import Path

HEADER = "mine\tcreated\tupdated\tmessages\tproject\ttitle\tuuid"
MINE_RUNS = [[], ["--holdout", "0"], ["--holdout", "1"], ["--holdout", "0.5"], ["--cutoff", "2026-03-02"],
             ["--cutoff", "9999"], ["--cutoff", ""]]


def message(sender, when, text, blocks=True):
    m = {"uuid": f"m-{when}", "sender": sender, "created_at": when, "updated_at": when, "text": text,
         "attachments": [], "files": []}
    if blocks:
        m["content"] = [{"type": "text", "text": text}]
    return m


def conversation(uuid, name, when, *messages, **extra):
    return {"uuid": uuid, "name": name, "created_at": when, "updated_at": when,
            "account": {"uuid": "acct"}, "chat_messages": list(messages), **extra}


BASIC = [
    conversation("c-grant", "a funder aims\tdraft", "2026-03-01T12:00:00.000000Z",
                 message("human", "2026-03-01T12:00:00.000000Z", "tighten aim 2 around the community partner need"),
                 message("assistant", "2026-03-01T12:00:05.000000Z", "Here is a tighter aim 2 focused on partners."),
                 message("human", "2026-03-01T12:01:00.000000Z", "keep the partner sentence first, cut the rest"),
                 project={"name": "Grants"}),
    conversation("c-sam", "Acme deploy plan", "2026-03-02T09:00:00+00:00",
                 message("human", "2026-03-02T09:00:00+00:00", "plan the staging release"), project={"name": "Ops"}),
    conversation("c-doctor", "Appointment", "2026-03-03T09:00:00Z",
                 message("human", "2026-03-03T09:00:00Z", "my doctor wants to move the follow-up")),
    conversation("c-unmarked", "Syllabus week 3", "2026-03-04T09:00:00Z",
                 message("human", "2026-03-04T09:00:00Z", "swap the week 3 readings")),
    conversation("c-blocks", "Course tracker", "2026-03-05T09:00:00Z",
                 message("human", "2026-03-05T09:00:00Z", "", blocks=False) | {
                     "content": [{"type": "text", "text": "grade levels stay at four"}]},
                 message("human", "2026-03-05T09:01:00Z", "email someone@example.edu about it")),
]
BASIC_MARKS = [["--project", "Grants", "--as", "y"], ["--title", "(?i)acme", "--as", "n"],
               ["--uuid", "c-doctor", "--uuid", "c-blocks", "--as", "y"],
               ["--after", "2026-03-04", "--before", "2026-03-04", "--as", "n"],
               ["--unmarked", "--as", "n"], ["--project", "", "--all", "--as", "y"],
               ["--uuid", "c-sam", "--as", "n"]]

UNTITLED = [
    conversation("c-named", "Grant aims", "2026-03-01T09:00:00Z",
                 message("human", "2026-03-01T09:00:00Z", "my doctor said rest")),
    conversation("c-short", "", "2026-03-02T09:00:00Z",
                 message("human", "2026-03-02T09:00:00Z", "swap the week 3 readings")),
    conversation("c-long", "  ", "2026-03-03T09:00:00Z",
                 message("human", "2026-03-03T09:00:00Z",
                         "can you help me tighten the abstract for the renewal please")),
    conversation("c-eight", "", "2026-03-03T10:00:00Z",
                 message("human", "2026-03-03T10:00:00Z", "one two three four five six seven eight")),
    conversation("c-personal", "", "2026-03-04T09:00:00Z",
                 message("human", "2026-03-04T09:00:00Z",
                         "please move the grant meeting to friday because my doctor said rest")),
    conversation("c-secret", "", "2026-03-05T09:00:00Z",
                 message("human", "2026-03-05T09:00:00Z", "write to someone@example.edu today")),
    conversation("c-second", "", "2026-03-06T09:00:00Z",
                 message("assistant", "2026-03-06T09:00:00Z", "Hello there"),
                 message("human", "2026-03-06T09:00:01Z", "", blocks=False),
                 message("human", "2026-03-06T09:00:02Z", "plan the\tstaging\nrelease")),
    conversation("c-silent", None, "2026-03-07T09:00:00Z",
                 message("assistant", "2026-03-07T09:00:00Z", "Hello there")),
    conversation("c-false", False, "2026-03-07T10:00:00Z"),
    conversation("c-link", "", "2026-03-08T09:00:00Z",
                 message("human", "2026-03-08T09:00:00Z",
                         "edit https://docs.example.org/document/d/1AbCdEfGhIjKlMnOpQrStUvWxYz/edit now")),
    conversation("c-exact", "", "2026-03-09T09:00:00Z",
                 message("human", "2026-03-09T09:00:00Z", "abcdefghijklmnopqrstuvwx abcdefghijklmnopqrstuvwxy")),
]

TIMESTAMPS = [
    conversation("t-b", "Same time b", "2026-03-01T12:00:00Z",
                 message("human", "2026-03-01T12:00:00.123456789Z", "nine digits of fraction")),
    conversation("t-a", "Same time a", "2026-03-01 12:00:00",
                 message("human", "2026-03-01T12:00:00.9999999Z", "seven nines truncate")),
    conversation("t-midnight", "Crosses midnight", "2026-03-01T23:30:00-01:00",
                 message("human", "2026-03-01T12:00:00-23:59", "a far western offset"),
                 message("human", "2026-03-02T00:30:00+01:00", "back across midnight")),
    conversation("t-year", "Crosses the year", "2025-12-31T23:30:00-01:00",
                 message("human", "2026-01-01T00:30:00+01:00", "back into last year"),
                 message("human", "2026-01-01T00:30:00.5+01:00", "half a second"),
                 message("human", "2026-01-01T00:30:00.12+01:00", "two digits"),
                 message("human", "2026-01-01T00:30:00.123+00:00", "zero offset")),
    conversation("t-short", "Short forms", "2026-03-03",
                 message("human", "2026-03-03", "a date alone"),
                 message("human", "2026-03-03T08:15", "minutes only"),
                 message("human", "2026-03-03T08:15Z", "minutes and a zone"),
                 message("human", "2026-03-03T08:15-00:00", "a negative zero zone"),
                 message("human", "2024-02-29T00:00:00", "a leap day")),
    conversation("t-updated", "Falsy update", "2026-03-04T09:00:00Z",
                 message("human", "2026-03-04T09:00:00Z", "the updated date is blank"), updated_at=""),
    conversation("t-null", "Null update", "2026-03-05T09:00:00Z",
                 message("human", "2026-03-05T09:00:00Z", "the updated date is null"), updated_at=None),
    conversation("t-later", "Updated later", "2026-03-06T09:00:00Z",
                 message("human", "2026-03-06T09:00:00Z", "updated a day on")) | {"updated_at": "2026-03-07T01:00:00+02:00"},
]


def blocks_case():
    def bare(**fields):
        return {"sender": "human", "created_at": "2026-03-01T10:00:00Z", **fields}
    return [conversation(
        "b-all", "Blocks", "2026-03-01T10:00:00Z",
        bare(text="  \n", content=[{"type": "text", "text": "blank text uses the blocks"}]),
        bare(text=7, content="a numeric text field is ignored"),
        bare(content={"type": "text", "text": "an object content gives nothing"}),
        bare(content=None, text="null content, plain text"),
        bare(text="", content=[{"type": "text"}, {"type": "image", "text": "not text"},
                               {"type": "text", "text": "after a block with no text"}, "loose string"]),
        bare(text=""),
        bare(text=None, content=[{"type": "text", "text": "first"}, {"type": "text", "text": "second"}]),
        {"sender": "assistant", "text": "", "content": [{"type": "text", "text": "a reply from blocks"}]},
        bare(text="after the block reply"),
    )]


def messages_case():
    long_reply = "R" * 250 + " " + "S" * 100
    return [conversation(
        "m-all", "Messages", "2026-03-01T10:00:00Z",
        message("human", "2026-03-01T10:00:00Z", "Pasted: a pasted brief"),
        message("human", "2026-03-01T10:00:01Z", "You are a pasted prompt"),
        message("human", "2026-03-01T10:00:02Z", "Continue"),
        message("human", "2026-03-01T10:00:03Z", "  retry  "),
        message("human", "2026-03-01T10:00:04Z", "x" * 4000),
        message("human", "2026-03-01T10:00:05Z", "y" * 4001),
        message("human", "2026-03-01T10:00:06Z", "my doctor moved it"),
        message("human", "2026-03-01T10:00:07Z", "call 5551234567 later"),
        message("human", "2026-03-01T10:00:08Z", "my therapist agrees"),
        message("assistant", "2026-03-01T10:00:09Z", long_reply),
        message("human", "2026-03-01T10:00:10Z", "after a long reply"),
        message("assistant", "2026-03-01T10:00:11Z", "   "),
        message("human", "2026-03-01T10:00:12Z", "the blank reply keeps the context"),
        message("system", "2026-03-01T10:00:13Z", "a system note is skipped"),
        message("human", "not a date", "Continue"),
        {"sender": "human", "created_at": "2026-03-01T10:00:14Z", "text": "  spaced out  \n"},
    )]


def marks_case(root):
    export = [conversation(f"k-{i}", f"Chat {i}", f"2026-03-0{i}T09:00:00Z",
                           message("human", f"2026-03-0{i}T09:00:00Z", f"decision number {i}"),
                           project={"name": "Kit"})
              for i in range(1, 9)]
    rows = [("y", 1), (" Yes ", 2), ("MINE", 3), ("n", 4), ("no", 5), ("maybe", 6), ("", 7)]
    lines = [HEADER] + [f"{mark}\t2026-03-0{i}\t2026-03-0{i}\t1\tKit\tChat {i}\tk-{i}" for mark, i in rows]
    lines += ["y\t2026-02-01\t2026-02-01\t1\t\tGone\tk-gone", "", "   ",
              "y\t2026-03-01\t2026-03-01\t1\tKit\tChat 1\tk-1\textra\tmore"]
    (root / "review.tsv").write_bytes(("\r\n".join(lines) + "\r\n").encode())
    return export


UNICODE = [
    conversation("u-1", "é日😀 tab\there\nnew\x1cline\x85end", "2026-03-01T09:00:00Z",
                 message("human", "2026-03-01T09:00:00Z", "keep é, 日 and 😀  in the text\x85"),
                 project={"name": "  Kit one  "}),
    conversation("u-2", "", "2026-03-02T09:00:00Z",
                 message("human", "2026-03-02T09:00:00Z", "　日本語 の\x1cことば  それと  もっと  たくさん  の  ことば  です  ね")),
]

SHAPES = [
    {"uuid": "s-1", "name": "Shapes", "created_at": "2026-03-01T09:00:00Z", "a.b": 1, "a": {"b": "x", "c": [1, 2.5]},
     "lists": [[1, 2], [{"inner": True}], {"not": "in a list"}], "nums": [0, -3, 1.0, 1e5, None, False],
     "chat_messages": [{"sender": "human", "created_at": "2026-03-01T09:00:00Z", "text": "hi"},
                       {"sender": "bot", "created_at": "2026-03-01T09:00:01Z", "content": [{"type": "text", "text": "x"}]},
                       {"sender": 3, "created_at": "2026-03-01T09:00:02Z"}, "loose"]},
    {"uuid": "s-2", "name": "Shapes two", "created_at": "2026-03-02T09:00:00Z", "chat_messages": [], "extra": {}},
    "not an object",
]


def write_zip(path, conversations, others=("users.json", "projects.json"), inner="conversations.json", folders=()):
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for folder in folders:
            z.writestr(folder, "")
        z.writestr(inner, json.dumps(conversations))
        for name in others:
            z.writestr(name, json.dumps([{"uuid": "u"}]))


def write_case(case, root):
    root.mkdir(parents=True, exist_ok=True)
    spec = {"export": "export.zip", "review": None, "titles": [], "marks": [], "mine": MINE_RUNS,
            "member": "ari", "broken": None}
    conversations = BASIC
    if case == "basic":
        spec["titles"] = ["--project-field", "project.name"]
        spec["marks"] = BASIC_MARKS
    elif case == "folder":
        (root / "export").mkdir()
        (root / "export" / "conversations.json").write_text(json.dumps(BASIC))
        (root / "export" / "users.json").write_text("[]")
        (root / "export" / "sub").mkdir()
        spec.update(export="export", marks=BASIC_MARKS[:3])
    elif case == "plain":
        (root / "conversations.json").write_text(json.dumps(BASIC))
        spec.update(export="conversations.json", marks=BASIC_MARKS[:3], member="ada")
    elif case == "nested_zip":
        write_zip(root / "export.zip", BASIC, others=("data-2026/users.json", "notes/readme.txt"),
                  inner="data-2026/conversations.json", folders=("data-2026/", "notes/"))
        spec["marks"] = [["--all", "--as", "y"]]
    elif case == "untitled":
        conversations = UNTITLED
        spec["marks"] = [["--all", "--as", "y"]]
    elif case == "timestamps":
        conversations = TIMESTAMPS
        spec["marks"] = [["--all", "--as", "y"], ["--before", "2026-03-01", "--as", "n"]]
    elif case == "blocks":
        conversations = blocks_case()
        spec["marks"] = [["--all", "--as", "y"]]
    elif case == "messages":
        conversations = messages_case()
        spec["marks"] = [["--all", "--as", "y"]]
    elif case == "messages_ada":
        conversations = messages_case()
        spec.update(marks=[["--all", "--as", "y"]], member="ada")
    elif case == "marks":
        conversations = marks_case(root)
        spec.update(review="review.tsv", titles=["--project-field", "project.name"],
                    marks=[["--unmarked", "--as", "y"], ["--title", "Chat [45]", "--as", "y"]])
    elif case == "unicode":
        conversations = UNICODE
        spec.update(titles=["--project-field", "project.name"], marks=[["--all", "--as", "y"]])
    elif case == "inspect_shapes":
        conversations = SHAPES
        broken = [{k: v for k, v in c.items() if k != "created_at"} for c in BASIC]
        for c in broken:
            c["chat_messages"] = [{k: v for k, v in m.items() if k not in ("sender", "text", "content")}
                                  for m in c["chat_messages"]]
        write_zip(root / "broken.zip", broken)
        spec.update(broken="broken.zip", titles=None, marks=[], mine=[])
    else:
        raise ValueError(case)
    if spec["export"] == "export.zip" and not (root / "export.zip").exists():
        write_zip(root / "export.zip", conversations)
    return spec


if __name__ == "__main__":
    print(json.dumps(write_case(sys.argv[1], Path(sys.argv[2]))))
