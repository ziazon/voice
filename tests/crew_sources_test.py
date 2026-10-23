#!/usr/bin/env python3
"""Pin the extra sources and corpus steps of crew_decisions: the claude.ai export adapter with its
mine / not-mine review, the merge of per-machine corpora, and the folder inventory.

The export fixtures follow the shape community documentation describes (conversations.json: a
list of conversations with uuid, name, created_at, updated_at, chat_messages; each message with
sender, text, created_at, content). `claudeai inspect` exists because nobody has confirmed that
shape from Anthropic: it is the gate a real export must pass first.
"""
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_decisions_test import ARI, Harness  # noqa: E402
from crew_fixtures import assert_cli_error, ask, said  # noqa: E402


def message(sender, when, text, blocks=True):
    m = {"uuid": f"m-{when}", "sender": sender, "created_at": when, "updated_at": when, "text": text,
         "attachments": [], "files": []}
    if blocks:
        m["content"] = [{"type": "text", "text": text}]
    return m


def conversation(uuid, name, when, *messages, **extra):
    return {"uuid": uuid, "name": name, "created_at": when, "updated_at": when,
            "account": {"uuid": "acct"}, "chat_messages": list(messages), **extra}


EXPORT = [
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


class Export:
    def __init__(self, h, conversations=EXPORT, as_zip=True):
        self.path = h.root / ("data-export.zip" if as_zip else "export")
        if as_zip:
            with zipfile.ZipFile(self.path, "w") as z:
                z.writestr("conversations.json", json.dumps(conversations))
                z.writestr("users.json", json.dumps([{"uuid": "u"}]))
                z.writestr("projects.json", json.dumps([]))
        else:
            self.path.mkdir()
            (self.path / "conversations.json").write_text(json.dumps(conversations))


def review_rows(h, name="review.tsv"):
    lines = h.read(name).splitlines()
    header = lines[0].split("\t")
    return header, [dict(zip(header, line.split("\t"))) for line in lines[1:]]


class Inspect(unittest.TestCase):
    def test_prints_field_names_and_types_but_no_values(self):
        h = Harness()
        member = h.member(ARI)
        export = Export(h)
        proc = h.crew(member, "claudeai", "inspect", "--export", str(export.path))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        out = proc.stdout
        for line in ("conversations.json: list of 5", ".uuid: str (5/5)", ".chat_messages: list (5/5)",
                     ".chat_messages[].sender: str (8/8) values: assistant, human",
                     ".chat_messages[].content[].type: str", "files: 3"):
            self.assertIn(line, out)
        self.assertIn("ok: every field the adapter reads is present", out)
        for value in ("a funder", "partner", "Acme", "doctor", "c-grant", "2026-03"):
            self.assertNotIn(value, out)

    def test_a_missing_field_fails_and_names_it(self):
        h = Harness()
        member = h.member(ARI)
        broken = [{k: v for k, v in c.items() if k != "created_at"} for c in EXPORT]
        for c in broken:
            c["chat_messages"] = [{k: v for k, v in m.items() if k != "sender"} for m in c["chat_messages"]]
        export = Export(h, broken, as_zip=False)
        proc = h.crew(member, "claudeai", "inspect", "--export", str(export.path))
        self.assertEqual(proc.returncode, 1)
        self.assertIn("missing: .created_at, .chat_messages[].sender", proc.stdout + proc.stderr)


class Review(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.member = self.h.member(ARI)
        self.export = Export(self.h)

    def crew(self, *args):
        proc = self.h.crew(self.member, "claudeai", *args)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc

    def test_titles_lists_every_conversation_unmarked_with_no_content(self):
        self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv", "--project-field", "project.name")
        header, rows = review_rows(self.h)
        self.assertEqual(header, ["mine", "created", "updated", "messages", "project", "title", "uuid"])
        self.assertEqual([r["uuid"] for r in rows], ["c-grant", "c-sam", "c-doctor", "c-unmarked", "c-blocks"])
        self.assertEqual(rows[0], {"mine": "", "created": "2026-03-01", "updated": "2026-03-01", "messages": "2",
                                   "project": "Grants", "title": "a funder aims draft", "uuid": "c-grant"})
        self.assertEqual(rows[2]["project"], "")
        self.assertNotIn("partner", self.h.read("review.tsv"))

    def test_an_untitled_conversation_shows_its_opening_words_unless_a_filter_trips(self):
        untitled = [
            conversation("c-named", "Grant aims", "2026-03-01T09:00:00Z",
                         message("human", "2026-03-01T09:00:00Z", "my doctor said rest")),
            conversation("c-short", "", "2026-03-02T09:00:00Z",
                         message("human", "2026-03-02T09:00:00Z", "swap the week 3 readings")),
            conversation("c-long", "  ", "2026-03-03T09:00:00Z",
                         message("human", "2026-03-03T09:00:00Z",
                                 "can you help me tighten the abstract for the renewal please")),
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
            conversation("c-link", "", "2026-03-08T09:00:00Z",
                         message("human", "2026-03-08T09:00:00Z",
                                 "edit https://docs.google.com/document/d/1AbCdEfGhIjKlMnOpQrStUvWxYz/edit now")),
        ]
        export = Export(self.h, untitled)
        proc = self.crew("titles", "--export", str(export.path), "--out", "review.tsv")
        _, rows = review_rows(self.h)
        self.assertEqual({r["uuid"]: r["title"] for r in rows},
                         {"c-named": "Grant aims", "c-short": "opens: swap the week 3 readings",
                          "c-long": "opens: can you help me tighten the abstract for…",
                          "c-personal": "[filtered]", "c-secret": "[filtered]",
                          "c-second": "opens: plan the staging release", "c-silent": "",
                          "c-link": "opens: edit … now"})
        self.assertIn("listed 8 conversations in review.tsv (0 already marked; "
                      "7 untitled: 4 by opening words, 2 filtered)", proc.stdout)
        for value in ("doctor", "example.edu", "renewal", "AbCd"):
            self.assertNotIn(value, self.h.read("review.tsv"))

    def test_titled_exports_keep_the_short_summary(self):
        proc = self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv")
        self.assertIn("listed 5 conversations in review.tsv (0 already marked)\n", proc.stdout)

    def test_without_a_project_field_the_column_is_empty(self):
        self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv")
        _, rows = review_rows(self.h)
        self.assertEqual({r["project"] for r in rows}, {""})

    def test_marks_by_project_title_date_and_uuid_and_rerun_keeps_marks(self):
        self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv", "--project-field", "project.name")
        self.assertIn("marked 1", self.crew("mark", "--review", "review.tsv", "--project", "Grants", "--as", "y").stdout)
        self.crew("mark", "--review", "review.tsv", "--title", "(?i)acme", "--as", "n")
        self.crew("mark", "--review", "review.tsv", "--uuid", "c-doctor", "--uuid", "c-blocks", "--as", "y")
        self.crew("mark", "--review", "review.tsv", "--after", "2026-03-04", "--before", "2026-03-04", "--as", "n")
        self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv", "--project-field", "project.name")
        _, rows = review_rows(self.h)
        self.assertEqual({r["uuid"]: r["mine"] for r in rows},
                         {"c-grant": "y", "c-sam": "n", "c-doctor": "y", "c-unmarked": "n", "c-blocks": "y"})
        self.assertIn("marked 0", self.crew("mark", "--review", "review.tsv", "--unmarked", "--as", "n").stdout)

    def test_bad_input_is_a_plain_error_not_a_traceback(self):
        self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv")
        proc = self.h.crew(self.member, "claudeai", "mark", "--review", "review.tsv", "--title", "(", "--as", "y")
        self.assertEqual(proc.returncode, 1)
        assert_cli_error(self, proc.stderr, "crew_decisions:")
        self.assertIn("--title", proc.stderr)
        bad = [dict(EXPORT[0], created_at=None)]
        (self.h.root / "bad").mkdir()
        (self.h.root / "bad" / "conversations.json").write_text(json.dumps(bad))
        proc = self.h.crew(self.member, "claudeai", "titles", "--export", str(self.h.root / "bad"), "--out", "r2.tsv")
        self.assertEqual(proc.returncode, 1)
        assert_cli_error(self, proc.stderr, "crew_decisions:")
        self.assertIn("not a timestamp", proc.stderr)
        self.assertFalse((self.h.root / "r2.tsv").exists())

    def test_mark_needs_a_selector(self):
        self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv")
        proc = self.h.crew(self.member, "claudeai", "mark", "--review", "review.tsv", "--as", "y")
        self.assertEqual(proc.returncode, 1)

    def test_mine_reads_only_conversations_marked_mine_and_filters_them(self):
        self.crew("titles", "--export", str(self.export.path), "--out", "review.tsv", "--project-field", "project.name")
        self.crew("mark", "--review", "review.tsv", "--uuid", "c-grant", "--uuid", "c-doctor", "--uuid", "c-blocks",
                  "--as", "y")
        self.crew("mark", "--review", "review.tsv", "--uuid", "c-sam", "--as", "n")
        proc = self.crew("mine", "--export", str(self.export.path), "--review", "review.tsv", "--out", "ai.jsonl")
        records = [json.loads(line) for line in self.h.read("ai.jsonl").splitlines()]
        self.assertEqual([r["text"] for r in records],
                         ["tighten aim 2 around the community partner need",
                          "keep the partner sentence first, cut the rest", "grade levels stay at four"])
        first, second, third = records
        self.assertEqual(first["project"], "claude.ai/Grants")
        self.assertEqual(third["project"], "claude.ai")
        self.assertEqual(first["kind"], "message")
        self.assertEqual(first["timestamp"], "2026-03-01T12:00:00.000Z")
        self.assertEqual(second["context"], "Here is a tighter aim 2 focused on partners.")
        self.assertEqual(first["context"], "")
        self.assertEqual({r["split"] for r in records}, {"train"})
        self.assertIn("read 3 conversations marked mine; skipped 1 not mine and 1 unmarked", proc.stdout)
        self.assertIn("dropped 1 personal records, 1 secret records", proc.stdout)
        self.assertNotIn("staging", self.h.read("ai.jsonl"))
        self.assertNotIn("week 3", self.h.read("ai.jsonl"))


class Merge(unittest.TestCase):
    def test_merges_machines_dedupes_and_resplits(self):
        h = Harness()
        member = h.member(ARI)
        h.session("-Users-p-projects-grants", "s1",
                  ask("2026-09-01T10:00:00Z", "Lead with aim 1?", ["Yes", "No"], "Yes (Recommended)"),
                  ask("2026-09-03T10:00:00Z", "Cut the table?", ["Cut", "Keep"], "Keep"),
                  said("2026-09-04T10:00:00Z", "keep the partner sentence first"))
        laptop = h.root / "laptop"
        (laptop / "-Users-p-projects-course").mkdir(parents=True)
        (laptop / "-Users-p-projects-course" / "s2.jsonl").write_text("".join(json.dumps(r) + "\n" for r in [
            ask("2026-09-02T10:00:00Z", "Four grade levels?", ["Yes", "No"], "Yes (Recommended)")]))
        for projects, out in ((h.projects, "decisions-desk.jsonl"), (laptop, "decisions-laptop.jsonl")):
            proc = h.crew(member, "mine", "--projects", str(projects), "--out", out, "--holdout", "0.5")
            self.assertEqual(proc.returncode, 0, proc.stderr)
        proc = h.crew(member, "merge", "--in", "decisions-desk.jsonl", "decisions-laptop.jsonl",
                      "decisions-desk.jsonl", "--out", "decisions.jsonl", "--holdout", "0.34")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        records = [json.loads(line) for line in h.read("decisions.jsonl").splitlines()]
        self.assertEqual([r.get("question") or r["text"] for r in records],
                         ["Lead with aim 1?", "Four grade levels?", "Cut the table?",
                          "keep the partner sentence first"])
        self.assertEqual([r["split"] for r in records], ["train", "train", "heldout", "heldout"])
        self.assertIn("merged 3 questions (1 held out) and 1 messages from 3 files; 3 duplicates dropped",
                      proc.stdout)


    def test_train_only_inputs_never_move_the_cutoff_or_join_the_test_set(self):
        h = Harness()
        member = h.member(ARI)
        h.session("-Users-p-projects-grants", "s1",
                  ask("2026-09-01T10:00:00Z", "Lead with aim 1?", ["Yes", "No"], "Yes (Recommended)"),
                  ask("2026-09-03T10:00:00Z", "Cut the table?", ["Cut", "Keep"], "Keep"))
        h.session("-Users-p-interview", "s2",
                  ask("2026-10-20T10:00:00Z", "Submit at 70%?", ["Submit", "Wait"], "Wait"))
        h.crew(member, "mine", "--projects", str(h.projects), "--out", "all.jsonl")
        records = [json.loads(line) for line in h.read("all.jsonl").splitlines()]
        (h.root / "real.jsonl").write_text("".join(json.dumps(r) + "\n" for r in records if r["project"] != "interview"))
        (h.root / "decisions-interview.jsonl").write_text(
            "".join(json.dumps(r) + "\n" for r in records if r["project"] == "interview"))
        proc = h.crew(member, "merge", "--in", "real.jsonl", "--train", "decisions-interview.jsonl",
                      "--out", "decisions.jsonl", "--holdout", "0.5")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        merged = {r["question"]: r["split"] for r in map(json.loads, h.read("decisions.jsonl").splitlines())}
        self.assertEqual(merged, {"Lead with aim 1?": "train", "Cut the table?": "heldout", "Submit at 70%?": "train"})
        self.assertIn("held out from 2026-09-03T10:00:00Z", proc.stdout)


    def test_merge_reapplies_the_current_privacy_filters(self):
        h = Harness()
        member = h.member(ARI)
        h.session("-Users-p-projects-course", "s1", said("2026-09-01T10:00:00Z", "use a grade curve this term"),
                  said("2026-09-02T10:00:00Z", "keep four grade levels"))
        self.assertEqual(h.crew(member, "mine", "--projects", str(h.projects), "--out", "laptop.jsonl").returncode, 0)
        stricter = h.member({**ARI, "filters": {**ARI["filters"], "personal": ARI["filters"]["personal"] + "|grade curve"}},
                            name="stricter.json")
        proc = h.crew(stricter, "merge", "--in", "laptop.jsonl", "laptop.jsonl", "--out", "decisions.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual([r["text"] for r in map(json.loads, h.read("decisions.jsonl").splitlines())],
                         ["keep four grade levels"])
        self.assertIn("dropped 1 personal records, 0 secret records", proc.stdout)
        self.assertIn("2 duplicates dropped", proc.stdout)


class Folders(unittest.TestCase):
    def test_lists_each_folder_with_its_sessions_and_whether_it_is_mined(self):
        h = Harness()
        member = h.member(ARI)
        h.session("-Users-p-projects-grants", "s1", said("2026-09-01T10:00:00Z", "a"))
        h.session("-Users-p-projects-grants", "s2", said("2026-09-01T10:00:00Z", "b"))
        h.session("-Users-p-hoa-board", "s3", said("2026-09-01T10:00:00Z", "c"))
        proc = h.crew(member, "folders", "--projects", str(h.projects))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout.splitlines(),
                         ["mined\t2\tgrants\t-Users-p-projects-grants",
                          "excluded\t1\thoa-board\t-Users-p-hoa-board",
                          "1 mined folders (2 sessions), 1 excluded (1 sessions)"])


if __name__ == "__main__":
    unittest.main()
