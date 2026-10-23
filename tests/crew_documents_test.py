#!/usr/bin/env python3
"""Document inputs stay private, filtered, ordered and usable by the existing shingle gate."""
import hashlib
import json
import os
import sys
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_decisions_test import ARI, Harness  # noqa: E402
from crew_fixtures import assert_cli_error  # noqa: E402


class Documents(unittest.TestCase):
    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.member = self.h.member(ARI)
        self.source = self.h.root / "work"
        self.source.mkdir()
        self.output = self.h.root / "private" / "documents"

    def file(self, name, text):
        path = self.source / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def docx(self, name, xml, compression=zipfile.ZIP_STORED):
        with zipfile.ZipFile(self.source / name, "w", compression=compression) as archive:
            archive.writestr("word/document.xml", xml)

    def documents(self, *args):
        return self.h.crew(self.member, "documents", "--dir", str(self.source),
                           "--out-dir", str(self.output), *args)

    def ok(self, proc):
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc

    def rows(self, name):
        return [json.loads(line) for line in (self.output / name).read_text().splitlines()]

    def parts(self):
        return [row for path in sorted(self.output.glob("documents-*.jsonl"))
                for row in self.rows(path.name)]

    def record(self, source, part, text):
        # Reproduce the published id format without importing any product functions.
        digest = hashlib.sha1(f"{source}\n{part}\n{text}".encode()).hexdigest()[:12]
        return {"id": "document:" + digest, "kind": "document", "source": source,
                "part": part, "text": text}

    def test_text_docx_table_image_exact_records_inventory_and_stdout(self):
        self.file("a.md", "  Build   the harbour.\n\nKeep\tthe market.  ")
        (self.source / "b.TXT").write_bytes(b"Climb the cliffs.\xff\n\nWatch the sea.")
        self.docx("c.docx", '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
                  '<w:body><w:p><w:r><w:t>First </w:t><w:t>bridge.</w:t></w:r></w:p>'
                  '<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Table route.</w:t></w:r></w:p>'
                  '</w:tc></w:tr></w:tbl></w:body></w:document>')
        self.file("maps/d.png", "IMAGE_BYTES_NEVER_READ")
        proc = self.ok(self.documents())
        self.assertEqual(self.parts(), [
            self.record("a.md", 1, "Build the harbour.\n\nKeep the market."),
            self.record("b.TXT", 1, "Climb the cliffs.\n\nWatch the sea."),
            self.record("c.docx", 1, "First bridge.\n\nTable route.")])
        self.assertEqual(self.rows("inventory.jsonl"), [
            {"source": "a.md", "kind": "text", "words": 6, "parts": 1,
             "dropped": {"personal": 0, "secret": 0}},
            {"source": "b.TXT", "kind": "text", "words": 6, "parts": 1,
             "dropped": {"personal": 0, "secret": 0}},
            {"source": "c.docx", "kind": "docx", "words": 4, "parts": 1,
             "dropped": {"personal": 0, "secret": 0}},
            {"source": "maps/d.png", "kind": "image"}])
        self.assertEqual(proc.stdout, "read 3 documents into 3 parts (16 words) in 1 chunks; "
                         "listed 1 images; skipped 0 files\ndropped 0 personal and 0 secret paragraphs\n")
        self.assertEqual(self.output.stat().st_mode & 0o777, 0o700)
        self.assertEqual(self.output.parent.stat().st_mode & 0o777, 0o700)
        for path in self.output.iterdir():
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertNotIn("IMAGE_BYTES_NEVER_READ", path.read_text())

    def test_filters_remove_paragraphs_and_redact_names_in_every_output(self):
        self.file("a.txt", "Keep the harbour.\n\nmy doctor moved the secret cove.\n\n"
                  "code 12345678 unlocks a gate.\n\nmy doctor knows 12345678 too.")
        self.file("my doctor.txt", "FILTERED_FILE_CONTENT")
        # An unreadable docx pins filtering before format parsing.
        self.file("12345678.docx", "FILTERED_BAD_ZIP")
        proc = self.ok(self.documents())
        self.assertEqual(self.parts(), [self.record("a.txt", 1, "Keep the harbour.")])
        self.assertEqual(self.rows("inventory.jsonl"), [
            {"source": "[filtered]", "kind": "skipped", "reason": "filtered"},
            {"source": "a.txt", "kind": "text", "words": 3, "parts": 1,
             "dropped": {"personal": 1, "secret": 2}},
            {"source": "[filtered]", "kind": "skipped", "reason": "filtered"}])
        for path in self.output.iterdir():
            for forbidden in ("my doctor", "12345678", "secret cove", "unlocks", "FILTERED_"):
                self.assertNotIn(forbidden, path.read_text())
        self.assertEqual(proc.stdout, "read 1 documents into 1 parts (3 words) in 1 chunks; "
                         "listed 0 images; skipped 2 files\ndropped 1 personal and 2 secret paragraphs\n")

    def test_junk_is_absent_and_file_and_directory_symlinks_are_not_followed(self):
        for name in (".hidden.txt", ".cache/nested.txt", "__MACOSX/nested.txt", "Thumbs.db",
                     "desktop.ini", "~$draft.docx", "nested/~$draft.txt"):
            self.file(name, "HIDDEN_CONTENT")
        target = self.h.root / "elsewhere"
        target.mkdir()
        (target / "outside.txt").write_text("SYMLINK_TARGET_CONTENT")
        (self.source / "a-link.txt").symlink_to(target / "outside.txt")
        (self.source / "b-link").symlink_to(target, target_is_directory=True)
        self.ok(self.documents())
        self.assertEqual(self.rows("inventory.jsonl"), [
            {"source": "a-link.txt", "kind": "skipped", "reason": "symlink"},
            {"source": "b-link", "kind": "skipped", "reason": "symlink"}])
        self.assertEqual(list(self.output.glob("documents-*.jsonl")), [])
        for path in self.output.iterdir():
            self.assertNotIn("SYMLINK_TARGET_CONTENT", path.read_text())
            self.assertNotIn("HIDDEN_CONTENT", path.read_text())

    def test_fifo_is_skipped_without_opening_and_regular_file_is_read(self):
        self.file("a.txt", "Keep the harbour.")
        os.mkfifo(self.source / "pipe.txt")
        self.ok(self.documents())
        self.assertEqual(self.parts(), [self.record("a.txt", 1, "Keep the harbour.")])
        self.assertEqual(self.rows("inventory.jsonl"), [
            {"source": "a.txt", "kind": "text", "words": 3, "parts": 1,
             "dropped": {"personal": 0, "secret": 0}},
            {"source": "pipe.txt", "kind": "skipped", "reason": "unreadable"}])

    def test_bad_docx_and_unsupported_audio_are_skipped(self):
        self.file("a.docx", "not a zip")
        self.docx("b.docx", '<!DOCTYPE document><document/>')
        self.docx("c.docx", '<!ENTITY hidden "text"><document/>')
        self.docx("d.docx", "<document>" + "x" * 100000 + "</document>", zipfile.ZIP_DEFLATED)
        with zipfile.ZipFile(self.source / "d.docx") as archive:
            info = archive.getinfo("word/document.xml")
            self.assertGreater(info.file_size, 100 * info.compress_size)
        with zipfile.ZipFile(self.source / "e.docx", "w") as archive:
            archive.writestr("unrelated.xml", "<document/>")
        self.docx("f.docx", "<broken")
        self.file("g.mp3", "AUDIO_BYTES_NEVER_READ")
        with (self.source / "h.txt").open("wb") as large:
            large.truncate(50 * 1024**2 + 1)
        self.ok(self.documents())
        self.assertEqual(self.rows("inventory.jsonl"), [
            *[{"source": name + ".docx", "kind": "skipped", "reason": "unreadable"}
              for name in "abcdef"],
            {"source": "g.mp3", "kind": "skipped", "reason": "unsupported type"},
            {"source": "h.txt", "kind": "skipped", "reason": "too large"}])
        self.assertEqual(self.parts(), [])

    def test_pdf_argv_and_form_feeds_are_paragraph_breaks(self):
        pdf = self.file("draft.pdf", "FAKE_PDF_BYTES")
        fake = self.h.root / "fake-pdftotext"
        fake.write_text('#!/bin/sh\n'
                        '[ "$#" = 5 ] && [ "$1" = -q ] && [ "$2" = -enc ] && '
                        '[ "$3" = UTF-8 ] && [ "$4" = "' + str(pdf) + '" ] && '
                        '[ "$5" = - ] || exit 8\n'
                        "printf 'First   coast.\\n\\nSecond harbour.\\fThird cliff.'\n")
        fake.chmod(0o700)
        self.ok(self.documents("--pdftotext", str(fake)))
        self.assertEqual(self.parts(), [self.record("draft.pdf", 1,
                         "First coast.\n\nSecond harbour.\n\nThird cliff.")])
        self.assertEqual(self.rows("inventory.jsonl"), [
            {"source": "draft.pdf", "kind": "pdf", "words": 6, "parts": 1,
             "dropped": {"personal": 0, "secret": 0}}])

    def test_missing_denied_and_failed_pdftotext_are_skips(self):
        self.file("draft.pdf", "FAKE_PDF_BYTES")
        denied = self.h.root / "denied"
        denied.write_text("#!/bin/sh\nexit 0\n")
        failed = self.h.root / "failed"
        failed.write_text("#!/bin/sh\nexit 1\n")
        failed.chmod(0o700)
        for program, reason in ((self.h.root / "missing", "no pdftotext (brew install poppler, "
                                 "or export it as .docx or .txt)"),
                                (denied, "no pdftotext (brew install poppler, or export it as .docx or .txt)"),
                                (failed, "unreadable")):
            with self.subTest(program=program.name):
                self.ok(self.documents("--pdftotext", str(program)))
                self.assertEqual(self.rows("inventory.jsonl"), [
                    {"source": "draft.pdf", "kind": "skipped", "reason": reason}])
                self.assertEqual(self.parts(), [])

    def test_part_limit_chunk_limit_and_rerun_removes_stale_chunks(self):
        paragraphs = [" ".join([word] * count) for word, count in
                      (("coast", 300), ("bridge", 200), ("market", 1), ("cliff", 501), ("sea", 2))]
        self.file("notes.markdown", "\n\n".join(paragraphs))
        self.ok(self.documents("--size", "501"))
        expected = [self.record("notes.markdown", n, text) for n, text in enumerate(
                    (paragraphs[0] + "\n\n" + paragraphs[1], paragraphs[2], paragraphs[3], paragraphs[4]), 1)]
        self.assertEqual(self.parts(), expected)
        self.assertEqual([len(r["text"].split()) for r in expected], [500, 1, 501, 2])
        self.assertEqual([self.rows(p.name) for p in sorted(self.output.glob("documents-*.jsonl"))],
                         [expected[:2], expected[2:3], expected[3:]])
        self.assertTrue((self.output / "documents-02.jsonl").exists())
        self.ok(self.documents("--size", "6000"))
        self.assertEqual([p.name for p in self.output.glob("documents-*.jsonl")], ["documents-01.jsonl"])
        self.assertEqual(self.parts(), expected)

    def test_all_filtered_document_counts_as_read_and_writes_no_chunks(self):
        self.file("draft.txt", "my doctor found a cove.")
        proc = self.ok(self.documents())
        self.assertEqual(proc.stdout, "read 1 documents into 0 parts (0 words) in 0 chunks; "
                         "listed 0 images; skipped 0 files\ndropped 1 personal and 0 secret paragraphs\n")
        self.assertEqual(self.parts(), [])
        self.assertEqual(self.rows("inventory.jsonl"), [
            {"source": "draft.txt", "kind": "text", "words": 0, "parts": 0,
             "dropped": {"personal": 1, "secret": 0}}])

    def test_output_inside_git_tree_is_refused_without_writes(self):
        repo = self.h.root / "fake-repo"
        repo.mkdir()
        (repo / ".git").write_text("gitdir: elsewhere\n")
        self.output = repo / "private" / "documents"
        proc = self.documents()
        self.assertEqual(proc.returncode, 1)
        assert_cli_error(self, proc.stderr, "crew_decisions:")
        self.assertIn("crew_decisions: out dir must not be inside a git work tree\n", proc.stderr)
        self.assertEqual(proc.stdout, "")
        self.assertEqual(sorted(p.name for p in repo.iterdir()), [".git"])

    def test_shingle_reads_document_chunk_text(self):
        self.file("notes.txt", "Always place the harbour beside the market.")
        self.ok(self.documents())
        chunk = str(self.output / "documents-01.jsonl")
        (self.h.root / "copied.md").write_text("Rule: place the harbour beside the market.")
        (self.h.root / "paraphrase.md").write_text("Trading belongs near the docks.")
        copied = self.h.crew(self.member, "shingle", "--doc", "copied.md", "--corpus", chunk)
        self.assertEqual(copied.returncode, 1, copied.stderr)
        self.assertIn("1 overlaps", copied.stdout)
        paraphrase = self.ok(self.h.crew(self.member, "shingle", "--doc", "paraphrase.md", "--corpus", chunk))
        self.assertEqual(paraphrase.stdout, "0 overlaps\n")


if __name__ == "__main__":
    unittest.main()
