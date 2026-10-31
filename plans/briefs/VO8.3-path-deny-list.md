# VO8.3 brief: the secrets deny list for paths

Persona Builder (soon Voice) is the only part of the app that reads a person's own files: their
Claude Code history, a claude.ai export they choose, a document they hand the CLI, and an
assistant's folder when it is saved. Today it filters what it reads by **text** only
(`builder/ingest/src/filters.rs`, `Kind::Secret`). This slice adds a **path** filter: a list of
files it never opens, whatever they contain, applied before any read. A model turn has no tools
(`builder/agent/src/claude.rs`, `--tools ""`), so these reads are the only way anything on disk
reaches a model; the list closes them to password and key files.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/source-driven-development/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`
(every sentence a person reads), `~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only (an assistant "Pebble", a principal "Rowan Vale"), no
real person, no fixed paths, and **no real secret in any fixture**: canary files hold the text
`canary` and nothing else. Add no dependency. Commit nothing; leave every change in the working
tree.

## 1. The list: `builder/core/src/deny.rs`, `persona_builder::deny`

It lives in the core crate because every reader depends on core (`ingest`, `save`, `cli`, `api`)
and the person's additions are parsed in `core/src/member.rs`. Add `pub mod deny;` to
`core/src/lib.rs`.

```rust
pub struct DenyList { /* private */ }
impl DenyList {
    /// The built-in list alone (it needs no home folder: every built-in entry is a name or a run
    /// of names).
    pub fn builtin() -> DenyList;
    /// The built-in list plus `extra`, the person's own entries. `home` expands a leading `~/`.
    pub fn new(home: &Path, extra: &[String]) -> Result<DenyList>;
    /// `new(home, &member.filters.denied_paths)`.
    pub fn for_member(member: &Member, home: &Path) -> Result<DenyList>;
    /// True when the path, as given or with every link resolved, is on the list.
    pub fn denied(&self, path: &Path) -> bool;
    /// True when a path relative to a folder is on the list, matching only its own components
    /// (no link resolution, no person's path entries): for checking a folder's contents without
    /// the folder's own location counting.
    pub fn denied_relative(&self, relative: &Path) -> bool;
    /// `Ok(())`, or `Err(Error::Invalid(text))` with the sentence in § 4, naming the path with
    /// the home folder shown as `~`.
    pub fn check(&self, path: &Path) -> Result<()>;
}
```

**The built-in list** (VO plan § *Read wide, exit narrow*), as two kinds of entry:

