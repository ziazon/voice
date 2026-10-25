"""Read sent mail from an mbox without filters, member config or output writes.

An mbox (Google Takeout's mail export, or any other) is split into messages here, read-only and
streamed. Each message yields its sender, recipients, Gmail labels, date and the plain text the
sender wrote, with quoted replies, forwarded headers and signatures cut away.
"""
import email
import email.errors
import re
from datetime import timezone
from email import policy
from email.utils import getaddresses, parsedate_to_datetime

MAX_MESSAGE_BYTES = 50 * 1024**2
UNREADABLE = (ValueError, LookupError, TypeError, AttributeError, IndexError, email.errors.MessageError)

# Everything from the first of these lines on is someone else's words or a signature.
CUT = [re.compile(p, re.M) for p in (
    r"^On\b[^\n]*(?:\n[^\n]*)?\bwrote:[ \t]*$",           # Gmail and Apple Mail, one or two lines
    r"^-{2,}[ \t]*Original Message[ \t]*-{2,}",           # Outlook
    r"^-{2,}[ \t]*Forwarded message[ \t]*-{2,}",          # a forward
    r"^_{10,}[ \t]*$",                                    # Outlook's rule above the quoted headers
    r"^From:[^\n]*\n(?:[^\n]*\n){0,3}?(?:Sent|Date):",    # quoted headers with no rule above them
    r"^-- ?$",                                            # the signature delimiter
)]
SIGN_OFF = re.compile(r"^Sent from my [^\n]*$", re.M)


class Unreadable(Exception):
    pass


def messages(path):
    """Each message's bytes, in file order, or None for one past the size cap. The file is opened
    read-only and read line by line; a body line escaped as `>From ` is unescaped one level."""
    current, size, too_big = None, 0, False
    with open(path, "rb") as box:
        for line in box:
            if line.startswith(b"From "):
                if current is not None:
                    yield None if too_big else b"".join(current)
                current, size, too_big = [], 0, False
                continue
            if current is None:
                continue
            size += len(line)
            if size > MAX_MESSAGE_BYTES:
                too_big = True
                current = []
                continue
            if not too_big:
                current.append(line[1:] if re.match(rb">+From ", line) else line)
    if current is not None:
        yield None if too_big else b"".join(current)


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
    value = message.get("Date")
    if value is None:
        return None
    try:
        when = parsedate_to_datetime(str(value))
    except (TypeError, ValueError, IndexError):
        return None
    if when is None:
        return None
    if when.tzinfo is None:
        when = when.replace(tzinfo=timezone.utc)
    return when.astimezone(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def parse(raw):
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


def own_words(text):
    """The sender's own paragraphs: quoted replies, forwarded headers, sign-offs and the
    signature cut, and every quoted (`>`) line dropped."""
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    cut = min((m.start() for m in (p.search(text) for p in CUT) if m), default=len(text))
    text = SIGN_OFF.sub("", text[:cut])
    lines = [line for line in text.split("\n") if not line.lstrip().startswith(">")]
    return [p.strip() for p in re.split(r"\n\s*\n", "\n".join(lines)) if p.strip()]
