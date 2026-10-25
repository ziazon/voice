#!/usr/bin/env python3
"""Pin the sources a principal adds beyond their Claude history: the registry in private.json and
its consent gate, sent mail from a local mbox (a Google Takeout export), the assistant's live log,
and project folders reviewed file by file for the lens.

Every fixture is made up here (Robin, Sam, Ada, example.com); no test reads a real mailbox, log or
folder.
"""
import json
import os
import stat
import sys
import time
import unittest
from email.message import EmailMessage
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from crew_decisions_test import ARI, Harness  # noqa: E402
from crew_fixtures import assert_cli_error  # noqa: E402

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "scripts"))
import crew_mail  # noqa: E402

CONSENT = "2026-10-25"


def mail(sender, to, date, body, labels="Sent", subject="Week 3 plan", html=False):
    """One message as mbox bytes: a From_ line, then the message with body From_ lines escaped."""
    message = EmailMessage()
    message["From"] = sender
    message["To"] = to
    message["Subject"] = subject
    if date is not None:
        message["Date"] = date
    if labels is not None:
        message["X-Gmail-Labels"] = labels
    if html:
        message.set_content(f"<p>{body}</p>", subtype="html")
    else:
        message.set_content(body)
    raw = message.as_bytes().replace(b"\nFrom ", b"\n>From ")
    return b"From 1234@xxx Mon Mar 02 10:00:00 +0000 2026\n" + raw + b"\n"


def raw_mail(headers, body):
    lines = "".join(f"{k}: {v}\n" for k, v in headers.items())
    return b"From 1234@xxx Mon Mar 02 10:00:00 +0000 2026\n" + lines.encode() + b"\n" + body + b"\n\n"


ROBIN = "Robin Lee <Robin@Example.com>"
SAM = "Sam <sam@example.org>"

SENT = """Keep the partner reading first in week 3, and move the quiz to Friday.

Ask me before you change the rubric.

--
Robin
Department of Something

On Mon, Mar 2, 2026 at 9:00 AM Sam <sam@example.org> wrote:
> Can we move the quiz?
"""

INTERLEAVED = """> Should week 4 have a lab?
Yes, a short lab, no more than an hour.

On Tue, Mar 3, 2026 at 9:00 AM Sam
<sam@example.org> wrote:
> thanks
"""

OUTLOOK = """Grade the drafts on structure first.

Sent from my iPhone

-----Original Message-----
From: Sam
Sent: Tuesday
Subject: drafts
"""

OUTLOOK_HEADERS = """Use the shorter reading list.

________________________________
From: Sam <sam@example.org>
Sent: Wednesday, March 4, 2026
To: Robin
"""

NO_DELIMITER = """Keep the quiz on Friday.

Best,
Robin Lee
Department of Something
"""

LONG_ATTRIBUTION = """Yes, go ahead.

On Tue, Mar 3, 2026 at 9:00 AM Sam Somebody Longname
of the Department of Things
wrote:
> can I?
"""

OTHER_LANGUAGE = """Ja, mach das.

Am 03.03.2026 um 09:00 schrieb Sam Lee:
> darf ich?
"""

MIXED = """Send the slides to ada@example.org before class.

Start the session with the case study, not the lecture.
"""


class Sources(unittest.TestCase):
    """A harness with Ari's member config and a private.json beside it."""

    def setUp(self):
        self.h = Harness()
        self.addCleanup(self.h.dir.cleanup)
        self.member = self.h.member(ARI)
        self.state = self.h.root / "state"

    def private(self, *sources, **extra):
        (self.h.root / "private.json").write_text(json.dumps({"sources": list(sources), **extra}))

    def crew(self, *args):
        return self.h.crew(self.member, *args)

    def ok(self, *args):
        proc = self.crew(*args)
        self.assertEqual(proc.returncode, 0, proc.stderr)
        return proc

    def rows(self, path):
        return [json.loads(line) for line in Path(path).read_text().splitlines()]