- **Name entries**, matched against **every component** of the path, so a folder of that name
  denies everything inside it: `.ssh`, `.aws`, `.gnupg`, `.env*`, `.envrc`, `.npmrc`, `.netrc`,
  `.pgpass`, `*.pem`, `*.key`, `*.p12`, `*.keychain`, `*.keychain-db`, `.credentials.json`
  (Claude Code's own credential file, in `~/.claude` or any `CLAUDE_CONFIG_DIR` profile).
- **Sequence entries**, matched against any run of consecutive components: `.config/gh`,
  `.docker/config.json`, `Library/Keychains`.

`*` matches any run of characters (including none) inside one component; no other character is
special. `.env*` therefore denies `.env`, `.env.local` and `.environment`; over-denying is the
safe side. `*.key` must not deny `monkey.txt` or `keys.md`; `docker/config.json` (no dot) and
`.config/other` are allowed.

**Matching.** A path is denied when either form below has a component (or a run of components)
matching an entry:
1. the path as given, made absolute against the current folder if relative, with no `.` or `..`
   resolution (so `/x/.ssh/../notes` is denied as given);
2. `std::fs::canonicalize(path)`, when it succeeds (every link resolved). When it fails (a missing
   file, a dangling link), form 1 alone decides; the read that follows fails on its own.

Compare case-insensitively: lower-case both sides with `str::to_lowercase`; a component that is
not UTF-8 is compared through `to_string_lossy`. (macOS folders are case-insensitive by default,
so `~/.SSH/ID_RSA` is the same file.)

**The person's entries** (`extra`). Each is one of:
- a **name** (no `/`): a name entry, as above;
- a **path** starting `~/` or `/`: the path, `~/` expanded to `home`, and everything under it.
  Its components may use `*`. Match it as a prefix of form 1 and form 2 (component-wise,
  case-insensitive); also canonicalize the entry itself when it exists, and match that too, so an
  entry naming a link's location also catches the link's target.

Refuse an entry (`Error::Invalid`, the sentence in § 4) that is empty or only spaces, longer than
255 bytes, holds a control character, has a `.` or `..` component, or contains `/` without
starting `~/` or `/`. Refuse more than 200 entries. A trailing `/` is allowed and means nothing
extra.

## 2. The person's entries in `member.json` and `private.json`

Add to `core/src/member.rs` `Filters`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub denied_paths: Vec<String>,
```

before `extra`. It is optional: `Member::from_value`'s required-key list does not change, and a
config without it reads and writes exactly as today (the Python parity tests must stay green).
`validate_config` refuses a `filters.denied_paths` that is not a list of strings, with
`": filters.denied_paths must be a list of strings"`, as it does for `pasted_prefixes`.
`read_member` merges `private.json`'s `filters.denied_paths` by appending to member.json's list,
exactly as it does `pasted_prefixes` (today any other `filters` key must be a regex string; make
`denied_paths` take the list branch). Do not add the field to `crew/_template/member.json`.

## 3. Where the list is applied

Every site below runs the check **before** opening the file or listing the folder.

| Site | Today | Change |
| --- | --- | --- |
| `ingest/src/mine.rs` `mine_with_progress` | lists `projects`, each project folder, opens each `*.jsonl` | Build `DenyList::for_member(member, home)` at the start. `deny.check(projects)?` (a denied history folder stops the run). For each project folder that is denied, skip it and add its `*.jsonl` count to a new `Mined::denied`. For each session file that is denied, skip it and add 1 to `Mined::denied`. |
| `ingest/src/mine.rs` `Mined`, `summary` | | New field `pub denied: usize` (doc: files on the never-read list, left out unopened). `summary` adds `; left out {n} files on the never-read list` after the `skipped` text, **only when `n > 0`**, so every existing output is byte-identical. |
| `ingest/src/mine.rs` `folders` | lists every folder for the folder table | Take `deny: &DenyList`; leave a denied folder out of the rows entirely (do not list it). Update its callers (`cli/src/main.rs`, `api/src/history.rs`) to build the list with `for_member`. |
| `ingest/src/claudeai.rs` `read_export(path)` | opens the export (zip, folder or file) | Signature `read_export(path: &Path, deny: &DenyList)`. `deny.check(path)?` first; in the folder branch, also check `path.join("conversations.json")` before opening it. Every caller passes a list: `cli/src/main.rs` `claudeai inspect` has no member, so it uses `DenyList::builtin()`; the commands that load a member use `for_member`; `api/src/history.rs` uses `for_member`. |
| `api/src/history.rs` export reads | maps every `read_export` error to `EXPORT_BAD` | Before calling `read_export`, call `deny.check(&path)` itself and, on `Err`, return `failure(<that error's text>)` so the screen shows § 4's sentence instead of `EXPORT_BAD`. |
| `api/src/history.rs` mining | `ReadCounts.private_left_out = personal + secret` | Add `mined.denied` into `private_left_out` (saturating). No DTO change. |
| `cli/src/main.rs` `shingle` | `std::fs::read(&doc)` | `DenyList::for_member(&member, &home)?.check(&doc)?` before the read. |
| `cli/src/panel.rs` `read` (a question file) | `draft::read_text(path, …)` | `DenyList::builtin().check(path)` first (the panel reads no member here); on `Err` return the denial's own text, not `READ_INPUT`. |
| `save/src/git.rs` `list` (the folder to save) | refuses links, skips `private.json` and `.DS_Store` | Refuse a file whose path relative to the saved folder is denied by the **built-in** list: `Error::Invalid(text::denied_in_folder(rel))` (§ 4). Build `DenyList::builtin()` once in `folder_files` and test each file with `denied_relative(rel)`, so the folder's own location never denies it. |

Do **not** change `ingest/src/check.rs` (the privacy check is byte-compared with the Python kit's,
`tests/check_parity.rs`), the Python scripts, or anything under `builder/ui`.

## 4. The sentences (plain words, no tool words)

Put them in each crate's `text.rs` where one exists (core has none: put core's in `deny.rs` as
`pub fn` or `pub const`).

- `check`'s refusal: `"{shown} is a kind of file that holds passwords or keys, so Persona Builder
  never opens it. Choose another file."` (`shown`: the path with the home folder as `~`).
- A bad entry: `"member config: filters.denied_paths: {entry:?} isn't a name or a path Persona
  Builder can use. Write a name like diary*, or a path starting with ~/ or /, with no . or ..
  parts, at most 255 characters."` Too many: `"member config: filters.denied_paths holds more
  than 200 entries."`
- `save`'s `denied_in_folder(rel)`: `"{rel} is a kind of file that holds passwords or keys, so
  Persona Builder won't save this folder. Move it out of the assistant's folder, then save
  again."`

## 5. Tests first

Write these first and run them red (they fail to compile or fail their assertion), then
implement. Use temporary folders the tests make and remove; never the real home.

`builder/core/tests/deny.rs`:
- Each built-in entry denies a canary under a temp home: `.env`, `.env.local`, `.envrc`, `.npmrc`,
  `.netrc`, `.pgpass`, `.ssh/id_ed25519`, `.aws/credentials`, `.gnupg/pubring.kbx`,
  `.config/gh/hosts.yml`, `.docker/config.json`, `certs/site.pem`, `certs/site.KEY`,
  `certs/id.p12`, `login.keychain-db`, `Library/Keychains/x.db`, `.claude/.credentials.json`.
- Allowed controls: `notes.md`, `monkey.txt`, `keys.md`, `docker/config.json`,
  `.config/other/x`, `.sshrc`.
- Letter case: `.SSH/ID_RSA`, `Certs/Site.PEM` denied.
- Links: `notes.md` linking to `.ssh/id_rsa` denied; a folder link `docs -> .aws`, then
  `docs/credentials`, denied; a dangling link named `notes.md` allowed (form 1 decides).
- As given: `<home>/.ssh/../notes.md` denied.
- The person's entries: `diary*` denies `journal/Diary-2026.md`; `~/Private/` denies
  `Private/a/b.txt` and not `Privateer.txt`; `/` + temp path entries work; an entry naming a link
  denies the link's target too. Each refused entry shape (empty, `../x`, `a/b`, `~/./x`, 256
  bytes, a control character) and 201 entries are refused with the sentence.
