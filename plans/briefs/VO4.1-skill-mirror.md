# VO4.1 brief: the mirror and its hashes

VO3 built `voice anonymize --out <dir>`: it copies the skills a person agreed to, with private
details replaced or refused, into a folder they choose. This slice gives each assistant **its own
copy that stays up to date**, the skill mirror (VO-D7): one fixed private folder, rebuilt by
`voice skills refresh <member_dir>`, with a list of file hashes so anyone reading it can tell what
changed. Gate answers Q1 (a new skill waits for a yes) and Q2 (claude.ai's synced skills are never
copied unless named) are already VO3's skill states; they are unchanged here.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `.ai/SECURITY.md`, `CONTRIBUTING.md`, and
`plans/briefs/VO3-anonymizer.md` (the engine this builds on). Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/source-driven-development/SKILL.md`
(`sha2` 0.10's `Digest` API), `~/.codex/skills/plain-language/SKILL.md` (every sentence a person
reads), `~/.codex/skills/project-config/SKILL.md`.

This is a public repo: invented names only (an assistant "Pebble", a principal "Rowan Vale", a
company "Pebble Freight", hosts under `example.com`), no real person, no fixed paths, no real
secret in a fixture. Commit nothing; leave every change in the working tree.

Out of scope (later slices): refreshing when a panel is convened (VO4.2), a screen in the app
(VO4.3), anything that reads the mirror.

## 1. Decisions already made (do not change them)

- **Where:** the mirror is `~/.claude/state/<slug>/mirror/`, inside the assistant's private state
  folder. Forgetting the assistant (`voice forget`) already deletes the whole state folder, so it
  deletes the mirror too; nothing in `forget` changes.
- **Every refresh starts from nothing.** A refresh empties the mirror and copies every agreed skill
  afresh with today's rules, so no copy made under older rules survives a refresh. There is no
  "skip unchanged skills" shortcut.
- **Hashes of the copies only.** The manifest lists the SHA-256 of each file **as written in the
  copy**, never of the source file. (A hash of a private source file would let anyone who can read
  the mirror test a guess about what the private file says.)
- **A failed refresh removes the mirror.** Once the member is read, any error removes the mirror
  folder (only when it is Voice's own: see § 4) before the error is returned. A mirror is either
  fresh or absent, never stale.
- **One copying loop.** `voice anonymize --out` and `voice skills refresh` share one function, and
  both write the manifest.
- **New dependency:** `sha2 = "=0.10.9"` in `builder/ingest/Cargo.toml` `[dependencies]` (and in
  `[dev-dependencies]` only if the tests can't use the normal dependency; they can). It is already
  in `Cargo.lock` through Tauri, so `Cargo.lock` gains only the new edge; check its diff is that
  small.

## 2. Where the code goes

| File | What |
| --- | --- |
| `builder/ingest/src/skills/copy.rs` (new) | `copy_agreed` (§ 3): the loop now in `builder/cli/src/skills.rs` `anonymize`, moved here unchanged in behaviour, plus the manifest |
| `builder/ingest/src/skills/mirror.rs` (new) | `refresh` (§ 4), `Manifest` (§ 5) |
| `builder/ingest/src/skills/output.rs` | `pub fn write_manifest(out: &Path, bytes: &[u8]) -> Result<()>` (writes `.manifest.json` 0600 with the existing no-follow, `O_EXCL` `write_file`); `pub fn remove_copy(path: &Path) -> Result<bool>` (§ 4) |
| `builder/ingest/src/skills.rs` | `mod copy; mod mirror;` and their `pub use` |
| `builder/cli/src/skills.rs` | `anonymize` becomes a thin caller of `copy_agreed`; new `SkillsCommand::Refresh` |
| `builder/ingest/Cargo.toml`, `Cargo.lock` | `sha2` |

## 3. One copying loop: `skills::copy_agreed`

```rust
pub struct Copied {
    /// The report lines, exactly what `voice anonymize` prints today for each skill, then each
    /// discovery note on its own line. Every line ends with '\n'.
    pub lines: String,
    pub agreed: usize,
    pub copied: usize,
    /// Skills waiting for a yes (waiting, not offered, or moved).
    pub waiting: usize,
    pub manifest: Manifest,
}
pub fn copy_agreed(
    home: &Path,
    slug: &str,
    rules: &Rules,
    found: &Found,
    agreements: &Agreements,
    mapping: &mut Mapping,
    out: &Path,
) -> Result<Copied>;
```

Move the body of the CLI's loop here **unchanged in behaviour**: every check, every line of text,
the order, saving `anonymizer/placeholders.json` after each skill and once at the end, the
`SKILL_TOO_LARGE` and `SKILL_UNREADABLE` handling. Instead of `print!`/`println!`, append to
`lines`. For each skill it writes (`skills::write_skill`), add an entry to the manifest:
skill name → (relative path with `/` separators → lowercase hex SHA-256 of the exact bytes
written, `text.as_bytes()`). The relative paths are UTF-8 already (VO3 leaves odd names out).

`voice anonymize` keeps its output byte for byte: it prints `copied.lines`, then the two lines it
prints today (`Copied <a> of <b> skills into <out>.` and the waiting line), and exits as today. New:
after the loop it writes the manifest to `<out>/.manifest.json` (§ 5). Its date is
`voice_agent::ledger::local_day(SystemTime::now())`, computed in the CLI as `skills agree` does.

## 4. The refresh: `skills::refresh`

```rust
pub struct Refreshed {
    pub copied: Copied,
    pub mirror: PathBuf,
    pub new: usize,
    pub updated: usize,
    pub unchanged: usize,
    pub removed: usize,
}
/// The caller holds `private::lock_state(home, &member.slug)`.
pub fn refresh(home: &Path, member: &Member, date: &str) -> Result<Refreshed>;
pub const MIRROR_REMOVED: &str =
    "Voice removed your assistant's copy of your skills, so it can't be out of date. Fix this, then run voice skills refresh again.";