class Registry(Sources):
    def test_list_names_each_source_its_kind_and_consent_but_no_path_or_address(self):
        box = self.h.root / "mail.mbox"
        self.private({"name": "mail", "kind": "mbox", "path": str(box), "own_addresses": ["robin@example.com"],
                      "consent": CONSENT},
                     {"name": "teaching", "kind": "folder", "path": str(self.h.root / "teaching")},
                     {"name": "live", "kind": "live", "consent": CONSENT})
        out = self.ok("sources", "list").stdout
        self.assertEqual(out.splitlines(), [
            f"mail\tmbox\tconsent {CONSENT}",
            "teaching\tfolder\tno consent yet: not read",
            f"live\tlive\tconsent {CONSENT}"])
        self.assertNotIn("example.com", out)
        self.assertNotIn(str(self.h.root), out)

    def test_no_sources_is_not_an_error(self):
        out = self.ok("sources", "list").stdout
        self.assertEqual(out.strip(), "no sources in private.json")

    def test_a_bad_entry_is_a_plain_error_naming_the_source_and_key_never_a_value(self):
        good = {"name": "mail", "kind": "mbox", "path": "/x.mbox", "own_addresses": ["robin@example.com"]}
        cases = {
            "unknown key": ([{**good, "consnet": CONSENT}], "sources[0] (mail): unknown key consnet"),
            "bad name": ([{**good, "name": "My Mail"}], "sources[0]: name must be"),
            "duplicate": ([good, dict(good)], "sources[1] (mail): name is used twice"),
            "unknown kind": ([{**good, "kind": "pst"}], "sources[0] (mail): kind must be one of folder, live, mbox"),
            "two live": ([{"name": "a", "kind": "live"}, {"name": "b", "kind": "live"}],
                         "sources[1] (b): only one live source"),
            "no addresses": ([{**good, "own_addresses": []}],
                             "sources[0] (mail): own_addresses must be a non-empty list of addresses"),
            "no path": ([{k: v for k, v in good.items() if k != "path"}], "sources[0] (mail): path is required"),
            "bad consent": ([{**good, "consent": "yes"}], "sources[0] (mail): consent must be a YYYY-MM-DD date"),
            "bad regex": ([{**good, "exclude_to": "("}], "sources[0] (mail): exclude_to is not a valid regex"),
            "bad after": ([{**good, "after": "March"}], "sources[0] (mail): after must be a YYYY-MM-DD date"),
            "bad globs": ([{"name": "f", "kind": "folder", "path": "/f", "include": "*.md"}],
                          "sources[0] (f): include must be a list of strings"),
            "not a list": ({"name": "mail"}, "sources must be a list"),
        }
        for label, (sources, message) in cases.items():
            with self.subTest(label):
                (self.h.root / "private.json").write_text(json.dumps({"sources": sources}))
                proc = self.crew("sources", "list")
                self.assertEqual(proc.returncode, 1)
                assert_cli_error(self, proc.stderr, "crew_decisions: ")
                self.assertIn(message, proc.stderr)
                self.assertNotIn("robin@example.com", proc.stderr)

    def test_sources_in_the_committed_member_json_are_refused(self):
        member = self.h.member({**ARI, "sources": [{"name": "live", "kind": "live"}]}, name="other.json")
        proc = self.h.crew(member, "sources", "list")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("sources belong in private.json", proc.stderr)

    def test_withdrawn_or_removed_consent_deletes_what_was_mined(self):
        box = self.h.root / "mail.mbox"
        box.write_bytes(mail(ROBIN, SAM, "Mon, 02 Mar 2026 10:00:00 +0000", "Keep the quiz on Friday."))
        self.state.mkdir(parents=True, exist_ok=True)
        (self.state / "live.jsonl").write_text(json.dumps(
            {"timestamp": "2026-10-20T10:00:00Z", "question": "Q?", "ari_pick": "A", "robin_answer": "B"}) + "\n")
        entry = {"name": "mail", "kind": "mbox", "path": str(box), "own_addresses": ["robin@example.com"]}
        self.private({**entry, "consent": CONSENT}, {"name": "live", "kind": "live", "consent": CONSENT})
        self.ok("sources", "mine")
        self.assertTrue((self.state / "decisions-src-mail.jsonl").exists())
        self.assertTrue((self.state / "decisions-live.jsonl").exists())
        (self.state / "decisions-src-old.jsonl").write_text("")
        self.private(entry)
        out = self.ok("sources", "mine").stdout
        self.assertFalse((self.state / "decisions-src-mail.jsonl").exists())
        self.assertFalse((self.state / "decisions-src-old.jsonl").exists())
        self.assertFalse((self.state / "decisions-live.jsonl").exists())
        self.assertIn("removed decisions-src-mail.jsonl: its source has no consent", out)
        self.assertTrue((self.state / "live.jsonl").exists())

    def test_mine_reads_nothing_without_consent(self):
        box = self.h.root / "mail.mbox"
        box.write_bytes(mail(ROBIN, SAM, "Mon, 02 Mar 2026 10:00:00 +0000", "Keep the quiz on Friday."))
        self.private({"name": "mail", "kind": "mbox", "path": str(box), "own_addresses": ["robin@example.com"]})
        out = self.ok("sources", "mine").stdout
        self.assertIn("mail: no consent yet, not read", out)
        self.assertFalse((self.state / "decisions-src-mail.jsonl").exists())


