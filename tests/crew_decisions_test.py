#!/usr/bin/env python3
"""Pin how crew_decisions reads fictional member configs and applies their filters, marks and names.

Every case runs the real CLI over fixture transcripts in a temp dir, so no test reads a real one.
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_fixtures import assert_cli_error, ask, reply, said  # noqa: E402

SCRIPTS = Path(__file__).resolve().parent.parent / "scripts"
CREW = SCRIPTS / "crew_decisions.py"


def marked(when, question, labels, answer):
    """A question whose labels already carry their gate marks."""
    return ask(when, question, labels, answer, recommended=None)


ADA = {"name": "Ada", "slug": "ada", "pronouns": "she/her", "principal": "Sam",
       "principal_slug": "sam", "principal_pronouns": "they/them", "state_dir": "STATE", "mark_word": "Ada",
       "filters": {"excluded_folders": r"garden-club|^-private-", "personal": r"\bmy therapist\b",
                   "secret": r"\b\d{9,}\b", "pasted_prefixes": ["You are "]}}

ARI = {"name": "Ari", "slug": "ari", "pronouns": "they/them", "principal": "Robin",
       "principal_slug": "robin", "state_dir": "STATE", "mark_word": "Ari",
       "filters": {"excluded_folders": r"hoa-board|^-private-", "personal": r"\bmy doctor\b",
                   "secret": r"\b\d{7,}\b|[\w.+-]+@[\w-]+\.[a-z]{2,}", "pasted_prefixes": ["Pasted: "]}}


class Harness:
    def __init__(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.projects = self.root / "projects"

    def session(self, folder, name, *records):
        path = self.projects / folder / f"{name}.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("".join(json.dumps(r) + "\n" for r in records))

    def member(self, config, name="member.json"):
        config = json.loads(json.dumps(config).replace("STATE", str(self.root / "state")))
        (self.root / name).write_text(json.dumps(config))
        return str(self.root / name)

    def run(self, script, *args, env=None):
        return subprocess.run([sys.executable, str(script), *args], capture_output=True, text=True,
                              cwd=self.root, timeout=60, env={**os.environ, "HOME": "/Users/p", **(env or {})})

    def crew(self, member, *args):
        return self.run(CREW, "--member", member, *args)

    def read(self, name):
        return (self.root / name).read_text()

    def write(self, name, value):
        (self.root / name).write_text(json.dumps(value))


def ada_fixture(h):
    h.session("-Users-p-projects-app", "s1",
              reply("2026-09-01T09:59:00Z", "Shall I keep two copies of the review queue?"),
              said("2026-09-01T10:00:00Z", "no, keep one source of truth for the review queue"),
              ask("2026-09-01T10:01:00Z", "Where should the review queue live?", ["Repo", "State"],
                  "Repo (Recommended · Ada)", member_pick=0),
              ask("2026-09-02T10:00:00Z", "Which review queue order?", ["Oldest", "Newest"], "Newest (Ada)",
                  member_pick=1),
              ask("2026-09-03T10:00:00Z", "Split the review queue?", ["Yes", "No"], "split it by repo instead"),
              said("2026-09-03T10:01:00Z", "You are a lane runner"),
              said("2026-09-03T10:02:00Z", "my therapist moved the review queue meeting"),
              said("2026-09-03T10:03:00Z", "the account number is 123456789"))
    h.session("-Users-p-projects-garden-club", "s2", said("2026-09-01T10:00:00Z", "grant call"))
    h.session("-private-scratch", "s3", said("2026-09-01T10:00:00Z", "budget call"))


class ExampleMember(unittest.TestCase):
    def test_mine_split_score_and_search(self):
        h = Harness()
        ada_fixture(h)
        member = h.member(ADA)
        out = h.crew(member, "mine", "--projects", str(h.projects), "--out", "d.jsonl", "--holdout", "1")
        self.assertEqual(out.returncode, 0, out.stderr)
        records = [json.loads(line) for line in h.read("d.jsonl").splitlines()]
        self.assertEqual(len(records), 4)
        self.assertEqual(sum(r["kind"] == "question" for r in records), 3)
        out = h.crew(member, "split", "--corpus", "d.jsonl", "--questions", "q.jsonl", "--answers", "a.jsonl")
        self.assertEqual(out.returncode, 0, out.stderr)
        questions = [json.loads(line) for line in h.read("q.jsonl").splitlines()]
        answers = [json.loads(line) for line in h.read("a.jsonl").splitlines()]
        self.assertEqual(len(answers), 3)
        self.assertEqual([q["id"] for q in questions], [a["id"] for a in answers])
        self.assertTrue(all("picks" not in q and "ada_pick" not in q for q in questions))
        found = h.crew(member, "similar", "--corpus", "d.jsonl", "--query",
                       "where should the review queue live", "--json")
        self.assertEqual(found.returncode, 0, found.stderr)
        self.assertEqual(json.loads(found.stdout), [
            {"kind": "message", "date": "2026-09-01", "project": "app",
             "text": "no, keep one source of truth for the review queue"}])
        h.write("labels.json", {a["id"]: {"category": "scope"} for a in answers})
        h.write("p.json", {a["id"]: {"pick": "Repo", "confidence": "high"} for a in answers})
        for fmt in ([], ["--json"]):
            scored = h.crew(member, "score", "--answers", "a.jsonl", "--labels", "labels.json",
                            "--predictions", "p.json", *fmt)
            self.assertEqual(scored.returncode, 0, scored.stderr)
            if not fmt:
                self.assertIn("Ada-led questions: 1 of 2 (50%); Sam took Ada's pick 100%", scored.stdout)
            else:
                self.assertEqual(json.loads(scored.stdout)["ada_led"],
                                 {"n": 2, "hits": 1, "accuracy": 0.5, "took_ada": 1.0})

    def test_record_uses_the_member_and_principal_keys(self):
        h = Harness()
        member = h.member(ADA)
        recorded = h.crew(member, "record", "--log", "live.jsonl", "--work", "plan.md", "--kind", "escalated",
                          "--category", "scope", "--tier", "low", "--question", "Q?", "--ada-pick", "A",
                          "--rule", "below-high", "--sam-answer", "B")
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        line = json.loads(h.read("live.jsonl"))
        self.assertTrue(line.pop("timestamp"))
        self.assertEqual(list(line), ["work", "kind", "category", "tier", "question", "ada_pick", "rule",
                                     "sam_answer"])
        self.assertEqual(list(line.values()), ["plan.md", "escalated", "scope", "low", "Q?", "A", "below-high", "B"])


HOSTILE = "$(touch pwned-dollar) `touch pwned-tick` ${HOME} \"double\" 'single' back\\slash café\nEND_OF_TEXT\ntext"


class StdinTexts(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.member = self.h.member(ARI)
        self.values = {"work": HOSTILE, "kind": "escalated", "category": HOSTILE, "tier": "low",
                       "question": HOSTILE, "pick": HOSTILE}

    def stdin(self, command, payload, *flags):
        data = payload if isinstance(payload, bytes) else payload.encode("utf-8")
        return subprocess.run([sys.executable, str(CREW), "--member", self.member, command, *flags],
                              input=data, capture_output=True, cwd=self.h.root, timeout=60)

    def test_record_stdin_matches_flags_and_keeps_optional_nulls(self):
        for optional in ({}, {"rule": None, "answer": None}, {"rule": HOSTILE, "answer": HOSTILE}):
            with self.subTest(optional=optional):
                values = {**self.values, **optional}
                proc = self.stdin("record", json.dumps(values, ensure_ascii=False), "--log", "stdin.jsonl", "--stdin")
                self.assertEqual(proc.returncode, 0, proc.stderr)
                flags = ["--log", "flags.jsonl"]
                for key, value in values.items():
                    if value is not None:
                        flag = {"pick": "ari-pick", "answer": "robin-answer"}.get(key, key)
                        flags.extend(["--" + flag, value])
                flagged = self.h.crew(self.member, "record", *flags)
                self.assertEqual(flagged.returncode, 0, flagged.stderr)
                self.assertEqual(proc.stdout.decode(), flagged.stdout)
                actual = json.loads(self.h.read("stdin.jsonl").splitlines()[-1])
                expected = json.loads(self.h.read("flags.jsonl").splitlines()[-1])
                actual.pop("timestamp")
                expected.pop("timestamp")
                self.assertEqual(actual, expected)
                self.assertEqual(list(actual), ["work", "kind", "category", "tier", "question", "ari_pick",
                                                "rule", "robin_answer"])
                self.assertEqual(actual["question"], HOSTILE)

    def test_similar_stdin_matches_query_with_both_output_forms(self):
        ada_fixture(self.h)
        member = self.h.member(ADA)
        mined = self.h.crew(member, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl")
        self.assertEqual(mined.returncode, 0, mined.stderr)
        self.member = member
        query = "review queue " + HOSTILE
        for fmt in ([], ["--json"]):
            proc = self.stdin("similar", " \n" + json.dumps({"query": query}) + "\n ",
                              "--corpus", "d.jsonl", "--stdin", *fmt)
            self.assertEqual(proc.returncode, 0, proc.stderr)
            flagged = self.h.crew(member, "similar", "--corpus", "d.jsonl", "--query", query, *fmt)
            self.assertEqual(flagged.returncode, 0, flagged.stderr)
            self.assertEqual(proc.stdout.decode(), flagged.stdout)
            self.assertIn("review queue", flagged.stdout)

    def test_stdin_refusals_are_usage_errors_before_reading_or_writing(self):
        for command, good in (("record", self.values), ("similar", {"query": HOSTILE})):
            key = "pick" if command == "record" else "query"
            cases = [(b"not json", "JSON object"), (b"[]", "JSON object"),
                     (json.dumps({**good, "unknown": "x"}).encode(), "unknown"),
                     ((json.dumps(good)[:-1] + ', "' + key + '": "again"}').encode(), key),
                     (json.dumps({k: v for k, v in good.items() if k != key}).encode(), key),
                     (json.dumps({**good, key: 7}).encode(), key),
                     (b" " * 65537, "64 KiB"), (b"\xff", "UTF-8"),
                     (json.dumps({**good, key: "\ud800"}).encode(), "JSON object")]
            if command == "record":
                cases += [(json.dumps({**good, "rule": 7}).encode(), "rule"),
                          (json.dumps({**good, "answer": 7}).encode(), "answer"),
                          (json.dumps({**good, "kind": "maybe"}).encode(), "kind"),
                          (json.dumps({**good, "tier": "top"}).encode(), "tier")]
            for payload, problem in cases:
                with self.subTest(command=command, problem=problem, payload=payload[:80]):
                    flags = ["--log", "live.jsonl"] if command == "record" else ["--corpus", "absent.jsonl"]
                    proc = self.stdin(command, payload, *flags, "--stdin")
                    self.assertEqual(proc.returncode, 2, proc.stderr)
                    self.assertIn(problem, proc.stderr.decode())
                    self.assertIn(command + ": error:", proc.stderr.decode())
                    self.assertEqual(proc.stdout, b"")
                    self.assertFalse((self.h.root / "live.jsonl").exists())

    def test_stdin_and_flags_are_either_or_and_flags_still_name_every_missing_argument(self):
        for flag in ("--work", "--kind", "--category", "--tier", "--question", "--ari-pick", "--rule", "--robin-answer"):
            value = {"--kind": "decided", "--tier": "high"}.get(flag, "text")
            proc = self.stdin("record", json.dumps(self.values), "--stdin", flag, value)
            self.assertEqual(proc.returncode, 2, proc.stderr)
            self.assertIn(b"give the texts either on stdin (--stdin) or as flags, not both", proc.stderr)
        proc = self.stdin("record", b"")
        self.assertEqual(proc.returncode, 2, proc.stderr)
        self.assertIn(b"the following arguments are required: --work, --kind, --category, --tier, --question, --ari-pick",
                      proc.stderr)
        proc = self.stdin("similar", '{"query": "text"}', "--stdin", "--query", "text")
        self.assertEqual(proc.returncode, 2, proc.stderr)
        proc = self.stdin("similar", b"")
        self.assertEqual(proc.returncode, 2, proc.stderr)
        self.assertIn(b"--query", proc.stderr)


class AnotherMember(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.member = self.h.member(ARI)

    def mine(self):
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc, [json.loads(line) for line in self.h.read("d.jsonl").splitlines()]

    def test_their_mark_word_records_their_pick_and_another_members_mark_is_not_theirs(self):
        self.h.session("-Users-p-projects-grants", "s1",
                       marked("2026-09-01T10:00:00Z", "Lead with aim 1?", ["Yes (Recommended · Ari)", "No"],
                              "Yes (Recommended · Ari)"),
                       marked("2026-09-02T10:00:00Z", "Cut the budget table?", ["Cut", "Keep (Ada)"], "Keep (Ada)"))
        _, records = self.mine()
        a, b = records
        self.assertEqual(a["ari_pick"], "Yes (Recommended · Ari)")
        self.assertEqual(a["outcome"], "took_rec")
        self.assertNotIn("ada_pick", a)
        self.assertIsNone(b["ari_pick"])

    def test_their_filters_replace_the_other_members(self):
        self.h.session("-Users-p-projects-lab-dashboard", "s1",
                       said("2026-09-01T10:00:00Z", "the grant call is due friday"),
                       said("2026-09-01T10:01:00Z", "my doctor moved it"),
                       said("2026-09-01T10:02:00Z", "student 12345678 asked for an extension"),
                       said("2026-09-01T10:03:00Z", "write to someone@example.edu"),
                       said("2026-09-01T10:04:00Z", "Pasted: a syllabus"),
                       said("2026-09-01T10:05:00Z", "the IRB amendment goes first"),
                       said("2026-09-01T10:06:00Z", "You are kept for this member"))
        self.h.session("-Users-p-projects-hoa-board", "s2", said("2026-09-01T10:00:00Z", "dues"))
        proc, records = self.mine()
        self.assertEqual([r["text"] for r in records],
                         ["the grant call is due friday", "the IRB amendment goes first",
                          "You are kept for this member"])
        self.assertIn("dropped 1 excluded folders, 1 personal records, 2 secret records", proc.stdout)

    def test_score_and_record_use_their_names(self):
        self.h.session("-Users-p-projects-grants", "s1",
                       marked("2026-09-01T10:00:00Z", "Lead with aim 1?", ["Yes (Recommended · Ari)", "No"],
                              "Yes (Recommended · Ari)"))
        self.mine()
        h = self.h
        self.assertEqual(h.crew(self.member, "split", "--corpus", "d.jsonl", "--questions", "q.jsonl",
                                "--answers", "a.jsonl").returncode, 0)
        [answer] = [json.loads(line) for line in h.read("a.jsonl").splitlines()]
        self.assertIn("ari_pick", answer)
        h.write("labels.json", {answer["id"]: {"category": "funder-strategy"}})
        h.write("p.json", {answer["id"]: {"pick": "Yes", "confidence": "high"}})
        scored = h.crew(self.member, "score", "--answers", "a.jsonl", "--labels", "labels.json", "--predictions",
                        "p.json", "--json")
        self.assertEqual(scored.returncode, 0, scored.stderr)
        result = json.loads(scored.stdout)
        self.assertEqual(result["ari_led"], {"n": 1, "hits": 1, "accuracy": 1.0, "took_ari": 1.0})
        text = h.crew(self.member, "score", "--answers", "a.jsonl", "--labels", "labels.json", "--predictions",
                      "p.json").stdout
        self.assertIn("Ari-led questions: 1 of 1 (100%); Robin took Ari's pick 100%", text)
        recorded = h.crew(self.member, "record", "--work", "w", "--kind", "decided", "--category", "scope",
                          "--tier", "high", "--question", "Q?", "--ari-pick", "A", "--robin-answer", "B")
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        line = json.loads((h.root / "state" / "live.jsonl").read_text())
        self.assertEqual((line["ari_pick"], line["robin_answer"]), ("A", "B"))
        self.assertNotIn("ada_pick", line)

    def test_similar_defaults_to_the_corpus_in_their_state_dir(self):
        self.h.session("-Users-p-projects-grants", "s1",
                       said("2026-09-01T10:00:00Z", "always lead the aims page with the community partner need"))
        state = self.h.root / "state"
        state.mkdir()
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out",
                           str(state / "decisions.jsonl"), "--holdout", "0")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        found = self.h.crew(self.member, "similar", "--query", "aims page community partner", "--json")
        self.assertEqual(found.returncode, 0, found.stderr)
        self.assertEqual(len(json.loads(found.stdout)), 1)

    def test_the_env_var_names_the_member(self):
        self.h.session("-Users-p-projects-grants", "s1", said("2026-09-01T10:00:00Z", "kept"))
        proc = self.h.run(CREW, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl",
                          env={"CREW_MEMBER": self.member})
        self.assertEqual(proc.returncode, 0, proc.stderr)


class PrivateOverlay(unittest.TestCase):
    """A gitignored private.json beside member.json holds what must not be committed: its filters
    extend the member's (both apply), and any other key it names replaces the member's."""

    def setUp(self):
        self.h = Harness()
        self.member = self.h.member(ARI)

    def overlay(self, value):
        (self.h.root / "private.json").write_text(value if isinstance(value, str) else json.dumps(value))

    def test_its_filters_extend_the_members(self):
        self.overlay({"filters": {"excluded_folders": "garden-club", "personal": r"\bmy cat\b",
                                  "secret": r"\bpin \d{4}\b", "pasted_prefixes": ["Quoted: "]}})
        self.h.session("-Users-p-projects-grants", "s1",
                       said("2026-09-01T10:00:00Z", "the grant call is due friday"),
                       said("2026-09-01T10:01:00Z", "my doctor moved it"),
                       said("2026-09-01T10:02:00Z", "my cat is at the vet"),
                       said("2026-09-01T10:03:00Z", "the door pin 1234 changed"),
                       said("2026-09-01T10:04:00Z", "Pasted: a syllabus"),
                       said("2026-09-01T10:05:00Z", "Quoted: an email"))
        self.h.session("-Users-p-projects-hoa-board", "s2", said("2026-09-01T10:00:00Z", "dues"))
        self.h.session("-Users-p-garden-club", "s3", said("2026-09-01T10:00:00Z", "the plot rota"))
        proc = self.h.crew(self.member, "mine", "--projects", str(self.h.projects), "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        texts = [json.loads(line)["text"] for line in self.h.read("d.jsonl").splitlines()]
        self.assertEqual(texts, ["the grant call is due friday"])
        self.assertIn("dropped 2 excluded folders, 2 personal records, 1 secret records", proc.stdout)

    def test_any_other_key_replaces_the_members(self):
        self.overlay({"principal_slug": "sam"})
        recorded = self.h.crew(self.member, "record", "--work", "w", "--kind", "decided", "--category", "scope",
                               "--tier", "high", "--question", "Q?", "--ari-pick", "A", "--sam-answer", "B")
        self.assertEqual(recorded.returncode, 0, recorded.stderr)
        line = json.loads((self.h.root / "state" / "live.jsonl").read_text())
        self.assertEqual(line["sam_answer"], "B")

    def test_a_broken_overlay_is_named(self):
        self.overlay("{not json")
        proc = self.h.crew(self.member, "mine", "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("private.json is not JSON", proc.stderr)

    def test_an_overlay_of_the_wrong_shape_is_named(self):
        for value, named in (([1, 2], "private.json must be a JSON object"),
                             ({"filters": {"personal": ["a", "b"]}}, "private.json: filters.personal must be a string")):
            self.overlay(value)
            proc = self.h.crew(self.member, "mine", "--out", "d.jsonl")
            self.assertEqual(proc.returncode, 1, value)
            self.assertIn(named, proc.stderr)


FILLED = {"NAME": "Ari", "SLUG": "ari", "PRONOUNS": "they/them", "PRINCIPAL": "Robin", "PRINCIPAL_SLUG": "robin",
          "PRINCIPAL_PRONOUNS": "they/them", "REPO": "~/projects/voice", "DATE": "2026-10-22"}


def asking(when, tool_id, question, entrypoint):
    """An AskUserQuestion call still waiting for its answer, in a session opened from `entrypoint`."""
    return {"type": "assistant", "timestamp": when, "cwd": "/repo", "entrypoint": entrypoint,
            "message": {"role": "assistant", "content": [
                {"type": "tool_use", "id": tool_id, "name": "AskUserQuestion",
                 "input": {"questions": [{"question": question, "options": [{"label": "a"}, {"label": "b"}]}]}}]}}


class Pending(unittest.TestCase):
    def test_only_headless_sessions_are_reported(self):
        h = Harness()
        member = h.member(ADA)
        for name, entrypoint in [("desktop", "claude-desktop"), ("terminal", "cli"), ("job-run", "sdk-cli"),
                                 ("unknown", None)]:
            h.session("-Users-p-projects-app", name, asking("2026-10-25T10:00:00Z", "t1", "Which?", entrypoint))
        proc = h.crew(member, "pending", "--projects", str(h.projects), "--seen", str(h.root / "seen.json"))
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual([json.loads(line)["session"] for line in proc.stdout.splitlines()], ["job-run"])


class PrivacyCheck(unittest.TestCase):
    """The step 11 gate over a crew folder made from the template the way install.sh makes it."""

    def setUp(self):
        self.h = Harness()
        self.dir = self.h.root / "crew"
        shutil.copytree(SCRIPTS.parent / "crew" / "_template", self.dir)
        (self.dir / "README.md").unlink()
        for path in self.dir.rglob("*"):
            text = path.read_text() if path.is_file() else None
            for key, value in FILLED.items():
                text = text and text.replace("{{" + key + "}}", value)
            if text is not None:
                path.write_text(text)

    def check(self):
        return self.h.crew(str(self.dir / "member.json"), "privacy-check", "--dir", str(self.dir))

    def append(self, name, line):
        path = self.dir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text((path.read_text() if path.exists() else "") + line + "\n")
        return len(path.read_text().splitlines())

    def edit_member(self, change):
        config = json.loads((self.dir / "member.json").read_text())
        change(config)
        (self.dir / "member.json").write_text(json.dumps(config, indent=2))

    def test_the_template_is_clean_although_its_filters_match_themselves(self):
        proc = self.check()
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertEqual(proc.stdout, "clean\n")

    def test_markdown_hits_name_the_file_and_line_but_never_the_text(self):
        line = self.append("decisions.md", "We moved it because my doctor called.")
        self.append("briefs/extra.md", "Write to robin@example.org first.")
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn(f"decisions.md:{line} (personal)", proc.stdout)
        self.assertIn("briefs/extra.md:1 (secret)", proc.stdout)
        self.assertTrue(proc.stdout.endswith("2 hits\n"), proc.stdout)
        for text in ("doctor", "robin@"):
            self.assertNotIn(text, proc.stdout + proc.stderr)

    def test_member_json_is_scanned_outside_its_filters(self):
        self.edit_member(lambda c: c["codebook"]["domains"].append("robin@example.org"))
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json:codebook.domains[4] (secret)", proc.stdout)
        self.assertNotIn("robin@", proc.stdout)

    def test_filters_that_differ_from_the_template_are_hits(self):
        self.edit_member(lambda c: c["filters"].update(personal=c["filters"]["personal"] + r"|\bgarden club\b"))
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json:filters.personal (differs from crew/_template)", proc.stdout)
        self.assertIn("private.json", proc.stdout)
        self.assertNotIn("garden", proc.stdout)

    def test_private_json_patterns_apply_but_private_json_is_not_scanned(self):
        (self.dir / "private.json").write_text(json.dumps({"filters": {"personal": r"\bgarden club\b"}}))
        line = self.append("autonomy.md", "the garden club rota")
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn(f"autonomy.md:{line} (personal)", proc.stdout)
        self.assertNotIn("private.json:", proc.stdout)
        self.assertTrue(proc.stdout.endswith("1 hits\n"), proc.stdout)

    def test_every_committed_file_is_scanned_at_any_depth(self):
        (self.dir / "engineers").mkdir()
        self.append("engineers/example.md", "my doctor again")
        self.append("briefs/sub/deep.md", "my doctor again")
        self.append("report-template.html", "<p>my doctor again</p>")
        self.append("notes.txt", "my doctor again")
        self.append("extra.json", '{"note": "my doctor again"}')
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        for name in ("engineers/example.md:1", "briefs/sub/deep.md:1", "report-template.html:1", "notes.txt:1",
                     "extra.json:1"):
            self.assertIn(f"{name} (personal)", proc.stdout)
        self.assertTrue(proc.stdout.endswith("5 hits\n"), proc.stdout)

    def test_a_file_that_is_not_text_is_a_hit_by_name(self):
        (self.dir / "clip.bin").write_bytes(b"caf\xe9")
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("clip.bin (not text)", proc.stdout)

    def test_a_key_that_trips_a_filter_is_named_by_its_parent_only(self):
        self.edit_member(lambda c: c["codebook"]["groups"].update({"robin@example.org": ["scope"]}))
        proc = self.check()
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json:codebook.groups (a key, secret)", proc.stdout)
        self.assertNotIn("robin@", proc.stdout)

    def test_a_member_json_that_is_not_an_object_is_named(self):
        member = self.h.member(ARI)
        (self.dir / "member.json").write_text("[1, 2]")
        proc = self.h.crew(member, "privacy-check", "--dir", str(self.dir))
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member.json must be a JSON object", proc.stderr)
        assert_cli_error(self, proc.stderr, "crew_decisions:")

    def test_a_folder_without_member_json_is_named(self):
        member = self.h.member(ARI)
        empty = self.h.root / "empty"
        empty.mkdir()
        proc = self.h.crew(member, "privacy-check", "--dir", str(empty))
        self.assertEqual(proc.returncode, 1)
        self.assertIn(f"{empty}/member.json", proc.stderr)
        assert_cli_error(self, proc.stderr, "crew_decisions:")


class BadConfig(unittest.TestCase):
    def test_no_member_is_an_error(self):
        h = Harness()
        env = {k: v for k, v in os.environ.items() if k != "CREW_MEMBER"}
        proc = subprocess.run([sys.executable, str(CREW), "mine", "--out", "d.jsonl"], capture_output=True,
                              text=True, cwd=h.root, env=env, timeout=60)
        self.assertEqual(proc.returncode, 1)
        self.assertIn("crew_decisions: no member config (--member or CREW_MEMBER)", proc.stderr)

    def test_a_missing_key_is_named(self):
        h = Harness()
        member = h.member({k: v for k, v in ARI.items() if k != "slug"})
        proc = h.crew(member, "mine", "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member config: missing slug", proc.stderr)

    def test_a_bad_regex_is_named(self):
        h = Harness()
        member = h.member({**ARI, "filters": {**ARI["filters"], "personal": "(unclosed"}})
        proc = h.crew(member, "mine", "--out", "d.jsonl")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("filters.personal", proc.stderr)


if __name__ == "__main__":
    unittest.main()
