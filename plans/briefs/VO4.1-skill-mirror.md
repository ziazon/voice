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