class Mail(Sources):
    def mine(self, messages, **source):
        box = self.h.root / "mail.mbox"
        box.write_bytes(b"".join(messages))
        self.private({"name": "mail", "kind": "mbox", "path": str(box), "own_addresses": ["robin@example.com"],
                      "consent": CONSENT, **source})
        proc = self.ok("sources", "mine")
        out = self.state / "decisions-src-mail.jsonl"
        return proc.stdout, (self.rows(out) if out.exists() else [])

    def test_keeps_only_sent_mail_stripped_of_headers_quotes_and_signatures(self):
        stdout, records = self.mine([
            mail(ROBIN, SAM, "Mon, 02 Mar 2026 10:00:00 -0500", SENT, labels="Sent,Opened"),
            mail(SAM, ROBIN, "Mon, 02 Mar 2026 09:00:00 -0500", "Can we move the quiz?", labels="Inbox"),
            mail(ROBIN, SAM, "Mon, 02 Mar 2026 11:00:00 -0500", "half a thought", labels="Drafts"),
            mail(ROBIN, SAM, "Mon, 02 Mar 2026 11:10:00 -0500", "thrown away", labels="Sent,Trash"),
            mail(ROBIN, SAM, "Mon, 02 Mar 2026 11:20:00 -0500", "flagged", labels="Spam,Sent"),
            mail(ROBIN, ROBIN, "Mon, 02 Mar 2026 12:00:00 -0500", "a note to myself", labels="Inbox"),
            mail(ROBIN, SAM, "Tue, 03 Mar 2026 10:00:00 +0000", INTERLEAVED, labels=None),
        ])
        self.assertEqual([r["text"] for r in records], [
            "Keep the partner reading first in week 3, and move the quiz to Friday.\n\n"
            "Ask me before you change the rubric.",
            "Yes, a short lab, no more than an hour."])
        first = records[0]
        self.assertEqual(first["kind"], "message")
        self.assertEqual(first["timestamp"], "2026-03-02T15:00:00.000Z")
        self.assertEqual(first["project"], "mail")
        self.assertEqual(first["context"], "")
        self.assertEqual(first["split"], "train")
        self.assertTrue(first["id"].startswith("message:"))
        written = (self.state / "decisions-src-mail.jsonl").read_text()
        for value in ("Week 3 plan", "sam@example.org", "Sam", "Department", "Can we move", "half a thought",
                      "note to myself", "Robin", "thrown away", "flagged"):
            self.assertNotIn(value, written)
        self.assertIn("mail: read 7 messages; kept 2 you sent", stdout)
        self.assertIn("1 not from you", stdout)
        self.assertIn("4 not in Sent", stdout)
        for value in ("partner", "quiz", "Sam", "example"):
            self.assertNotIn(value, stdout)

    def test_outlook_quotes_phone_sign_offs_and_separators_are_cut(self):
        _, records = self.mine([
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 10:00:00 +0000", OUTLOOK),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 11:00:00 +0000", OUTLOOK_HEADERS),
        ])
        self.assertEqual([r["text"] for r in records],
                         ["Grade the drafts on structure first.", "Use the shorter reading list."])

    def test_closings_without_a_delimiter_and_any_attribution_line_are_cut(self):
        _, records = self.mine([
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 12:00:00 +0000", NO_DELIMITER),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 13:00:00 +0000", LONG_ATTRIBUTION),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 14:00:00 +0000", OTHER_LANGUAGE),
        ])
        self.assertEqual([r["text"] for r in records],
                         ["Keep the quiz on Friday.", "Yes, go ahead.", "Ja, mach das."])

    def test_an_opening_thanks_is_kept_and_a_long_closing_cut(self):
        _, records = self.mine([
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 15:00:00 +0000", "Thanks!\n\nI looked at the draft. Keep the lab short."),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 16:00:00 +0000", "Keep the lab short.\n\nYours sincerely,\nRobin Lee"),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 17:00:00 +0000", "Sent from my desk at 9: we keep the lab."),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 18:00:00 +0000",
                 "Yes.\n\nThanks!\n\nAlso, the room is 4B.\n\nBest,\nRobin"),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 19:00:00 +0000",
                 "Keep the lab short.\n\nBest,\nRobin\n\nPS: the quiz moves to Monday."),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 20:00:00 +0000", "Keep it.\n\nBest,\nRobin Lee\nPS 118 Harbour"),
            mail(ROBIN, SAM, "Wed, 04 Mar 2026 21:00:00 +0000",
                 "Keep it short.\n\nBest,\nRobin\n\nP.S. " + "the long note goes on. " * 20),
        ])
        self.assertEqual([r["text"] for r in records], [
            "Thanks!\n\nI looked at the draft. Keep the lab short.", "Keep the lab short.",
            "Sent from my desk at 9: we keep the lab.", "Yes.\n\nThanks!\n\nAlso, the room is 4B.",
            "Keep the lab short.\n\nPS: the quiz moves to Monday.", "Keep it.",
            "Keep it short.\n\nP.S. " + ("the long note goes on. " * 20).strip()])

    def test_other_writers_from_lines_split_and_a_from_line_inside_a_body_is_counted(self):
        def sent(hour):
            return (b"From: Robin <robin@example.com>\nDate: Mon, 02 Mar 2026 %d:00:00 +0000\n"
                    b"X-Gmail-Labels: Sent\n\n" % hour)
        stdout, records = self.mine([
            b"From robin@example.com Mon Mar 02 10:00:00 2026 -0500 (EST)\n" + sent(10) + b"Zone at the end.\n",
            b"From robin@example.com Mon Mar 02 11:00:00 2026\n" + sent(11) + b"No blank line after me.\n",
            b"From robin@example.com Mon Mar 02 12:00:00 2026\n" + sent(12) + b"A third one.\n\n"
            b"From here on we read chapter two.\n\n"
            b"From someone@example.net Mon Mar 02 13:00:00 2026\nnot a header line\n\n",
        ])
        self.assertEqual([r["text"].split("\n\n")[0] for r in records],
                         ["Zone at the end.", "No blank line after me.", "A third one."])
        self.assertIn("read 3 messages", stdout)
        self.assertIn("1 From lines left inside a message", stdout)

    def test_a_fifo_at_the_mbox_path_is_refused_without_hanging(self):
        fifo = self.h.root / "mail.fifo"
        os.mkfifo(fifo)
        self.private({"name": "mail", "kind": "mbox", "path": str(fifo),
                      "own_addresses": ["robin@example.com"], "consent": CONSENT})
        proc = self.crew("sources", "mine")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("mail: the mbox must be a regular file, not a link", proc.stderr)

    def test_a_paragraph_that_trips_a_filter_is_dropped_and_the_rest_kept(self):
        stdout, records = self.mine([mail(ROBIN, SAM, "Thu, 05 Mar 2026 10:00:00 +0000", MIXED)])
        self.assertEqual([r["text"] for r in records],
                         ["Start the session with the case study, not the lecture."])
        self.assertIn("dropped 0 personal and 1 secret paragraphs", stdout)

    def test_skips_html_only_undated_unreadable_too_long_and_empty_mail(self):
        stdout, records = self.mine([
            mail(ROBIN, SAM, "Thu, 05 Mar 2026 10:00:00 +0000", "only markup", html=True),
            mail(ROBIN, SAM, None, "no date at all"),
            raw_mail({"From": ROBIN, "To": SAM, "Date": "not a date", "X-Gmail-Labels": "Sent",
                      "Content-Type": "text/plain"}, b"a bad date"),
            raw_mail({"From": ROBIN, "To": SAM, "Date": "Thu, 05 Mar 2026 10:00:00 +0000",
                      "X-Gmail-Labels": "Sent", "Content-Type": "text/plain; charset=x-no-such-charset"},
                     b"odd bytes \xff\xfe"),
            mail(ROBIN, SAM, "Thu, 05 Mar 2026 11:00:00 +0000", "word " * 1000),
            mail(ROBIN, SAM, "Thu, 05 Mar 2026 12:00:00 +0000", "> only a quote\n"),
            mail(ROBIN, SAM, "Thu, 05 Mar 2026 13:00:00 +0000", "From the top: keep the warm-up."),
        ])
        self.assertEqual([r["text"] for r in records], ["From the top: keep the warm-up."])
        for line in ("1 with no plain text", "2 undated", "1 unreadable", "1 too long", "1 empty once stripped"):
            self.assertIn(line, stdout)

    def test_a_crafted_message_is_counted_and_the_rest_still_read(self):
        stdout, records = self.mine([
            raw_mail({"From": "eve@example.net " + "(" * 5000, "Date": "Mon, 02 Mar 2026 10:00:00 +0000"}, b"x"),
            raw_mail({"From": ROBIN, "Date": "Mon, 02 Mar 2026 10:00:00 +0000",
                      "X-Pad": "a" * 300 * 1024}, b"too much header"),
            mail(ROBIN, SAM, "Mon, 02 Mar 2026 11:00:00 +0000", "Keep the quiz on Friday."),
        ])
        self.assertEqual([r["text"] for r in records], ["Keep the quiz on Friday."])
        self.assertIn("2 unreadable", stdout)

    def test_an_unescaped_from_line_in_a_body_does_not_start_a_forged_message(self):
        forged = (b"From 1234@xxx Mon Mar 02 10:00:00 +0000 2026\nFrom: Sam <sam@example.org>\n"
                  b"Date: Mon, 02 Mar 2026 09:00:00 +0000\n\nhello\nFrom x\nX-Gmail-Labels: Sent\n"
                  b"From: robin@example.com\nDate: Mon, 02 Mar 2026 09:00:00 +0000\n\nFORGED words\n\n")
        stdout, records = self.mine([forged])
        self.assertEqual(records, [])
        self.assertIn("read 1 messages", stdout)

    def test_an_unreadable_mbox_error_names_no_path(self):
        box = self.h.root / "secret-folder-name"
        box.mkdir()
        (box / "mail.mbox").write_bytes(b"")
        os.chmod(box / "mail.mbox", 0)
        self.addCleanup(os.chmod, box / "mail.mbox", 0o600)
        self.private({"name": "mail", "kind": "mbox", "path": str(box / "mail.mbox"),
                      "own_addresses": ["robin@example.com"], "consent": CONSENT})
        proc = self.crew("sources", "mine")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("mail: cannot read the source (Permission denied)", proc.stderr)
        self.assertNotIn("secret-folder-name", proc.stderr)

    def test_a_message_that_parses_too_long_is_cut_off_and_an_earlier_timer_survives(self):
        import signal
        fired = []
        previous = signal.signal(signal.SIGALRM, lambda *_: fired.append(1))
        self.addCleanup(signal.signal, signal.SIGALRM, previous)
        signal.setitimer(signal.ITIMER_REAL, 30)
        self.addCleanup(signal.setitimer, signal.ITIMER_REAL, 0)
        with self.assertRaises(crew_mail.Unreadable):
            with crew_mail.budget(0.05):
                while True:
                    pass
        self.assertGreater(signal.getitimer(signal.ITIMER_REAL)[0], 25)

    def test_an_over_long_mail_is_skipped_before_the_filters_run(self):
        started = time.monotonic()
        stdout, records = self.mine([mail(ROBIN, SAM, "Mon, 02 Mar 2026 10:00:00 +0000",
                                          "Keep the quiz on Friday.\n\n" + "a" * 120000)])
        self.assertLess(time.monotonic() - started, 5)
        self.assertEqual(records, [])
        self.assertIn("1 too long", stdout)

    def test_recipients_and_dates_narrow_what_is_read(self):
        stdout, records = self.mine([
            mail(ROBIN, "Ada <ada@example.net>", "Mon, 02 Mar 2026 10:00:00 +0000", "a family plan"),
            mail(ROBIN, SAM, "Mon, 02 Mar 2026 10:00:00 +0000", "Keep the quiz on Friday.", labels="Sent"),
            mail(ROBIN, SAM, "Mon, 02 Dec 2024 10:00:00 +0000", "an old term"),
        ], exclude_to=r"@example\.net$", after="2025-01-01")
        self.assertEqual([r["text"] for r in records], ["Keep the quiz on Friday."])
        self.assertIn("1 to excluded recipients", stdout)
        self.assertIn("1 outside the dates", stdout)

    def test_the_file_is_private_and_a_link_or_missing_mbox_is_a_plain_error(self):
        self.mine([mail(ROBIN, SAM, "Mon, 02 Mar 2026 10:00:00 +0000", "Keep the quiz on Friday.")])
        mode = stat.S_IMODE(os.stat(self.state / "decisions-src-mail.jsonl").st_mode)
        self.assertEqual(mode, 0o600)
        link = self.h.root / "link.mbox"
        link.symlink_to(self.h.root / "mail.mbox")
        for path, message in ((link, "mail: the mbox must be a regular file, not a link"),
                              (self.h.root / "gone.mbox", "mail: no mbox at the source's path")):
            with self.subTest(path.name):
                self.private({"name": "mail", "kind": "mbox", "path": str(path),
                              "own_addresses": ["robin@example.com"], "consent": CONSENT})
                proc = self.crew("sources", "mine")
                self.assertEqual(proc.returncode, 1)
                assert_cli_error(self, proc.stderr, "crew_decisions: ")
                self.assertIn(message, proc.stderr)


