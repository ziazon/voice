#!/usr/bin/env python3
"""Pin the labelling steps of crew_decisions: chunking the corpus for labellers, checking their
labels against the member's codebook, grouping labelled records for distillation, and the
6-word-shingle check that keeps a distilled file paraphrase only."""
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_decisions_test import ARI, Harness  # noqa: E402
from crew_fixtures import ask, reply, said  # noqa: E402

CODEBOOK = {"categories": ["scope", "methods", "writing-voice", "other"],
            "domains": ["research", "grants", "teaching", "other"],
            "escalates": ["send-external", "submit", "privacy"],
            "signals": ["ruling", "correction", "direction", "ack", "other"],
            "groups": {"g1-work": ["scope", "methods"], "g2-words": ["writing-voice", "other"]}}


class Labels(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.member = self.h.member({**ARI, "codebook": CODEBOOK})
        self.h.session("-Users-p-projects-grants", "s1",
                       ask("2026-09-01T10:00:00Z", "Lead with aim 1?", ["Yes", "No"], "Yes (Recommended)"),
                       reply("2026-09-01T10:00:30Z", "Shall I trim the methods section by half?"),
                       said("2026-09-01T10:01:00Z", "always keep the community partner sentence first in every aim"),
                       ask("2026-09-02T10:00:00Z", "Cut the table?", ["Cut", "Keep"], "Keep"),
                       ask("2026-09-03T10:00:00Z", "Send to the PO?", ["Send", "Wait"], "Wait"))
        self.ok(self.crew("mine", "--projects", str(self.h.projects), "--out", "d.jsonl", "--holdout", "0.34"))
        self.records = [json.loads(line) for line in self.h.read("d.jsonl").splitlines()]

    def crew(self, *args):
        return self.h.crew(self.member, *args)

    def ok(self, proc):
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        return proc

    def train_labels(self):
        out = {}
        for r in self.records:
            if r["split"] != "train":
                continue
            label = {"category": "scope", "domain": "grants", "reason": "keeps the partner need up front"}
            label.update({"escalates": []} if r["kind"] == "question" else {"signal": "ruling"})
            out[r["id"]] = label
        return out

    def test_chunk_writes_train_records_and_answerless_heldout_questions(self):
        proc = self.ok(self.crew("chunk", "--corpus", "d.jsonl", "--out-dir", "chunks", "--size", "2"))
        chunks = sorted(p.name for p in (self.h.root / "chunks").iterdir())
        self.assertEqual(chunks, ["heldout-01.jsonl", "train-01.jsonl", "train-02.jsonl"])
        self.assertEqual((self.h.root / "chunks").stat().st_mode & 0o777, 0o700)
        for name in chunks:
            self.assertEqual((self.h.root / "chunks" / name).stat().st_mode & 0o777, 0o600)
        train = [json.loads(line) for name in ("train-01.jsonl", "train-02.jsonl")
                 for line in (self.h.root / "chunks" / name).read_text().splitlines()]
        self.assertEqual(len(train), 3)
        [held] = [json.loads(line) for line in (self.h.root / "chunks/heldout-01.jsonl").read_text().splitlines()]
        self.assertEqual(held["question"], "Send to the PO?")
        for answer in ("picks", "own_words", "notes", "outcome", "ari_pick"):
            self.assertNotIn(answer, held)
        self.assertIn("3 train records in 2 chunks, 1 held-out questions in 1 chunks", proc.stdout)

    def test_check_labels_accepts_a_codebook_match(self):
        self.crew("chunk", "--corpus", "d.jsonl", "--out-dir", "chunks", "--size", "10")
        self.h.write("train.json", self.train_labels())
        proc = self.ok(self.crew("check-labels", "--chunk", "chunks/train-01.jsonl", "--labels", "train.json",
                                 "--mode", "train"))
        self.assertEqual(proc.stdout.strip(), "OK")

    def test_check_labels_names_every_problem(self):
        self.crew("chunk", "--corpus", "d.jsonl", "--out-dir", "chunks", "--size", "10")
        labels = self.train_labels()
        ids = list(labels)
        labels[ids[0]]["category"] = "approach"
        labels[ids[1]]["domain"] = "household"
        labels[ids[2]]["reason"] = "word " * 30
        labels["question:extra"] = labels.pop(ids[2])
        self.h.write("train.json", labels)
        proc = self.crew("check-labels", "--chunk", "chunks/train-01.jsonl", "--labels", "train.json",
                         "--mode", "train")
        self.assertEqual(proc.returncode, 1)
        for problem in ("missing 1, extra 1", f"{ids[0]} category", f"{ids[1]} domain"):
            self.assertIn(problem, proc.stdout)

    def test_check_labels_refuses_a_reason_on_a_heldout_question(self):
        self.crew("chunk", "--corpus", "d.jsonl", "--out-dir", "chunks", "--size", "10")
        [held] = [json.loads(line) for line in (self.h.root / "chunks/heldout-01.jsonl").read_text().splitlines()]
        self.h.write("held.json", {held["id"]: {"category": "scope", "domain": "grants", "escalates": ["submit"],
                                                "reason": "peeked"}})
        proc = self.crew("check-labels", "--chunk", "chunks/heldout-01.jsonl", "--labels", "held.json",
                         "--mode", "heldout")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("has reason", proc.stdout)

    def test_distil_in_groups_labelled_training_records(self):
        labels = self.train_labels()
        message_id = next(r["id"] for r in self.records if r["kind"] == "message")
        labels[message_id]["category"] = "writing-voice"
        self.h.write("labels.json", labels)
        proc = self.ok(self.crew("distil-in", "--corpus", "d.jsonl", "--labels", "labels.json", "--out-dir", "in"))
        self.assertIn("g1-work 2", proc.stdout)
        self.assertIn("g2-words 1", proc.stdout)
        self.assertEqual((self.h.root / "in").stat().st_mode & 0o777, 0o700)
        for path in (self.h.root / "in").iterdir():
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        [row] = [json.loads(line) for line in (self.h.root / "in/g2-words.jsonl").read_text().splitlines()]
        self.assertEqual(row["msg"], "always keep the community partner sentence first in every aim")
        self.assertEqual((row["signal"], row["domain"], row["date"], row["project"]),
                         ("ruling", "grants", "2026-09-01", "grants"))
        work = [json.loads(line) for line in (self.h.root / "in/g1-work.jsonl").read_text().splitlines()]
        self.assertEqual(work[0]["options"], ["Yes (Recommended) — about Yes (Recommended)", "No — about No"])
        self.assertEqual(work[0]["picks"], ["Yes (Recommended)"])

    def test_distil_in_refuses_a_category_outside_the_groups(self):
        labels = self.train_labels()
        labels[next(iter(labels))]["category"] = "approach"
        self.h.write("labels.json", labels)
        proc = self.crew("distil-in", "--corpus", "d.jsonl", "--labels", "labels.json", "--out-dir", "in")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("approach", proc.stderr)

    def test_shingle_finds_six_of_her_words_in_a_row(self):
        (self.h.root / "good.md").write_text("She puts the partner's need ahead of everything else in an aim.")
        (self.h.root / "bad.md").write_text("Rule: keep the community partner sentence first, always.")
        good = self.ok(self.crew("shingle", "--doc", "good.md", "--corpus", "d.jsonl"))
        self.assertEqual(good.stdout.strip(), "0 overlaps")
        bad = self.crew("shingle", "--doc", "bad.md", "--corpus", "d.jsonl")
        self.assertEqual(bad.returncode, 1)
        self.assertIn("1 overlaps", bad.stdout)

    def test_no_codebook_is_an_error_for_labelling_steps(self):
        bare = self.h.member(ARI, name="bare.json")
        proc = self.h.crew(bare, "distil-in", "--corpus", "d.jsonl", "--labels", "x.json", "--out-dir", "in")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("member config: missing codebook", proc.stderr)


if __name__ == "__main__":
    unittest.main()