- `denied_relative`: `.env` denied, `sub/.ssh/x` denied, `notes.md` allowed.

`builder/core` member tests (extend the existing ones in `member.rs` or `core/tests/`):
`denied_paths` absent reads and writes as today; present in member.json and private.json, the
lists append in that order; a non-list is refused with the sentence above.

`builder/ingest/tests/` (a new `deny_rules.rs`): mining with `projects` itself under `.ssh` is
refused; a project folder holding a session file named `.env.jsonl` and a good one mines the good
one, `denied == 1`, and `summary` carries the new clause; with nothing denied, `summary` is
byte-identical to today's (assert the exact string for one fixture). `read_export` of an export
under `<home>/.aws/` is refused before it is opened, and of a folder export whose
`conversations.json` is a link into `.ssh` is refused.

`builder/save/tests/`: saving a folder holding `.env` or `keys/site.pem` is refused with
`denied_in_folder`; a folder holding `notes.md` saves as today.

`builder/cli/tests/`: `persona-builder claudeai inspect <home>/.aws/export.zip` fails with the
sentence (exit non-zero, nothing else printed on stdout); `shingle` with a doc under `.ssh` fails
the same way.

`builder/api/tests/`: a chosen export under `.ssh` gives the sentence, not `EXPORT_BAD`.

## 6. Done means

- `bash scripts/ci-local.sh` all passed (it runs fmt, clippy with `-D warnings`, every test and
  the Python parity tests).
- Every existing test is unchanged except for the call sites whose signatures § 3 changes.
- Report: the files changed, each test added, and anything this brief left unclear.