class Live(Sources):
    def log(self, *lines):
        self.state.mkdir(parents=True, exist_ok=True)
        (self.state / "live.jsonl").write_text("".join(
            (line if isinstance(line, str) else json.dumps(line)) + "\n" for line in lines))

    def entry(self, when, question, pick, answer=None, kind="escalated"):
        line = {"timestamp": when, "work": "plans/x.md", "kind": kind, "category": "scope", "tier": "medium",
                "question": question, "ari_pick": pick, "rule": None, "robin_answer": answer}
        return line

    def test_answered_lines_become_training_questions_and_the_rest_are_skipped(self):
        self.log(self.entry("2026-10-20T10:00:00Z", "Cut week 6?", "Keep it", "Keep it"),
                 self.entry("2026-10-21T10:00:00Z", "Which reading first?", "The survey", "the case study"),
                 self.entry("2026-10-22T10:00:00Z", "Move the exam?", "Yes", None, kind="decided"),
                 self.entry("2026-10-22T11:00:00Z", "Already asked in Claude Code?", "A", "B"),
                 "not json")
        (self.state / "decisions-mac.jsonl").write_text(json.dumps(
            {"id": "question:abc", "kind": "question", "question": "Already  asked in Claude code?",
             "timestamp": "2026-10-22T11:00:00.000Z"}) + "\n")
        self.private({"name": "live", "kind": "live", "consent": CONSENT})
        stdout = self.ok("sources", "mine").stdout
        records = self.rows(self.state / "decisions-live.jsonl")
        self.assertEqual(len(records), 2)
        took, own = records
        self.assertEqual(took["kind"], "question")
        self.assertEqual(took["timestamp"], "2026-10-20T10:00:00.000Z")
        self.assertEqual(took["options"], [{"label": "Keep it (Recommended)", "description": ""}])
        self.assertEqual(took["recommended"], "Keep it (Recommended)")
        self.assertEqual(took["picks"], ["Keep it (Recommended)"])
        self.assertEqual((took["own_words"], took["outcome"], took["split"]), ("", "took_rec", "train"))
        self.assertEqual(took["ari_pick"], "Keep it (Recommended)")
        self.assertEqual((own["picks"], own["own_words"], own["outcome"]), ([], "the case study", "own_words"))
        self.assertEqual(own["project"], "live")
        self.assertNotIn("plans/x.md", (self.state / "decisions-live.jsonl").read_text())
        self.assertIn("live: kept 2 answers; skipped 1 with no answer from you, 1 already in the history, "
                      "1 unreadable lines", stdout)

    def test_no_log_yet_is_not_an_error(self):
        self.private({"name": "live", "kind": "live", "consent": CONSENT})
        self.assertIn("live: no log yet", self.ok("sources", "mine").stdout)


