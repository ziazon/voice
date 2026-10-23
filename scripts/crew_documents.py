"""Read document paragraphs without filters, member config or output writes."""
import os
import re
import shutil
import stat
import subprocess
import zipfile
from pathlib import Path
from xml.etree import ElementTree

MAX_BYTES = 50 * 1024**2
WORD_NS = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"
IMAGES = {".png", ".jpg", ".jpeg", ".gif", ".webp", ".heic", ".svg", ".bmp", ".tif", ".tiff"}
NO_PDF = "no pdftotext (brew install poppler, or export it as .docx or .txt)"


class Unreadable(Exception):
    pass


def ignored(name):
    return (name.startswith((".", "~$")) or
            name in {"__MACOSX", "Thumbs.db", "desktop.ini"})


def walk(root):
    """Relative file and symlink paths, sorted without following symlink directories."""
    root = Path(root)
    if root.is_symlink():
        raise Unreadable("symlink")
    paths = []
    for folder, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            if ignored(name):
                continue
            path = Path(folder) / name
            if path.is_symlink() or name in files:
                paths.append(path.relative_to(root))
        dirs[:] = [name for name in dirs if not ignored(name) and not (Path(folder) / name).is_symlink()]
    return sorted(paths, key=lambda p: p.as_posix())


def docx_paragraphs(path):
    with zipfile.ZipFile(path) as archive:
        info = archive.getinfo("word/document.xml")
        if info.file_size > MAX_BYTES or info.file_size > 100 * info.compress_size:
            raise Unreadable("unreadable")
        xml = archive.read(info)
    # Removing NULs also catches declarations in UTF-16/32 before XML parsing.
    declarations = xml.replace(b"\x00", b"").upper()
    if b"<!DOCTYPE" in declarations or b"<!ENTITY" in declarations:
        raise Unreadable("unreadable")
    document = ElementTree.fromstring(xml)
    return ["".join(t.text or "" for t in p.iter(WORD_NS + "t"))
            for p in document.iter(WORD_NS + "p")]


def pdf_text(path, program):
    program = program if program is not None else shutil.which("pdftotext")
    if program is None:
        raise Unreadable(NO_PDF)
    try:
        result = subprocess.run([program, "-q", "-enc", "UTF-8", str(path), "-"],
                                capture_output=True, timeout=120)
    except (FileNotFoundError, PermissionError):
        raise Unreadable(NO_PDF)
    except subprocess.TimeoutExpired:
        raise Unreadable("unreadable")
    if result.returncode:
        raise Unreadable("unreadable")
    return result.stdout.decode("utf-8", errors="ignore").replace("\f", "\n\n")


def read(path, pdftotext=None):
    """Return a kind and raw paragraphs, or an Unreadable skip reason."""
    path = Path(path)
    if path.is_symlink():
        raise Unreadable("symlink")
    try:
        info = path.lstat()
        if not stat.S_ISREG(info.st_mode):
            raise Unreadable("unreadable")
        if info.st_size > MAX_BYTES:
            raise Unreadable("too large")
        suffix = path.suffix.lower()
        if suffix in IMAGES:
            return "image", []
        if suffix == ".docx":
            return "docx", docx_paragraphs(path)
        if suffix == ".pdf":
            text = pdf_text(path, pdftotext)
            return "pdf", re.split(r"\n\s*\n", text)
        if suffix in {".md", ".markdown", ".txt"}:
            text = path.read_bytes().decode("utf-8", errors="ignore")
            return "text", re.split(r"\n\s*\n", text)
        raise Unreadable("unsupported type")
    except (OSError, ValueError, KeyError, zipfile.BadZipFile, RuntimeError, ElementTree.ParseError):
        raise Unreadable("unreadable")