```

In this order:

1. `let mirror = private::private_folder(home, &member.slug, "mirror")?;` (creates it 0700 if it is
   missing; refuses a link and a folder others can open).
2. Everything from here to the end runs in a closure; if it returns `Err(e)`, call
   `remove_copy(&mirror)` (ignore its result) and return `e` with `MIRROR_REMOVED` appended:
   for `Error::Invalid(m)`, `Error::Invalid(format!("{m}. {MIRROR_REMOVED}"))` (no doubled full
   stop: trim one trailing `.` from `m` first); any other error: `Error::Invalid(format!("{e}. {MIRROR_REMOVED}"))`
   using the error's own `Display`.
3. Rules, discovery, agreements and the placeholder mapping, exactly as `voice anonymize` loads them
   today (move that loading into a small shared helper in `copy.rs` if it keeps both callers short).
4. Read the old manifest: `Manifest::read(&mirror.join(".manifest.json"))` (§ 5). Anything that
   can't be read or isn't the shape counts as an empty manifest; never an error.
5. `let out = skills::prepare_output_with_projects(&mirror, home, &found.skills, rules.projects())`.
   It empties a folder made by an earlier run (it holds `.anonymized-copy`) and refuses one that
   holds anything else. For the mirror, replace its refusal message with
   `format!("{} holds files Voice didn't put there. Move them away", display_path(&mirror, home))`
   (step 2 then appends `MIRROR_REMOVED`; `remove_copy` will not delete such a folder, § 4a).
6. `copy_agreed(home, &member.slug, &rules, &found, &agreements, &mut mapping, &out)`.
7. Write the new manifest (`write_manifest(&out, &manifest.to_json(date))`).
8. Compare with the old manifest by skill name: in the new one but not the old, **new**; in both
   with identical file maps, **unchanged**; in both otherwise, **updated**; in the old one only,
   **removed** (refused, withdrawn, never-copied, moved, or gone). Counts only: never return or
   print a removed skill's name, since a name that was fine last time may hold a term listed since.

### 4a. `remove_copy(path) -> Result<bool>`

Removes `path` only when `symlink_metadata` shows a real folder (not a link) that is empty or holds
the stamp file `.anonymized-copy` with exactly the stamp's bytes (the same test
`prepare_output_with_projects` uses; share it). Returns `Ok(true)` when it removed it, `Ok(false)`
when the folder is missing or not Voice's own, `Err` on an I/O failure.

## 5. The manifest: `.manifest.json`

```json
{"version":1,"made_on":"2026-11-01","skills":{"pebble-notes":{"SKILL.md":"<64 hex>","references/a.md":"<64 hex>"}}}
```

`Manifest { skills: BTreeMap<String, BTreeMap<String, String>> }` with `Manifest::read(path)` and
`to_json(&self, date) -> Vec<u8>` (compact JSON, keys sorted, one trailing `\n`).
`read` opens with `O_NOFOLLOW | O_NONBLOCK`, requires a regular file of at most 4 MiB (check with
`metadata()` on the open file, then read at most 4 MiB + 1 bytes), and returns an empty manifest on
any failure or on any other shape: `version` not 1, a missing or non-object `skills`, a value that
isn't an object of 64-lowercase-hex strings. The name starts with `.`, so it can never clash with a
skill (discovery skips names starting with `.`).

## 6. The command

```
voice skills refresh <member_dir>
```

Help line: `Bring your assistant's copy of your skills up to date`. Same preamble as the other
`skills` commands (`expand_home`, `checked_member`, `private::lock_state` held for the whole
command). Output:

