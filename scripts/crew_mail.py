"""Read sent mail from an mbox without filters, member config or output writes.

An mbox (Google Takeout's mail export, or any other) is split into messages here, read-only and
streamed. Each message yields its sender, recipients, Gmail labels, date and the plain text the
sender wrote, with quoted replies, forwarded headers and signatures cut away.
"""
import contextlib
import email
import email.errors
import re
import signal
import time
from datetime import timezone
from email import policy
from email.utils import getaddresses, parsedate_to_datetime

MAX_MESSAGE_BYTES = 50 * 1024**2
MAX_HEADER_BYTES = 256 * 1024
UNREADABLE = (ValueError, LookupError, TypeError, AttributeError, IndexError, RecursionError,
              email.errors.MessageError)
# A From_ line as mbox writers put it: an address (or `-`), then a date with a time of day; and
# the header line every message starts with.
FROM_LINE = re.compile(rb"From \S+ +\S.*\b\d{1,2}:\d{2}")
HEADER_LINE = re.compile(rb"[!-9;-~]+:")
CLOSING = re.compile(r"^(?:best|best wishes|best regards|kind regards|warm regards|regards|warmly|thanks|"
                     r"thank you|many thanks|cheers|sincerely|yours sincerely|sincerely yours|yours truly|"
                     r"all the best|take care|talk soon|yours)[ \t]*[,.!]?[ \t]*$", re.M | re.I)
SIGNATURE_CHARS = 300
PS = re.compile(r"^(?:P\.S\.|P\.?P?S:)", re.M)

# Everything from the first of these lines on is someone else's words or a signature.
CUT = [re.compile(p, re.M | re.I) for p in (
    r"^On\b[^\n]*(?:\n[^\n]*){0,2}\bwrote:[ \t]*$",       # Gmail and Apple Mail, up to three lines
    r"^-{2,}[ \t]*Original Message[ \t]*-{2,}",           # Outlook
    r"^-{2,}[ \t]*Forwarded message[ \t]*-{2,}",          # a forward
    r"^_{10,}[ \t]*$",                                    # Outlook's rule above the quoted headers
    r"^From:[^\n]*\n(?:[^\n]*\n){0,3}?(?:Sent|Date):",    # quoted headers with no rule above them
    r"^-- ?$",                                            # the signature delimiter
)]
SIGN_OFF = re.compile(r"^Sent from my [^\n:]{0,40}$", re.M)


class Unreadable(Exception):
    pass


def messages(box):
    """Each message from an open binary file, in file order, as (its bytes, or None past the size
    cap; how many From_-shaped lines were left inside it). A message starts at the file's first
    `From ` line, then only at a From_-shaped line whose next line is a header, so a writer that
    leaves no blank line still splits, and a From_-shaped line in prose is counted, never silently
    split on or merged. A body line escaped as `>From ` is unescaped one level."""
    current, size, too_big, inside = None, 0, False, 0
    line = box.readline()
    while line:
        following = box.readline()
        if line.startswith(b"From ") and (current is None or
                                          (FROM_LINE.match(line) and HEADER_LINE.match(following))):
            if current is not None:
                yield (None if too_big else b"".join(current)), inside
            current, size, too_big, inside = [], 0, False, 0
        elif current is not None:
            inside += bool(FROM_LINE.match(line))
            size += len(line)
            if size > MAX_MESSAGE_BYTES:
                too_big, current = True, []
            elif not too_big:
                current.append(line[1:] if re.match(rb">+From ", line) else line)
        line = following
    if current is not None:
        yield (None if too_big else b"".join(current)), inside


def addresses(message, *fields):
    values = [str(v) for f in fields for v in (message.get_all(f) or [])]
    return [address.lower() for _, address in getaddresses(values) if address]


def labels(message):
    """Gmail's labels, or None when the header is absent (an mbox from another mail app)."""
    value = message.get("X-Gmail-Labels")
    if value is None:
        return None
    return {label.strip().strip('"') for label in str(value).split(",")}


def date(message):
    """The UTC timestamp in the corpus' shape, or None when the Date is missing or bad."""
    try:
        # Python 3.9 parses the header when it is read, and raises on a bad one.
        value = message.get("Date")
        when = None if value is None else parsedate_to_datetime(str(value))
    except (TypeError, ValueError, IndexError):
        return None
    if when is None:
        return None
    if when.tzinfo is None:
        when = when.replace(tzinfo=timezone.utc)
    return when.astimezone(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


@contextlib.contextmanager
def budget(seconds):
    """Raise Unreadable when the block runs past the given seconds: a crafted message can make the
    standard parser slow. Where the system has no interval timer, the block runs unbounded."""
    if not hasattr(signal, "setitimer"):
        yield
        return

    def expired(signum, frame):
        raise Unreadable("too slow")

    previous = signal.signal(signal.SIGALRM, expired)
    delay, interval = signal.setitimer(signal.ITIMER_REAL, seconds)
    started = time.monotonic()
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)
        if delay:
            # An earlier timer goes back on with what it had left (it fires at once if that ran out).
            signal.setitimer(signal.ITIMER_REAL, max(delay - (time.monotonic() - started), 0.001), interval)


def parse(raw):
    end = min((i for i in (raw.find(b"\n\n"), raw.find(b"\r\n\r\n")) if i >= 0), default=len(raw))
    if end > MAX_HEADER_BYTES:
        raise Unreadable("unreadable")
    try:
        return email.message_from_bytes(raw, policy=policy.default)
    except UNREADABLE:
        raise Unreadable("unreadable")


def plain_text(message):
    """The text/plain body, or None when the message has none (HTML only)."""
    try:
        body = message.get_body(preferencelist=("plain",))
        return None if body is None else body.get_content()
    except UNREADABLE:
        raise Unreadable("unreadable")


def quoted(line):
    return line.lstrip().startswith(">")


def without_attribution(lines):
    """The lines without the paragraph that introduces the first quote, when it ends with a colon:
    in any language and any client, that is the "<someone> wrote:" line."""
    first = next((i for i, line in enumerate(lines) if quoted(line)), None)
    end = first
    while end and not lines[end - 1].strip():
        end -= 1
    if not end or not lines[end - 1].rstrip().endswith(":"):
        return lines
    start = end
    while start and lines[start - 1].strip():
        start -= 1
    return lines[:start] + lines[end:]


def own_words(text):
    """The sender's own paragraphs: quoted replies and the line that introduces them, forwarded
    headers, phone sign-offs, a closing and what follows it, and the signature cut; every quoted
    (`>`) line dropped."""
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    cut = min((m.start() for m in (p.search(text) for p in CUT) if m), default=len(text))
    # The last closing ends their words, when some come before it and no more than a signature
    # after it; a PS after the signature is still theirs.
    def signature_end(m):
        after = PS.search(text, m.end(), cut)
        return after.start() if after else cut

    closings = [m for m in CLOSING.finditer(text, 0, cut) if text[:m.start()].strip()
                and len(text[m.end():signature_end(m)].strip()) <= SIGNATURE_CHARS]
    if closings:
        end = signature_end(closings[-1])
        text = text[:closings[-1].start()] + ("\n\n" + text[end:cut] if end < cut else "")
        cut = len(text)
    text = SIGN_OFF.sub("", text[:cut])
    lines = [line for line in without_attribution(text.split("\n")) if not quoted(line)]
    return [p.strip() for p in re.split(r"\n\s*\n", "\n".join(lines)) if p.strip()]
