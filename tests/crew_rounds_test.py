#!/usr/bin/env python3
"""Off-Claude rounds preserve answers and a whole-round blind holdout through merge and score."""
import copy
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_decisions_test import ADA, ARI, Harness  # noqa: E402
from crew_fixtures import assert_cli_error, ask  # noqa: E402


QUESTION = {"n": 1, "header": "Coast", "question": "What does a new player see first?",
            "multiSelect": False, "recommended": "A",
            "options": [{"label": "The harbour", "description": "ships and a market"},
                        {"label": "The cliffs", "description": "a long climb"}]}

RECORD = {"id": "question:spring-01", "kind": "question", "timestamp": "2026-11-02T12:00:01.000Z",
          "project": "round-spring", "header": "Coast", "question": "What does a new player see first?",
          "multiSelect": False,
          "options": [{"label": "The harbour (Recommended)", "description": "ships and a market"},
                      {"label": "The cliffs", "description": "a long climb"}],
          "recommended": "The harbour (Recommended)", "ari_pick": None,
          "picks": ["The harbour (Recommended)"], "own_words": "", "notes": "",
          "outcome": "took_rec", "split": "train"}


class Rounds(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.member = self.h.member(ARI)
        self.questions = {"questions": [copy.deepcopy(QUESTION)]}
        self.answers = {"answered": "2026-11-02", "answers": {"1": {"pick": "A"}}}

    def round(self, *args, name="spring"):
        self.h.write("questions.json", self.questions)
        self.h.write("answers.json", self.answers)
        return self.h.crew(self.member, "round", "--name=" + name, "--questions", "questions.json",
                           "--answers", "answers.json", *args)

    def ok(self, proc):
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc

    def rows(self, name):
        return [json.loads(line) for line in self.h.read(name).splitlines()]

    def error(self, proc, *fields):
        self.assertEqual(proc.returncode, 1, proc.stderr)
        assert_cli_error(self, proc.stderr, "crew_decisions:")
        for field in fields:
            self.assertIn(field, proc.stderr)

    def test_literal_record_default_path_mode_and_idempotent_import(self):
        proc = self.ok(self.round())
        path = self.h.root / "state/decisions-round-spring.jsonl"
        self.assertEqual(self.rows("state/decisions-round-spring.jsonl"), [RECORD])
        self.assertEqual(proc.stdout, "imported 1 answers from round spring (train); 0 unanswered\n"
                         "dropped 0 personal records, 0 secret records\n")
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)
        first = path.read_bytes()
        self.ok(self.round())
        self.assertEqual(path.read_bytes(), first)

    def test_outcomes_multi_select_order_defaults_and_unanswered(self):
        self.questions["questions"] += [
            {**copy.deepcopy(QUESTION), "n": 2},
            {**copy.deepcopy(QUESTION), "n": 3},
            {"n": 4, "question": "Which coast features?", "multiSelect": True,
             "options": [{"label": "Boats"}, {"label": "Cliffs"}, {"label": "Market"}]},
            {**copy.deepcopy(QUESTION), "n": 5},
            {**copy.deepcopy(QUESTION), "n": 6}]
        self.answers["answers"].update({"2": {"pick": ["B"], "notes": "only in winter"},
                                      "3": {"own_words": "the lighthouse, always"},
                                      "4": {"pick": ["C", "A"]}, "6": {"own_words": ""}})
        proc = self.ok(self.round("--out", "round.jsonl"))
        records = self.rows("round.jsonl")
        self.assertEqual([r["outcome"] for r in records], ["took_rec", "other_option", "own_words", "no_rec"])
        self.assertEqual([r["id"] for r in records], [f"question:spring-{n:02d}" for n in range(1, 5)])
        self.assertEqual(records[1]["picks"], ["The cliffs"])
        self.assertEqual(records[1]["notes"], "only in winter")
        self.assertEqual(records[2]["own_words"], "the lighthouse, always")
        self.assertEqual(records[2]["picks"], [])
        self.assertEqual(records[3]["picks"], ["Boats", "Market"])
        self.assertEqual(records[3]["options"], [{"label": "Boats", "description": ""},
                         {"label": "Cliffs", "description": ""}, {"label": "Market", "description": ""}])
        self.assertEqual(records[3]["header"], "")
        self.assertIsNone(records[3]["recommended"])
        self.assertEqual(proc.stdout, "imported 4 answers from round spring (train); 2 unanswered\n"
                         "dropped 0 personal records, 0 secret records\n")

    def test_heldout_and_member_pick_field(self):
        self.member = self.h.member(ADA)
        self.ok(self.round("--heldout", "--out", "held.jsonl"))
        expected = {k: v for k, v in RECORD.items() if k != "ari_pick"}
        expected.update(ada_pick=None, split="heldout")
        self.assertEqual(self.rows("held.jsonl"), [expected])

    def test_personal_and_secret_records_dropped_and_counted(self):
        self.questions["questions"][0]["question"] = "What did my doctor say?"
        self.questions["questions"].append({**copy.deepcopy(QUESTION), "n": 2})
        self.answers["answers"]["2"] = {"pick": "A", "notes": "code 12345678"}
        proc = self.ok(self.round("--out", "round.jsonl"))
        self.assertEqual(self.rows("round.jsonl"), [])
        self.assertEqual(proc.stdout, "imported 0 answers from round spring (train); 0 unanswered\n"
                         "dropped 1 personal records, 1 secret records\n")

    def test_invalid_names(self):
        for name in ("", "Spring", "-spring", "spring_1", "a" * 41):
            with self.subTest(name=name):
                self.error(self.round("--out", "bad.jsonl", name=name), "name")
                self.assertFalse((self.h.root / "bad.jsonl").exists())

    def test_invalid_question_fields(self):
        cases = [
            ("options", [QUESTION["options"][0]]),
            ("options", [{"label": str(n)} for n in range(5)]),
            ("options", [{"label": "Same"}, {"label": "Same"}]),
            ("options", [{"label": ""}, {"label": "Cliffs"}]),
            ("n", 0), ("n", True), ("n", "1"),
            ("question", ""), ("question", 4), ("header", 4),
            ("multiSelect", "yes"), ("recommended", "E"),
        ]
        for field, value in cases:
            with self.subTest(field=field, value=value):
                self.questions = {"questions": [{**copy.deepcopy(QUESTION), field: value}]}
                # An invalid n cannot have a valid question number; still name the field.
                fields = (field,) if field == "n" else ("question 1", field)
                if field == "options" and len(value) == 2:
                    fields = ("question 1", "label")
                self.error(self.round("--out", "bad.jsonl"), *fields)
                self.assertFalse((self.h.root / "bad.jsonl").exists())
        self.questions = {"questions": [copy.deepcopy(QUESTION)]}
        self.questions["questions"][0]["options"][0]["description"] = 4
        self.error(self.round(), "question 1", "description")

    def test_duplicate_question_numbers(self):
        self.questions["questions"].append(copy.deepcopy(QUESTION))
        self.error(self.round(), "question 1", "n")

    def test_invalid_answers(self):
        for answer, field in (({"pick": "E"}, "pick"), ({"pick": ["A", "B"]}, "pick"),
                              ({"own_words": 7}, "own_words"), ({"notes": []}, "notes")):
            with self.subTest(answer=answer):
                self.answers["answers"] = {"1": answer}
                self.error(self.round("--out", "bad.jsonl"), "question 1", field)
                self.assertFalse((self.h.root / "bad.jsonl").exists())
        self.answers["answers"] = {"2": {"pick": "A"}}
        self.error(self.round(), "question 2")

    def test_missing_or_malformed_answered_date(self):
        for date in (None, "11-02-2026", "2026-02-30", "2026-11-02T12:00:00Z"):
            with self.subTest(date=date):
                self.answers = {"answers": {"1": {"pick": "A"}}}
                if date is not None:
                    self.answers["answered"] = date
                self.error(self.round(), "answered")