1. `copied.lines` as they are.
2. `Copied <a> of <b> skills into your assistant's copy, <mirror, home as ~>.`
3. `Since the last refresh: <n> new, <u> updated, <c> unchanged, <r> removed.`
4. When `waiting > 0`: `<w> skills wait for your yes: voice skills list <member_dir, home as ~>`
   (as `anonymize` prints it, through `report_path`).

Exit 0 when every agreed skill was copied, 1 otherwise (the copies made stay, as with `anonymize`).
On an error the CLI prints it as every other error; the mirror is already removed by § 4.

**No line ever prints a found text, a term, a pattern, an allow entry, a file's contents, a
removed skill's name, or a hash.**

## 7. Tests first

Write them first, run them red (failing on the assertion, or not compiling), then implement. Temp
folders only (`builder/core/tests/common` `Scratch` or what `builder/cli/tests/anonymize.rs` uses),
never the real home; CLI tests run the binary with `env_clear()` and `HOME` set. Reuse the
fixtures in `builder/cli/tests/anonymize.rs` (move shared helpers into the test crate's common
module if both files need them).

`builder/cli/tests/skills_refresh.rs` (Pebble, Rowan Vale, the term `Pebble Freight`):
- **First refresh:** two agreed skills, one with `Pebble Freight` in its `SKILL.md`. The mirror is
  `~/.claude/state/<slug>/mirror/`, folders 0700, files 0600, holding `.anonymized-copy`,
  `.manifest.json` and `<name>/…`; no file in it contains `pebble` in any letter case; the
  placeholder `<company-1>` is there. The stdout lines are exactly § 6's (two new, 0 updated,
  0 unchanged, 0 removed). Exit 0.
- **The manifest:** parse it; its skill names and paths are exactly the files written; each hash
  equals the SHA-256 of that mirror file's bytes (compute it in the test); the SHA-256 of the
  **source** `SKILL.md` that held the term appears nowhere in the manifest.
- **Nothing changed:** a second refresh prints `0 new, 0 updated, 2 unchanged, 0 removed` and the
  manifest bytes are the same.
- **An edit flows through:** change one source file's text; the next refresh says `1 updated`, and
  the mirror's file has the new text.
- **Removed:** withdraw one skill → `1 removed` and its folder is gone; put the other on
  `never_copy` → removed; move a skill's folder (agreement no longer matches) → removed. The removed
  skill's name appears in no stdout or stderr line of that run (use a skill named
  `pebble-freight-notes` for this case, with `Pebble Freight` a listed term, and assert
  `pebble-freight` is absent from both streams).
- **A rule added later:** a skill copied clean; then add a term that appears in it to
  `private.json`; the next refresh refuses it (its VO3 lines), says `1 removed`, exits 1, and the
  mirror holds no file of that skill.
- **A failed refresh removes the mirror:** after a good refresh, make `private.json`'s anonymizer
  object invalid (an unknown key); refresh fails (non-zero exit), stderr ends with
  `MIRROR_REMOVED`'s text, and `mirror/` no longer exists.