class Folder(Sources):
    def setUp(self):
        super().setUp()
        self.folder = self.h.root / "teaching"
        for name, text in {"syllabus.md": "Week one opens with a story.",
                           "notes/week2.txt": "Discussion before lecture.",
                           "AGENTS.md": "agent rules", "node_modules/pkg/readme.md": "vendored",
                           "drafts/plan.md": "Claude drafted this plan.", "drafts/edited.md": "touched up",
                           "skip/old.md": "an excluded file", "my doctor.md": "filtered name",
                           "image.png": ""}.items():
            path = self.folder / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        (self.folder / "tab\tname.md").write_text("odd")
        self.h.session("-Users-p-teaching", "s1",
                       self.tool("Write", self.folder / "drafts/plan.md"),
                       self.tool("Edit", self.folder / "drafts/edited.md"))
        self.source = {"name": "teaching", "kind": "folder", "path": str(self.folder),
                       "exclude": ["skip/**"], "consent": CONSENT}
        self.private(self.source)
        self.review = self.state / "sources" / "teaching-review.tsv"

    @staticmethod
    def tool(name, path):
        return {"type": "assistant", "timestamp": "2026-10-01T10:00:00Z",
                "message": {"role": "assistant", "content": [
                    {"type": "tool_use", "id": "t", "name": name, "input": {"file_path": str(path)}}]}}

    def table(self):
        lines = self.review.read_text().splitlines()
        header = lines[0].split("\t")
        self.assertEqual(header, ["mine", "modified", "kind", "claude", "path"])
        return {row["path"]: row for row in (dict(zip(header, line.split("\t"))) for line in lines[1:])}

    def review_files(self):
        return self.ok("sources", "review", "--name", "teaching", "--projects", str(self.h.projects))

    def test_review_lists_files_with_claude_written_and_agent_files_marked_not_mine(self):
        stdout = self.review_files().stdout
        rows = self.table()
        self.assertEqual(sorted(rows), ["AGENTS.md", "drafts/edited.md", "drafts/plan.md", "image.png",
                                        "node_modules/pkg/readme.md", "notes/week2.txt", "syllabus.md"])
        self.assertEqual({p: (r["mine"], r["claude"]) for p, r in rows.items()}, {
            "AGENTS.md": ("n", "agent"), "drafts/edited.md": ("n", "edited"), "drafts/plan.md": ("n", "wrote"),
            "image.png": ("", ""), "node_modules/pkg/readme.md": ("n", "agent"), "notes/week2.txt": ("", ""),
            "syllabus.md": ("", "")})
        self.assertEqual(rows["syllabus.md"]["kind"], "text")
        self.assertRegex(rows["syllabus.md"]["modified"], r"^\d{4}-\d{2}-\d{2}$")
        self.assertEqual(stat.S_IMODE(os.stat(self.review).st_mode), 0o600)
        self.assertIn("teaching: listed 7 files; 4 start as not mine (2 Claude wrote or edited, 2 agent files); "
                      "skipped 1 excluded, 1 filtered names, 1 odd names", stdout)
        self.assertNotIn("Week one", stdout)

    def test_marks_by_path_glob_survive_a_rerun(self):
        self.review_files()
        self.ok("sources", "mark", "--name", "teaching", "--as", "y", "--path", "*.md", "--path", "notes/**")
        self.ok("sources", "mark", "--name", "teaching", "--as", "y", "--path", "drafts/edited.md")
        self.review_files()
        marks = {p: r["mine"] for p, r in self.table().items()}
        self.assertEqual(marks["syllabus.md"], "y")
        self.assertEqual(marks["notes/week2.txt"], "y")
        self.assertEqual(marks["AGENTS.md"], "y")
        self.assertEqual(marks["drafts/edited.md"], "y")
        self.assertEqual(marks["drafts/plan.md"], "n")
        self.assertEqual(marks["image.png"], "")
        proc = self.crew("sources", "mark", "--name", "teaching", "--as", "y")
        self.assertEqual(proc.returncode, 1)
        self.assertIn("mark needs --path, --unmarked or --all", proc.stderr)

    def test_documents_reads_only_files_marked_mine_named_by_source(self):
        self.review_files()
        self.ok("sources", "mark", "--name", "teaching", "--as", "y", "--path", "syllabus.md",
                "--path", "notes/**")
        raw = self.h.root / "raw"
        raw.mkdir()
        (raw / "essay.md").write_text("Plan the ending first.")
        out = self.h.root / "lens-in"
        stdout = self.ok("documents", "--dir", str(raw), "--sources", "--out-dir", str(out)).stdout
        parts = [json.loads(line) for line in (out / "documents-01.jsonl").read_text().splitlines()]
        self.assertEqual([(p["source"], p["text"]) for p in parts], [
            ("essay.md", "Plan the ending first."),
            ("teaching/notes/week2.txt", "Discussion before lecture."),
            ("teaching/syllabus.md", "Week one opens with a story.")])
        self.assertIn("read 3 documents", stdout)
        only = self.h.root / "lens-only"
        self.ok("documents", "--sources", "--out-dir", str(only))
        self.assertEqual(len((only / "documents-01.jsonl").read_text().splitlines()), 2)

    def test_documents_reads_nothing_once_consent_is_withdrawn(self):
        self.review_files()
        self.ok("sources", "mark", "--name", "teaching", "--as", "y", "--all")
        self.private({k: v for k, v in self.source.items() if k != "consent"})
        out = self.h.root / "lens-in"
        stdout = self.ok("documents", "--sources", "--out-dir", str(out)).stdout
        self.assertIn("teaching: no consent yet, not read", stdout)
        self.assertIn("read 0 documents", stdout)
        self.assertEqual(list(out.glob("documents-*.jsonl")), [])

    def test_a_hand_edited_review_never_reaches_outside_the_folder(self):
        outside = self.h.root / "outside"
        outside.mkdir()
        (outside / "secret.md").write_text("OUTSIDE_TEXT")
        (self.folder / "link.md").symlink_to(outside / "secret.md")
        (self.folder / "linkdir").symlink_to(outside)
        self.review_files()
        with self.review.open("a") as review:
            for path in ("../outside/secret.md", str(outside / "secret.md"), "link.md", "linkdir/secret.md"):
                review.write(f"y\t2026-10-25\ttext\t\t{path}\n")
        out = self.h.root / "lens-in"
        self.ok("documents", "--sources", "--out-dir", str(out))
        written = "".join(p.read_text() for p in out.glob("*.jsonl"))
        self.assertNotIn("OUTSIDE_TEXT", written)

    def test_review_says_so_when_there_are_no_transcripts_and_honours_claude_projects(self):
        stdout = self.ok("sources", "review", "--name", "teaching", "--projects", str(self.h.root / "none")).stdout
        self.assertIn("no Claude Code transcripts there: no file starts as not mine for being Claude's", stdout)
        proc = self.h.run(Path(__file__).resolve().parent.parent / "scripts" / "crew_decisions.py",
                          "--member", self.member, "sources", "review", "--name", "teaching",
                          env={"CLAUDE_PROJECTS": str(self.h.projects)})
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(self.table()["drafts/plan.md"]["claude"], "wrote")

    def test_patterns_match_folder_by_folder_without_runaway_backtracking(self):
        deep = self.folder / ("a" * 20) / ("a" * 20) / ("a" * 20)
        deep.mkdir(parents=True)
        (deep / "note.md").write_text("deep")
        self.review_files()
        started = time.monotonic()
        out = self.ok("sources", "mark", "--name", "teaching", "--as", "y", "--path", "**a" * 8 + "z",
                      "--path", "**/" * 10 + "x*a*a*a*a*a*a*a*z").stdout
        self.assertLess(time.monotonic() - started, 10)
        self.assertIn("marked 0 files", out)
        self.ok("sources", "mark", "--name", "teaching", "--as", "y", "--path", "**/" * 10 + "syllabus.md",
                "--path", "**/note.md")
        marks = {p: r["mine"] for p, r in self.table().items()}
        self.assertEqual(marks["syllabus.md"], "y")
        self.assertEqual(marks["/".join(["a" * 20] * 3) + "/note.md"], "y")

    def test_review_of_an_unreadable_folder_keeps_the_marks(self):
        self.review_files()
        self.ok("sources", "mark", "--name", "teaching", "--as", "y", "--path", "syllabus.md")
        os.chmod(self.folder, 0)
        self.addCleanup(os.chmod, self.folder, 0o700)
        proc = self.crew("sources", "review", "--name", "teaching", "--projects", str(self.h.projects))
        self.assertEqual(proc.returncode, 1)
        self.assertIn("teaching: cannot read the folder", proc.stderr)
        os.chmod(self.folder, 0o700)
        self.assertEqual(self.table()["syllabus.md"]["mine"], "y")
        os.chmod(self.folder / "notes", 0)
        self.addCleanup(os.chmod, self.folder / "notes", 0o700)
        proc = self.crew("sources", "review", "--name", "teaching", "--projects", str(self.h.projects))
        self.assertEqual(proc.returncode, 1)
        self.assertIn("cannot read a source file (Permission denied)", proc.stderr)
        os.chmod(self.folder / "notes", 0o700)
        self.assertIn("notes/week2.txt", self.table())

    def test_documents_needs_a_dir_or_sources_and_skips_a_source_with_no_review(self):
        proc = self.crew("documents", "--out-dir", str(self.h.root / "x"))
        self.assertEqual(proc.returncode, 1)
        self.assertIn("documents needs --dir, --sources or both", proc.stderr)
        stdout = self.ok("documents", "--sources", "--out-dir", str(self.h.root / "y")).stdout
        self.assertIn("teaching: no review yet (sources review --name teaching)", stdout)

    def test_review_needs_consent_and_a_folder_kind(self):
        self.private({k: v for k, v in self.source.items() if k != "consent"},
                     {"name": "live", "kind": "live", "consent": CONSENT})
        for name, message in (("teaching", "teaching: no consent yet, not read"),
                              ("live", "live: review is for folder sources"),
                              ("nope", "no source named nope")):
            with self.subTest(name):
                proc = self.crew("sources", "review", "--name", name, "--projects", str(self.h.projects))
                self.assertEqual(proc.returncode, 1)
                self.assertIn(message, proc.stderr)
                self.assertFalse(self.review.exists())

    def test_mine_leaves_folders_to_the_lens(self):
        self.assertIn("teaching: a folder feeds the lens (documents --sources)", self.ok("sources", "mine").stdout)


if __name__ == "__main__":
    unittest.main()