class Merge(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.member = self.h.member(ARI)

    def write_rows(self, name, records):
        (self.h.root / name).write_text("".join(json.dumps(r) + "\n" for r in records))

    def rows(self, name):
        return [json.loads(line) for line in self.h.read(name).splitlines()]

    def ok(self, proc):
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc

    def test_only_rounds_preserves_splits_and_counts_files(self):
        train = copy.deepcopy(RECORD)
        held = {**copy.deepcopy(RECORD), "id": "question:winter-01", "split": "heldout"}
        self.write_rows("train.jsonl", [train])
        self.write_rows("held.jsonl", [held])
        proc = self.ok(self.h.crew(self.member, "merge", "--rounds", "train.jsonl", "held.jsonl",
                                   "--out", "merged.jsonl"))
        self.assertEqual(self.rows("merged.jsonl"), [train, held])
        self.assertIn("merged 2 questions (1 held out) and 0 messages from 2 files; 0 duplicates dropped; "
                      "held out from nothing", proc.stdout)

    def test_no_inputs_is_a_one_line_error(self):
        proc = self.h.crew(self.member, "merge", "--out", "merged.jsonl")
        self.assertEqual(proc.returncode, 1)
        assert_cli_error(self, proc.stderr, "crew_decisions:")
        self.assertIn("crew_decisions: merge needs at least one input\n", proc.stderr)
        self.assertFalse((self.h.root / "merged.jsonl").exists())

    def test_bad_round_split_names_the_file(self):
        for split in ("test", None):
            with self.subTest(split=split):
                row = copy.deepcopy(RECORD)
                row.pop("split")
                if split is not None:
                    row["split"] = split
                self.write_rows("bad-round.jsonl", [row])
                proc = self.h.crew(self.member, "merge", "--rounds", "bad-round.jsonl", "--out", "merged.jsonl")
                self.assertEqual(proc.returncode, 1)
                assert_cli_error(self, proc.stderr, "crew_decisions:")
                self.assertIn("bad-round.jsonl", proc.stderr)
                self.assertIn("split", proc.stderr)

    def test_mined_cutoff_ignores_later_rounds_and_rounds_remain_blind(self):
        for n in range(1, 5):
            self.h.session("-Users-p-projects-world", f"s{n}",
                           ask(f"2026-09-0{n}T10:00:00Z", f"Which route {n}?", ["Coast", "Cliff"],
                               "Coast (Recommended)"))
        self.ok(self.h.crew(self.member, "mine", "--projects", str(self.h.projects),
                            "--out", "mined.jsonl", "--cutoff", "9999"))
        # Import both later rounds through the public CLI, rather than manufacturing their records.
        self.h.write("q.json", {"questions": [QUESTION]})
        self.h.write("a.json", {"answered": "2026-11-02", "answers": {"1": {"pick": "A"}}})
        for name, flags in (("train", []), ("blind", ["--heldout"])):
            self.ok(self.h.crew(self.member, "round", "--name", name, "--questions", "q.json", "--answers", "a.json",
                                "--out", name + ".jsonl", *flags))
        proc = self.ok(self.h.crew(self.member, "merge", "--in", "mined.jsonl", "--rounds", "train.jsonl",
                                   "blind.jsonl", "--out", "merged.jsonl", "--holdout", "0.5"))
        records = self.rows("merged.jsonl")
        self.assertEqual([r["split"] for r in records[:4]], ["train", "train", "heldout", "heldout"])
        self.assertEqual({r["id"]: r["split"] for r in records[4:]},
                         {"question:blind-01": "heldout", "question:train-01": "train"})
        self.assertIn("held out from 2026-09-03T10:00:00Z", proc.stdout)
        self.assertIn("from 3 files", proc.stdout)
        # A rounds-only backtest contains exactly the held-out round, never its training sibling.
        self.ok(self.h.crew(self.member, "merge", "--rounds", "train.jsonl", "blind.jsonl", "--out", "rounds.jsonl"))
        self.ok(self.h.crew(self.member, "split", "--corpus", "rounds.jsonl", "--questions", "blind-q.jsonl",
                            "--answers", "blind-a.jsonl"))
        [question] = self.rows("blind-q.jsonl")
        [answer] = self.rows("blind-a.jsonl")
        self.assertEqual(question, {**{k: RECORD[k] for k in
                         ("timestamp", "header", "question", "multiSelect", "options", "recommended")},
                         "id": "question:blind-01", "project": "round-blind"})
        self.assertEqual(answer, {**{k: RECORD[k] for k in
                         ("recommended", "ari_pick", "picks", "own_words", "notes", "outcome", "multiSelect")},
                         "id": "question:blind-01"})
        self.h.write("labels.json", {answer["id"]: {"category": "scope"}})
        self.h.write("predictions.json", {answer["id"]: {"pick": "The harbour", "confidence": "high"}})
        score = self.ok(self.h.crew(self.member, "score", "--answers", "blind-a.jsonl", "--labels", "labels.json",
                                   "--predictions", "predictions.json", "--json"))
        self.assertEqual(json.loads(score.stdout)["overall"], {"n": 1, "hits": 1, "accuracy": 1.0, "baseline": 1.0})

    def test_round_duplicates_and_current_privacy_filters(self):
        personal = {**copy.deepcopy(RECORD), "id": "question:private-01", "question": "What did my doctor say?"}
        self.write_rows("round.jsonl", [RECORD, personal])
        proc = self.ok(self.h.crew(self.member, "merge", "--rounds", "round.jsonl", "round.jsonl",
                                   "--out", "merged.jsonl"))
        self.assertEqual(self.rows("merged.jsonl"), [RECORD])
        self.assertIn("2 duplicates dropped", proc.stdout)
        self.assertIn("dropped 1 personal records, 0 secret records", proc.stdout)


if __name__ == "__main__":
    unittest.main()