- **Not Voice's folder:** put a file `notes.md` (no stamp) in `mirror/` before the first refresh:
  the refresh fails with § 4 step 5's message plus `MIRROR_REMOVED`, and `mirror/notes.md` is still
  there, unchanged.
- **A bad manifest** (not JSON, or `version: 2`, or a 5 MiB file) is not an error: every copied
  skill counts as new.
- **A link:** `mirror` as a symlink to a folder outside: refused, nothing written or deleted at the
  link's target.
- **Forget:** after a refresh, `voice forget <member_dir> --yes` leaves no `mirror/`.

`builder/cli/tests/anonymize.rs`: `anonymize --out` now writes `.manifest.json` (0600) whose
hashes match the copies; every existing test stays green with its output unchanged.

`builder/ingest` unit tests (in `mirror.rs`): `Manifest::read` on each bad shape returns empty;
`to_json` round-trips through `read`; `remove_copy` removes a stamped folder and an empty one,
leaves an unstamped non-empty folder and a link alone (returns `false`).

Then run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test -p voice-ingest -p voice-cli`, and `bash scripts/ci-local.sh`, and report each command
with its result, the red output you saw before implementing, and every file you changed.

## 8. After the security review and the Verifier (V1, 2026-11-01): decision-complete

Same rules as the top of this brief (invented names only, commit nothing). Red first for every
item: write the test, run it, see it fail on its assertion, then fix. Touch only the files named
here and their tests.

### 8.1 A failed refresh that couldn't remove the mirror says so (security review MEDIUM 1)

- `remove_copy` removes in an order that never leaves something that looks like a whole copy:
  after the stamp check passes, delete `.manifest.json` first (when present), then every other
  entry except `.anonymized-copy` (folders with `fs::remove_dir_all`, after `symlink_metadata`;
  files and links with `fs::remove_file`), then the stamp, then the folder itself with
  `fs::remove_dir`. A failure part way returns `Err` and leaves the stamp in place, so the next
  run still recognises the folder as Voice's and can finish removing it.
- A stamp that exists but can't be read (`check::read` fails) is `Ok(false)`, not `Err` (this also
  puts back `voice anonymize --out`'s friendly "Choose an output folder…" message for that case,
  security LOW 3).
- In `refresh`'s error path, append `MIRROR_REMOVED` only when `remove_copy` returned `Ok(true)`
  or the mirror no longer exists. Otherwise append the new sentence
  `Voice couldn't remove your assistant's copy of your skills, so it may be out of date. Delete the folder <path> yourself, then run voice skills refresh again.`
  (a `pub const MIRROR_NOT_REMOVED` holding the text around `<path>`, or a `pub fn`), with
  `<path>` the mirror shown as § 8.3 says.
- Tests (`builder/cli/tests/skills_refresh.rs`, skip when `geteuid() == 0`, restore modes in a
  guard that runs on panic too): after a good refresh, (a) `chmod 000` the stamp and break
  `private.json`: the refresh fails, stderr ends with the not-removed sentence naming
  `~/.claude/state/pebble/mirror`; (b) `chmod 500` one copied skill's folder and break
  `private.json`: the refresh fails with the not-removed sentence, and `.manifest.json` is gone
  while `.anonymized-copy` is still there; after restoring the mode and fixing `private.json`, the
  next refresh succeeds and the mirror is whole again.

### 8.2 Withdrawing a skill removes the mirror (security review MEDIUM 2)

A withdrawn skill must not stay in the mirror until someone refreshes. In the `Withdraw` arm, after
`agreements.save`: if any listed id had a record before the withdraw, and the mirror folder exists
(`private::private_folder_exists(home, slug, "mirror")`), call `remove_copy` on it. Print the usual
`<id>: no longer copied.` lines, then:
- removed: `Your assistant's copy of your skills was removed, so nothing you withdrew stays in it. Make it again with: voice skills refresh <member_dir, home as ~>`;
- not removed (`Ok(false)` or `Err`): `Voice couldn't remove your assistant's copy of your skills. Delete the folder <mirror, home as ~> yourself, then run voice skills refresh <member_dir, home as ~>`, and exit 1.

Both paths go through `report_path` as § 8.3 says. Nothing is printed about the mirror when no
listed id had a record or there is no mirror. Tests: withdraw after a refresh removes `mirror/` and
prints the line; withdraw of an id with no record leaves the mirror; a mirror holding someone
else's file is left and the command exits 1.

