# VO3 brief: the anonymizer engine

Voice (today still named Persona Builder; the rename is VO2, so keep today's names everywhere)
will keep an anonymized copy of a person's own skills for its own use, and Flightdeck's public
skills export will later use the same engine. This slice builds the engine and the command that
runs it: find every skill folder on the computer, ask the person's yes per skill, then copy each
agreed skill with private details replaced or refused, and report what was found by **file, line
and kind only, never the text**. The plan is VO-D6 (§ *The public anonymizer*) with gate answers
Q1 to Q3: a new skill waits until the person says yes once; claude.ai's synced skills are never
offered unless the person names one; a listed term becomes a consistent placeholder, and anything
still found after that is refused.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/source-driven-development/SKILL.md`
(the `regex` crate's syntax: no look-around, no backreferences), `~/.codex/skills/plain-language/SKILL.md`
(every sentence a person reads), `~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only (an assistant "Pebble", a principal "Rowan Vale", a
company "Pebble Freight", hosts under `example.com`), no real person, no fixed paths, and **no
real secret in any fixture** (build token-shaped canaries from pieces at run time, e.g.
`format!("ghp_{}", "a".repeat(36))`, so the repo itself never holds one). Add no dependency.
Commit nothing; leave every change in the working tree.

Out of scope (later slices): file hashes and refresh (VO4), a screen in the app (VO4), the
Flightdeck export (VO5), the panel leak guard using these rules (VO7), any model call.

## 1. Where the code goes

| File | What |
| --- | --- |
| `builder/ingest/src/anonymize.rs` (new; split into `anonymize/{rules,detect,terms,sections}.rs` if it passes ~600 lines) | The rules, the transform, the re-scan, the placeholder mapping, the report text |
| `builder/ingest/src/skills.rs` (new) | Finding skill folders, reading a skill's files, the agreement records, writing the output folder |
| `builder/ingest/src/lib.rs` | `pub mod anonymize; pub mod skills;` |
| `builder/ingest/src/check.rs` | Make `read` (the `O_NOFOLLOW` reader) `pub(crate)` and reuse it; no other change (the privacy check is byte-compared with Python's) |
| `builder/panel-runner/src/guard.rs` | Move `INVISIBLE` and `scrub` into `anonymize` unchanged; here keep `pub use persona_builder_ingest::anonymize::scrub;` so every caller and test of `guard::scrub` is untouched |
| `builder/core/src/member.rs` `read_member` | Refuse an `anonymizer` key in `member.json` (§ 2) |
| `builder/cli/src/main.rs` (+ a new `builder/cli/src/skills.rs` holding the handlers) | `skills list|agree|withdraw` and `anonymize` (§ 6) |

## 2. The rules: `anonymize::Rules`

Three layers: the built-in detectors (§ 3, always on, no switch), the person's own lists from
the `anonymizer` object in their gitignored `private.json`, and the never-copy list.

`read_member` already copies `private.json`'s non-`filters` keys into `Member::extra`, so read the
object from `member.extra.get("anonymizer")`. In `core/src/member.rs` `read_member`, before the
merge, refuse an `anonymizer` key in **member.json** itself with
`Error::Invalid("member config: member.json: the anonymizer's lists go in private.json, not member.json".into())`
(the lists are private terms; member.json may be shared). Add a core unit test beside
`private_json_cannot_supply_ceilings`.

```json
"anonymizer": {
  "terms": ["Rowan Vale", {"term": "Pebble Freight", "kind": "company"}],
  "patterns": ["pebble-[0-9]+"],
  "allow": ["rowanberry"],
  "never_copy": ["family-*"],
  "drop_files": ["NOTES-*.md"],
  "projects": ["~/work/pebble"]
}
```

```rust
pub struct Rules { /* private */ }
impl Rules {
    /// The built-in detectors and nothing else.
    pub fn builtin() -> Rules;
    /// `builtin()` plus `member.extra["anonymizer"]` when present; `home` expands `~/` in projects.
    pub fn for_member(member: &Member, home: &Path) -> Result<Rules>;
    pub fn from_value(value: Option<&Value>, home: &Path) -> Result<Rules>;
    pub fn never_copy(&self, skill_name: &str) -> bool;
    pub fn projects(&self) -> &[PathBuf];
}
```

Every key is optional; an unknown key is refused. Each message starts
`member config: private.json: anonymizer` and names the key and the index, **never the value**
(values are private terms and paths), e.g. `member config: private.json: anonymizer.terms[3] must be 2 to 200 characters on one line`.

| Key | Each entry | Limits and refusals |
| --- | --- | --- |
| `terms` | a string, or `{"term": <string>, "kind": <string>}`; a string's kind is `term` | at most 2,000; the term, trimmed, is 2 to 200 characters with no control character (newline included); `kind` matches `^[a-z][a-z-]{0,31}$`; two entries with the same normalized term (lower-cased with `str::to_lowercase`, whitespace runs made one space, trimmed) are refused (`anonymizer.terms[5] repeats anonymizer.terms[2]`); an object with any other key is refused |
| `patterns` | a regex, matched without regard to case | at most 500; at most 1,000 bytes each; compiled with `RegexBuilder::new(p).case_insensitive(true).size_limit(1 << 20)`; a bad one: `anonymizer.patterns[2] is not a valid regex, or it uses look-around or a backreference, which Persona Builder can't read` |
| `allow` | a literal string that is never a finding (§ 4, masking) | at most 500; 3 to 200 characters, no control character |
| `never_copy` | a skill name; `*` matches any run of characters; no other character is special; compared without regard to case | at most 500; matches `^[A-Za-z0-9._*-]{1,100}$` |
| `drop_files` | a file name (no `/`), `*` as above, compared without regard to case; such files are left out of every skill | at most 100; 1 to 255 bytes, no `/`, no control character |
| `projects` | a folder whose `.claude/skills/` is searched (§ 5) | at most 100; starts `~/` or `/`; no `.` or `..` component; at most 1,024 bytes |

## 3. The built-in detectors

Compile each once (`LazyLock`). `kind` is what the report prints.

| Kind (printed) | Pattern (regex crate syntax), and what is not a finding |
| --- | --- |
| `email` | `[\w.+-]+@[\w-]+(?:\.[\w-]+)+`. Not a finding when the part after `@`, lower-cased, is `example.com`, `example.org` or `example.net`, or ends `.example`, `.test`, `.invalid` or `.localhost` (RFC 2606 and 6761's reserved names) |
| `phone` | `(?:\+\d{1,3}[\s.-]?)?\(?\b\d{3}\)?[\s.-]\d{3}[\s.-]\d{4}\b` and `\+\d{1,3}(?:[\s.-]?\d{2,4}){2,5}\b` |
| `long number` | `\b\d{7,}\b` and `\b\d{3}-\d{2}-\d{4}\b` |
| `secret` | `\bsk-[A-Za-z0-9_-]{20,}`, `\bsk_(?:live\|test)_[A-Za-z0-9]{16,}`, `\bgh[pousr]_[A-Za-z0-9]{20,}`, `\bgithub_pat_[A-Za-z0-9_]{20,}`, `\bxox[abprs]-[A-Za-z0-9-]{10,}`, `\bxapp-[A-Za-z0-9-]{10,}`, `\bAKIA[0-9A-Z]{16}\b`, `\beyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}`, `\bAIza[0-9A-Za-z_-]{30,}`, `\bhf_[A-Za-z0-9]{30,}`, `\bglpat-[A-Za-z0-9_-]{20,}`, `\bnpm_[A-Za-z0-9]{30,}`, `-----BEGIN [A-Z ]*PRIVATE KEY-----`, `(?i)\b(?:passw(?:or)?d\|secret\|token\|api[_-]?key\|access[_-]?key)\b["']?\s*[:=]\s*["']?[A-Za-z0-9/+_.-]{8,}`, and a password in a link, `(?i)\b[a-z][a-z0-9+.-]*://[^/\s:@]+:[^/\s@]+@` |
| `home folder` | `~[A-Za-z][\w.-]*/` (another user's home, `~rowan/`): what is left after § 4 step 4 |
| `private host` | `(?i)\b[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*\.(?:local\|lan\|internal\|intranet\|corp\|home\.arpa\|ts\.net)\b`. Not a finding when the character after the match is `-` or `_`, or is `.` followed by a letter or digit (so `settings.local.json` and `app.internal.ts` are not hosts) |
| `private address` | `\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}\|192\.168\.\d{1,3}\.\d{1,3}\|172\.(?:1[6-9]\|2\d\|3[01])\.\d{1,3}\.\d{1,3}\|100\.(?:6[4-9]\|[7-9]\d\|1[01]\d\|12[0-7])\.\d{1,3}\.\d{1,3}\|169\.254\.\d{1,3}\.\d{1,3})\b` |
| `ticket id` | `\b([A-Z][A-Z0-9]{1,9})-\d{1,6}\b`, case-sensitive. Not a finding when group 1 is one of `AES ARM CSS CVE CWE ECMA ES FIPS GPT HTML HTTP IEC IEEE IPV ISO MD NIST OWASP PEP PKCS RFC RSA SHA SP SSL TLS UCS UTF WCAG X86` (standards, not tickets: `UTF-8`, `SHA-256`, `ISO-8601`) |
| `your term` | § 4 step 7 |
| `your pattern` | each of the person's `patterns` |
| `private marker` | § 4 step 3 |

(The `\|` above is a table escape: the regex has a plain `|`.)

Built-in masks (always masked in the re-scan copy, as the person's `allow` is): `git@github.com`,
`git@gitlab.com`, `git@bitbucket.org` (SSH clone addresses, not mail).

## 4. One file: `anonymize::file`

```rust
pub struct Mapping { /* private */ }          // § 4a
pub enum LeftOut { PrivateFile, Hidden, Denied, Link, NotText, TooLarge }
pub struct Finding { pub file: PathBuf, pub line: usize, pub kind: &'static str }
pub enum FileResult { Kept(String), Refused(Vec<Finding>), LeftOut(LeftOut) }
pub fn file(relative: &Path, bytes: &[u8], rules: &Rules, mapping: &mut Mapping) -> FileResult;
```

In this order:

1. **Left out, unread** (`LeftOut`; no finding, the skill is still copied):
   - `PrivateFile`: the file name is `LOCAL.md` or `LEARNINGS.md` (any letter case: private
     overlays and owner journals), or matches a `drop_files` entry;
   - `Hidden`: any component of `relative` starts with `.`;
   - `Denied`: `persona_builder::deny::DenyList::builtin().denied_relative(relative)`;
   - `TooLarge`: more than 1 MiB;
   - `NotText`: not UTF-8.
   (`Link` is set by the folder reader, § 5.)
2. **Remove invisible characters** with `scrub` (moved from `guard.rs`).
3. **Drop private sections.** A line whose content, trimmed, equals `<!-- private -->` (ASCII case
   ignored) opens a section; `<!-- /private -->` closes it. The two marker lines and everything
   between them are dropped. An opener inside an open section, a closer with none open, or a
   section still open at the end is a `private marker` finding on that line (for the last, the
   opener's line). Keep a map from each kept line to its line in the source file: **every
   finding's line is the source file's line**, so the person can fix it.
4. **Home folders become `~`**: `/Users/<name>` and `/home/<name>` (`<name>` is a run of
   characters other than `/`, whitespace, quotes, backtick, `<>()[]{}`), except
   `/Users/Shared`, and `<letter>:\Users\<name>` (letter case ignored), each become `~`.
5. **Terms become placeholders.** For every term, find its matches: case-insensitive, its words
   (split on whitespace) joined by `[^\p{L}\p{N}\n]{1,8}`, so `Pebble Freight`, `pebble-freight`
   and `Pebble_Freight` all match, but never across a line break; a match counts only when the
   character before it and the one after it (if any) are neither a letter nor a digit (on a
   rejected match, search again from the next character after its start). Gather every term's
   matches, keep a non-overlapping set (earliest start first; at the same start, the longest),
   and replace each with `mapping.placeholder(kind, term)`. Line breaks never change, so the line
   map from step 3 still holds.
6. **The re-scan copy.** From the result: replace every placeholder
   (`<[a-z][a-z-]{0,31}-[0-9]+>`) and every case-insensitive occurrence of an `allow` entry or a
   built-in mask with spaces, **byte for byte** (same length, so offsets and lines hold).
7. **Re-scan** the copy with every detector in § 3, every `patterns` entry, and the terms again,
   two ways: as in step 5 but with `\n` allowed in the separator (a term wrapped over two lines),
   and, for a term of 4 or more letters and digits with no whitespace, as a plain case-insensitive
   substring (`PebbleFreightApi` for the term `Pebble`). The finding's line is its start's line.
8. **Refuse or keep.** One finding per (line, kind), sorted by line then kind. Any finding:
   `Refused(findings)`; none: `Kept(text)` (the step-5 text, never the masked copy).

### 4a. Placeholders: `Mapping`

`<kind>-<n>`, e.g. `<company-1>`, numbered per kind from 1 in the order first used, and **kept
for good**: a term once given a number keeps it, even after it leaves the person's list, so
copies stay the same between runs. The key is the normalized term (§ 2).

```rust
impl Mapping {
    pub fn from_json(bytes: &[u8]) -> Result<Mapping>;   // {"version":1,"kinds":{"company":["pebble freight"]}}
    pub fn to_json(&self) -> Vec<u8>;
    pub fn placeholder(&mut self, kind: &str, term: &str) -> String;
}
```

A file that is not that shape, or another `version`, is refused:
`"the anonymizer's placeholder file can't be read. Move it away and run anonymize again."` It
holds the person's terms, so it lives only in their private state (§ 6).

## 5. Skills: `skills.rs`

### Finding them: `skills::discover(home, rules) -> Result<Found>`

`Found { skills: Vec<Skill>, notes: Vec<String> }`;
`Skill { id: String, name: String, root: Root, folder: PathBuf, shown: String }`, `folder`
canonical, `shown` with the home folder as `~`, sorted by `id`. A skill is a folder holding a
`SKILL.md` file. A skill **folder** may be a link (people link skills in), so canonicalize it; a
**file or folder inside** a skill that is a link is never followed (§ 5, reading).

| Root | Where | Id |
| --- | --- | --- |
| `Claude` | `~/.claude/skills/<s>/SKILL.md`, every `<s>` but `synced` | `claude:<s>` |
| `Synced` | `~/.claude/skills/synced/<a>/<s>/SKILL.md` (claude.ai's synced skills) | `synced:<s>`; when two `<a>` hold the same `<s>`, keep the first `<a>` in name order and add this note once: `Some claude.ai skills are in more than one synced folder; each is listed once, from the first folder by name.` |
| `Plugin` | `~/.claude/plugins/installed_plugins.json` (at most 4 MiB): for each `plugins.<plugin>@<market>[].installPath`, `<installPath>/skills/<s>/SKILL.md` | `plugin:<plugin>:<s>` |
| `Project` | each `projects` folder `<p>`: `<p>/.claude/skills/<s>/SKILL.md` | `project:<last component of p>:<s>` |
| `Codex` | `~/.codex/skills/<s>/SKILL.md` | `codex:<s>` |

Everywhere: skip a `<s>` or `<a>` that starts with `.`; skip a folder whose canonical path
`DenyList::builtin().denied(...)`; a missing root is not an error. A repeated id keeps the first.
For plugins, an `installPath` that is not a string or whose canonical path is not inside
canonical `~/.claude/plugins/` is skipped (the file names where to read); a plugin list that
can't be read or isn't that JSON shape gives the note
`Couldn't read Claude Code's list of plugins, so their skills aren't listed.`

### Reading one: `skills::read(skill) -> Result<Vec<(PathBuf, Source)>>`

`Source::Bytes(Vec<u8>)` or `Source::Link`. Walk the canonical folder: never follow a link
(`symlink_metadata`; a link is `Source::Link`, reported `LeftOut::Link`), skip `.git` folders
whole, read files with `check::read` (`O_NOFOLLOW`). Refuse the skill (`Error::Invalid`, the
report line in § 6) past 500 files, 20 MiB in all, or 8 folders deep. Paths are relative to the
skill folder, sorted.

### One skill: `anonymize::skill`

`skill(files, rules, mapping) -> SkillResult { kept: Vec<(PathBuf, String)>, refused: Vec<Finding>, left_out: Vec<(PathBuf, LeftOut)> }`.
The skill is copied whole or not at all: any finding in any file refuses it; so does a
`SKILL.md` that is not kept (finding kind `skill file`, line 1).

### Agreement: private state `anonymizer/skills.json`

`{"version":1,"skills":{"claude:pebble-notes":{"agreed_on":"2026-10-31","folder":"~/.claude/skills/pebble-notes"}}}`,
read and written with `private::read_private_optional_at_max(home, slug, Some("anonymizer"), "skills.json", 4 << 20)`
and `write_private_at(..., Some("anonymizer"), "skills.json", ...)` (0600 in 0700, links
refused). The placeholders go beside it as `anonymizer/placeholders.json`. A record whose
`folder` differs from the skill's `shown` today no longer counts (the skill moved). A file that
is not this shape is refused with a sentence naming the file, not its contents. Each skill's
state:

| State | When | `list` prints |
| --- | --- | --- |
| never copied | `rules.never_copy(name)` (wins over a yes) | `never copied: on your never_copy list` |
| agreed | a record whose folder matches | `agreed <date>` |
| moved | a record whose folder differs | `moved since you agreed: agree again to copy it` |
| not offered | a `Synced` skill with no record | `a claude.ai skill: not offered, agree by name to copy it` |
| waiting | otherwise | `waiting for your yes` |

The date is `persona_builder_agent::ledger::local_day(SystemTime::now())`, computed in the CLI
and passed in (ingest does not depend on agent).

### Writing the copy

`--out <dir>` must not exist, or be an empty folder, or be a folder made by an earlier run
(it holds the file `.anonymized-copy`). Refuse a link, a file, or a folder of anything else; and
refuse an `--out` that is inside any discovered skill folder or inside `~/.claude/skills` (the
next run would read it). An earlier run's folder is removed whole (`fs::remove_dir_all` after
`symlink_metadata` shows a real folder) and made again. Create folders 0700 and files 0600 with
`O_CREAT | O_EXCL | O_NOFOLLOW`; write `.anonymized-copy` (one line,
`Made by persona-builder anonymize. Running it again replaces this folder.`) first. Each copied
skill goes to `<out>/<name>/<relative path>`. Two agreed skills with the same name: the first by
id is copied, the other is not (§ 6's line).

## 6. The commands

All take a member folder, as `consent agree` does (`checked_member`, `expand_home`), and hold
`private::lock_state(home, slug)` for the whole command.

```
persona-builder skills list <member_dir>
persona-builder skills agree <member_dir> <id>...
persona-builder skills withdraw <member_dir> <id>...
persona-builder anonymize <member_dir> --out <dir>
```

Help lines: `Skills on this computer, and which ones your assistant may copy`;
`Let your assistant copy these skills, with private details taken out`;
`Stop copying these skills`; `Copy the skills you agreed to, with private details taken out`.

- `skills list`: one line per skill, `<id>\t<state words>\t<shown>`, then each note on its own
  line. With none: `No skills found on this computer.`
- `skills agree`: an id that isn't listed today is refused before anything is written:
  `<id> isn't one of the skills on this computer. See them with: persona-builder skills list <member_dir>`.
  On success, one line per id: `<id>: agreed <date>.` A never-copied skill's line adds
  ` It is on your never_copy list, so it still won't be copied.`
- `skills withdraw`: `<id>: no longer copied.` (also for an id with no record).
- `anonymize`: for each agreed skill, in id order:
  - copied: `<name>: copied, <n> files` plus `, <m> left out` when `m > 0`, then one indented line
    per left-out file, `  <name>/<path> (left out: <reason>)`, the reason one of `private file`,
    `hidden`, `password or key file`, `a link`, `not text`, `too large`;
  - refused: `<name>: not copied, <k> things to fix`, then `  <name>/<path>:<line> (<kind>)` per
    finding, then the left-out lines;
  - too big to read: `<name>: not copied, it has more than 500 files, more than 20 MB or more than 8 folders inside each other`;
  - same name: `<id>: not copied, another skill you agreed to has the same name (<other id>)`.
  Then `Copied <a> of <b> skills into <out, home as ~>.` and, when any wait (waiting, not offered
  or moved), `<w> skills wait for your yes: persona-builder skills list <member_dir>`. Save the
  mapping after the run. Exit 0 when every agreed skill was copied, 1 otherwise (the copies made
  stay).

**No line ever prints a found text, a term, a pattern, an allow entry or a file's contents.**

## 7. Tests first

Write them first, run them red (failing on the assertion, or not compiling), then implement.
Temp folders only (`builder/core/tests/common` `Scratch`), never the real home; CLI tests run the
binary with `env_clear()` and `HOME` set, as `builder/cli/tests/deny_rules.rs` does.

`builder/ingest/tests/anonymize_rules.rs`: each refusal in § 2's table with its exact message,
and that no message contains the refused value; unknown key; duplicate term (case and spacing
differ); `for_member` reads private.json's object; core's member.json refusal.

`builder/ingest/tests/anonymize_file.rs`:
- each detector: one hit and the controls `rowan@example.com`, `git@github.com:org/repo.git`,
  `UTF-8`, `SHA-256`, `settings.local.json`, `app.internal.ts`, `2026-10-31`, `v1.13.1`,
  `localhost:8080`, `127.0.0.1`, a 6-digit number, `password store`, `token: ${{ secrets.X }}`;
- terms: `Pebble Freight`, `PEBBLE-freight` and `Pebble_Freight` become the same `<company-1>`;
  a second company term gets `<company-2>`; a string term gets `<term-1>`; longest wins (`Pebble`
  and `Pebble Freight` both listed); `Pebbles` is not `Pebble` but is refused by the substring
  re-scan; `Peb<U+200B>ble Freight` is replaced (step 2); `Pebble\nFreight` is refused on line 1;
  the mapping survives `to_json`/`from_json` and keeps numbers after a term leaves the list;
- a term that is a placeholder's kind word (`company`) never rewrites `<company-1>`;
- allow masking: `rowanberry` allowed while `Rowan` is a term;
- sections: dropped with their markers; each marker error; a finding after a dropped section
  reports the **source** line;
- home folders: `/Users/rowan/x` → `~/x`, `/home/rowan` → `~`, `C:\Users\rowan\x` → `~\x`,
  `/Users/Shared/x` unchanged;
- left out: `LOCAL.md`, `references/LEARNINGS.md`, `.env`, `certs/a.pem`, a `drop_files` match,
  2 MiB, invalid UTF-8;
- **the leak test**: a skill holding a canary email, phone, token, term and pattern hit; render
  the report through the CLI's text functions and assert none of the canaries (nor any term,
  pattern or allow entry) appears in it.

`builder/ingest/tests/skills_discover.rs`: each root; a linked skill folder is found; a link
inside a skill is `LeftOut::Link` and its target is never read (point it at a canary outside);
`.git` and dot folders skipped; a plugin `installPath` outside `~/.claude/plugins` skipped; a
bad plugin list gives the note; duplicate synced names; a skill under `.ssh` skipped; the size
and depth limits.

`builder/cli/tests/anonymize.rs` (Pebble, Rowan Vale): `skills list` shows each state; `agree`
an unknown id writes nothing; `agree`, then `anonymize --out`: the output is 0700/0600, holds the
stamp, the placeholders, no term; a refused skill prints its lines and exits 1 while the clean
one is still copied; a never-copied skill isn't copied; a moved skill waits; a second run
replaces the folder and keeps the same placeholders; `--out` refused when it is a file, a link,
a non-empty unstamped folder, inside a skill folder, or inside `~/.claude/skills`;
`anonymizer/skills.json` and `placeholders.json` are 0600 under `~/.claude/state/<slug>/`.

Then run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test -p persona-builder-ingest -p persona-builder-cli -p voice-panel-runner`, `cargo test --manifest-path builder/core/Cargo.toml`,
and `bash scripts/ci-local.sh`, and report each result.