### 8.3 Small cleanups (V1's review, the Verifier's should-fix)

- `builder/cli/src/skills.rs`: move refresh into its own `fn refresh(member_dir: PathBuf) -> Result<ExitCode>`
  called from `run` the way `anonymize` is, so `run`'s `match` has no `unreachable!` arm. Load the
  rules once: return them in `Refreshed` (`pub rules: Rules`) instead of calling
  `Rules::for_member` a second time for the waiting line.
- Print the mirror path through `anonymize::report_path(&PathBuf::from(display_path(...)), &rules)`,
  as `anonymize` prints its `--out`. This is the Verifier's should-fix: an assistant whose own name
  holds a listed term (slug `pebble-freight`, term `Pebble Freight`) must print
  `~/.claude/state/*/mirror`, as `anonymize --out ~/pebble-freight-out` prints `~/*`. Apply the same
  wrap to the path in § 4 step 5's message and in 8.1's not-removed sentence (the rules are loaded
  by then; when the error came from loading the rules themselves, use `display_path` alone). Test
  it with that slug.
- `mirror.rs` § 4 step 5: replace the refusal message only when the error is
  `prepare_output_with_projects`' own "Choose an output folder…" refusal (compare the message);
  any other error (a stamp that is a link, an I/O error) passes through unchanged.
- `copy.rs`: the manifest path uses `to_string_lossy()` instead of `.expect("UTF-8 skill path")`
  (`write_skill` already refused a non-UTF-8 path), so this code has no panic path.

### 8.5 Never tell someone to delete a folder holding their own files (V1's review of 8.1)

After 8.1, a refresh refused because the mirror holds files Voice didn't put there (§ 4 step 5)
ends with the not-removed sentence: "…so it may be out of date. Delete the folder … yourself".
Both halves are wrong there: such a folder holds no copy of Voice's (a stamped one is emptied, not
refused), and deleting it would delete the person's own files.

- `mirror.rs`: when the closure's error is the § 4 step 5 refusal (the one `output_error`
  rewrites), do not call `remove_copy`, and return exactly
  `<shown> holds files Voice didn't put there. Move them away, then run voice skills refresh again.`
  Carry that case out of the closure as its own variant (a small private `enum Failure { Foreign,
  Other(Error) }`, or `output_error` returning a marker), never by comparing message text again.
  Every other error keeps 8.1's behaviour.
- The not-removed sentences (refresh's `mirror_not_removed` and withdraw's) say to move the
  person's files out before deleting. Exact texts:
  - refresh: `Voice couldn't remove your assistant's copy of your skills, so it may be out of date. Move anything you want to keep out of the folder <path>, delete the folder, then run voice skills refresh again.`
  - withdraw: `Voice couldn't remove your assistant's copy of your skills. Move anything you want to keep out of the folder <mirror>, delete the folder, then run voice skills refresh <member_dir>`
- Tests (`builder/cli/tests/skills_refresh.rs`): the not-Voice's-folder test and the
  `pebble-freight` slug test expect the new whole message (`~/.claude/state/pebble/mirror` and
  `~/.claude/state/*/mirror`), and that stderr does not contain `couldn't remove` or `Delete`;
  `notes.md` is still there. The 8.1 and 8.2 not-removed tests expect the new texts.
- Delete the test `refresh_uses_its_own_handler_and_loaded_rules_without_path_panics`: it reads
  the source's text, not what the program does. 8.3's behaviour is covered by the `pebble-freight`
  slug tests, and the absence of `unreachable!`/`expect` needs no test.

### 8.6 The repeat Verifier and the second security review (V1, 2026-11-01)

Both reproduced these on `2984159`. Same rules: red first, invented names, commit nothing.

**8.6.1 No error names the mirror's real path (Verifier must-fix; security MEDIUM).** With slug
`pebble-freight` and term `Pebble Freight`, a refresh that fails inside `remove_copy` (a copied
skill's folder at 0500) prints `voice: /…/.claude/state/pebble-freight/mirror/notes: Permission
denied (os error 13). …`: the home folder, the slug and a skill's name. A mirror that is a link or
0755 leaks the same way through `private::private_folder`'s own message.
- In `mirror.rs`, add `fn masked(error: Error, raw: &Path, shown: &str) -> Error`:
  - `Error::Io { source, .. }` becomes `Error::Invalid(format!("Voice couldn't change {shown}: {source}"))`
    (the OS reason only; the error's own path is never shown);
  - `Error::Invalid(m)` becomes `Error::Invalid(m.replace(<raw as text>, shown))`, where `<raw as
    text>` is each of `raw.display()` and `display_path(raw, home)` (pass `home` in too);
  - any other variant: its `Display`, with the same two replacements.
- `refresh` computes the mirror's path **before** opening it (`home/.claude/state/<slug>/mirror`,
  the path `private_folder` returns; reuse the helper `private.rs` uses for it if there is one,
  else build it the same way) and `shown` from it, then calls `private_folder` inside the masked
  handling: its error goes through `masked` and gets no removal sentence (nothing was opened).
  Every `Failure::Other` error goes through `masked` before 8.1's sentence is appended.
- `voice anonymize --out` is out of scope here (its `--out` is a folder the person names).

**8.6.2 Withdraw never stops before the mirror line (Verifier should-fix; security LOW).** In the
`Withdraw` arm, an `Err` from `private_folder_exists`, `private_folder` or `remove_copy` is the
not-removed case: print 8.5's withdraw line (mirror path through `report_path`) and exit 1, never
`?`. Nothing is printed about the mirror only when no id had a record or `private_folder_exists`
is `Ok(false)`.

**8.6.3 A copy whose stamp can't be read is a stale copy, not someone else's files (Verifier
should-fix; security LOW).** `Failure::Foreign` only when, at the moment
`prepare_output_with_projects` returns `output_bad()`, the mirror holds at least one entry and none
named `.anonymized-copy` (check with `symlink_metadata` on `mirror/.anonymized-copy`: any kind of
entry there, readable or not, means *not* Foreign; read the folder with `fs::read_dir` for the
"at least one entry"). Every other `output_bad()` is `Failure::Other` and takes 8.1's path.
- In 8.1's path, when `remove_copy` did not remove the mirror and the mirror is a real folder (not
  a link) with an entry named `.anonymized-copy`, also remove `mirror/.manifest.json` when
  `symlink_metadata` shows a regular file (ignore that removal's result), so no stale copy keeps a
  manifest a reader would trust.

**8.6.4 Never drop the stamp while something else is left (security LOW).** In `remove_copy`,
after the loop and before removing the stamp, list the folder again: if any entry other than
`.anonymized-copy` is there (a `.DS_Store` written meanwhile), return an `Err` (the `Io`
`DirectoryNotEmpty` kind on the folder) and leave the stamp, so the next run finishes the job.
The same-user swap of the folder for a link between the check and the deletes stays a known limit
(the parents are 0700, only the person can do it); V1 records it in `.ai/SECURITY.md`.

**Tests** (`builder/cli/tests/skills_refresh.rs`, skip when `geteuid() == 0`, modes restored in a
guard):
- slug `pebble-freight`: refresh, then `chmod 500` the copied skill's folder and refresh again
  (every refresh empties the mirror first, so this one fails inside `remove_copy`): exit 1, stderr contains
  `~/.claude/state/*/mirror`, ends with the not-removed sentence, and contains neither
  `pebble-freight`, `notes` (the skill's folder name) nor the scratch home's path;
- slug `pebble-freight`, mirror set to 0755 after a refresh: refresh exits non-zero and stderr has
  no `pebble-freight` and no scratch home path; `withdraw` of a recorded id prints 8.5's withdraw
  line with `~/.claude/state/*/mirror`, exits 1, stderr empty;
- slug `pebble`, valid rules, stamp at `chmod 000` after a refresh: refresh exits 1, stderr ends
  with the not-removed sentence and does not contain `didn't put there`; `.manifest.json` is gone
  and `.anonymized-copy` is still there. Update 8.1(a)'s test the same way (manifest now gone).
- 8.6.4 needs no new test (a file can't be slipped in mid-loop from a test); say so.

**8.6.5 The real path too (V1's review of 8.6.1).** `private.rs` builds its paths from
`fs::canonicalize(home)`, so on macOS a home under `/var/folders/…` or `/tmp` shows up as
`/private/var/…`, and replacing the non-canonical text leaves `voice: /private~/.claude/state/*/mirror
can be opened by others…`. In `masked`, first replace the canonical mirror path
(`fs::canonicalize(home)` joined with `.claude/state/<slug>/mirror`, when `canonicalize` succeeds),
then the two 8.6.1 forms. Test: in `refresh_masks_errors_while_removing_or_opening_mirror`'s 0755
case, assert stderr starts with `voice: ~/.claude/state/*/mirror can be opened by others` and
contains no `/private`; red first (it fails today with `/private~/…`).

### 8.7 The third security review and the delta Verifier (V1, 2026-11-01)

Reproduced on `0b30b89`. Same rules: red first, invented names, commit nothing.

**8.7.1 A containment refusal never removes anything (security MEDIUM, a regression from 8.6.3).**
`prepare_output_with_projects` refuses (`output_bad()`) when the output is inside a skill root,
overlaps a skill folder, or holds a project, *before* its own `remove_copy`, to protect those
folders. Since 8.6.3 a stamped mirror's refusal is `Failure::Other`, so refresh's error path calls
`remove_copy` and deletes them: with `"projects":["~/.claude/state/pebble-freight/mirror/proj"]`
and `mirror/proj/work.txt`, a refresh deletes `work.txt`. Withdraw (8.2) calls `remove_copy` with
no containment check at all.
- `output.rs`: split `prepare_output_with_projects` into
  `pub fn check_output(out, home, skills, projects) -> Result<PathBuf>` (everything up to and
  including the `projects` loop; returns the canonical `out`) and
  `pub fn finish_output(out: &Path) -> Result<PathBuf>` (the `out.exists() && !remove_copy` step,
  `make_dirs`, the stamp). `prepare_output_with_projects` becomes the two in a row, unchanged in
  behaviour for `voice anonymize --out` (its tests stay green as they are).
- `mirror.rs` `refresh`: call `check_output`; its error is a new `Failure::Overlap` (no
  `remove_copy`). Then `finish_output`; only its `output_bad()` is classified by 8.6.3's rule
  (entries and no stamp entry: `Foreign`; else `Other`). Drop the message comparison in
  `output_error`: classify by which call failed. `Overlap`'s whole message is
  `<shown> overlaps one of your skill or project folders, so Voice left it as it is. Move that folder out of it, then run voice skills refresh again.`
- A shared `pub fn drop_manifest(mirror: &Path)` in `mirror.rs`: 8.6.3's removal (a real folder,
  not a link, with a `.anonymized-copy` entry; `.manifest.json` a regular file; result ignored).
  Refresh calls it on every failure except `Foreign` (so on `Overlap`, on 8.1's not-removed path,
  and when `private_folder` fails, on the unopened path).
- Withdraw (`builder/cli/src/skills.rs`): before `remove_copy`, call `skills::check_output(&mirror,
  &home, &found.skills, rules.projects())`; an `Err` is the not-removed case. On every not-removed
  case, call `drop_manifest` on the mirror's path before printing 8.5's line.

**8.7.2 Mask the whole state folder, not only the mirror (Verifier should-fix; security LOW).** A
state folder at 0775, or `anonymizer/` at 0755, prints `/private/…/.claude/state/pebble-freight…`.
In `masked`, replace the state folder's three forms (canonical, raw, `~`), longest first, with
`shown` minus its trailing `/mirror` (so `~/.claude/state/*` for slug `pebble-freight`, and
`~/.claude/state/pebble` when the slug holds no term). That covers the mirror forms too. Errors
raised before `skills::refresh` (`lock_state`, `checked_member`) and withdraw's `agreements.save`
are out of scope; V1 records them in `.ai/SECURITY.md`.

**8.7.3 An I/O error names its own file (Verifier should-fix).** `masked`'s `Error::Io { path,
source }` arm: when `path` is under the state folder (any form), show it masked as 8.7.2 says, cut
to the mirror folder when it is inside the mirror (no skill name below it):
`Voice couldn't change <masked path>: <source>`. A path outside the state folder:
`Voice couldn't read or change a file: <source>`.

**Tests** (`builder/cli/tests/skills_refresh.rs`; skip when `geteuid() == 0`; modes restored in a
guard):
- slug `pebble-freight`: refresh, then `mirror/proj/work.txt` and the project listed as above:
  refresh exits 1, stderr is exactly `voice: ` + the `Overlap` message with `~/.claude/state/*/mirror`,
  `work.txt` is still there with its text, `.manifest.json` is gone; withdraw of a recorded id
  exits 1 with 8.5's line, `work.txt` still there, `.manifest.json` gone.
- slug `pebble-freight`: state folder at 0775 after a refresh: refresh stderr has no
  `pebble-freight`, no `/private` and no scratch home path. If the error comes from a step before
  `skills::refresh` (so 8.7.2 can't reach it), drop this test and say so in the report.
- slug `pebble`: `anonymizer/placeholders.json` at 0000 after a refresh: stderr contains
  `~/.claude/state/pebble/anonymizer/placeholders.json` and not `couldn't change ~/.claude/state/pebble/mirror:`.
- Unit (`output.rs`): `check_output` refuses a project inside `out` and leaves it; `finish_output`
  still removes a stamped copy.

### 8.8 Never empty the mirror before the overlap check has passed (V1, 2026-11-01)

The delta Verifier (FAIL) and the security review (HIGH) on `50c7f40` both reproduced it: a
refresh that fails **before** `check_output` runs (`rules?` with an unknown key in `private.json`,
or `load_agreed` with `anonymizer/placeholders.json` at 0000 or `anonymizer/` at 0755) is
`Failure::Other`, and the error path calls `remove_copy` with no containment check. With
`"projects":["~/.claude/state/pebble-freight/mirror/proj"]` and `mirror/proj/work.txt`, `work.txt`
is deleted. When the rules didn't load, the projects aren't even known.

- `mirror.rs`: a new `Failure::Unchecked(Error)` for every error raised before `check_output`
  returns `Ok` (`rules?` and `load_agreed`). Its error path never calls `remove_copy`: it calls
  `drop_manifest(&mirror)` and returns `<masked message>. <MIRROR_STALE>`, with
  `pub const MIRROR_STALE: &str = "Voice marked your assistant's copy of your skills as out of date and left its files alone. Fix this, then run voice skills refresh again.";`
  (`masked` and the trailing-full-stop rule as in 8.1). `Failure::Other` (after the check passed)
  keeps 8.1's path.
- When the rules fail to load, `shown` is the mirror's `~` path with the slug replaced by `*`
  (`~/.claude/state/*/mirror`), never the raw slug: the terms are unknown, so assume the slug holds
  one. `masked` uses that `shown` too.
- Tests (`builder/cli/tests/skills_refresh.rs`, skip when `geteuid() == 0`, modes restored in a
  guard): slug `pebble-freight`, a refresh, then the project above with `work.txt`, and (a)
  `placeholders.json` at 0000, (b) an unknown key in `private.json`: each refresh exits 1, stderr
  ends with `MIRROR_STALE`, contains `~/.claude/state/*/mirror` wherever it names the mirror and no
  `pebble-freight`, `work.txt` keeps its text, `.manifest.json` is gone, `.anonymized-copy` stays.
  Every existing test that breaks `private.json` to force a failure (§ 7's "a failed refresh
  removes the mirror", 8.1(a), 8.1(b)) now expects `MIRROR_STALE`, the copies left in place and
  the manifest gone; 8.1's not-removed and removed sentences stay covered by the tests that fail
  after the check (the 0500 skill folder and the stale stamp with valid rules).
- Out of scope, recorded by V1 in `.ai/SECURITY.md` at the land: same-user races (the mirror
  swapped for a link between `drop_manifest`'s check and its unlink, or between `check_output` and
  `finish_output`); leaks before `skills::refresh` (`lock_state`, `checked_member`), in withdraw's
  `agreements.save`, and `voice skills list` showing a skill linked into the state folder by its
  real path.

### 8.4 Verify and report

Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test -p voice-ingest -p voice-cli`, then `bash scripts/ci-local.sh` (its macOS sandbox
tests can't run inside your own sandbox; say so if that is where it stops). Report each command
and its result, the red output you saw before each fix, and every file you changed.
