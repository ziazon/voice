# PB8.9b2 brief: take the newest version (the engine, the app command, `voice open --update`)

Lane V, runner V1, 2026-11-13. Card `pHz6miQ5` (split from PB8.9b). Codex implements; Claude reviews.

Read first: `AGENTS.md` (no real person in any file, fixture or string: invented names only, such as Wren, Ari and
Rowan Vale), `.ai/PROJECT.md` (the path-to-test table), `.ai/DECISIONS.md`, `.ai/SECURITY.md`, then
`plans/briefs/PB8.9b1-open-saved.md` (the slice this one builds on: same crates, same helpers, same test fixtures).
Skills: `~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (every sentence the person sees), `~/.codex/skills/code-quality/SKILL.md`.

## Why

PB8.9b1 opens an assistant saved on GitHub on a second computer (a **member computer**: `save.json` is
`SaveRecord::Github { role: Member, builder, .. }` and `~/.claude/state/<slug>/backup.git` holds `refs/heads/main`
as last taken). The building computer keeps saving new versions to the repo; nothing on the member computer takes
them yet. This slice adds *take the newest version*: fetch `main`, fast-forward only, and replace only the files Voice
wrote from the saved copy. Screens are PB8.9b3; nothing visual here.

## Decisions (locked; don't re-decide)

- Only a member computer takes new versions. A building computer, a local save or no save refuses with
  `text::not_a_member`.
- **Fast-forward only.** The new `main` must have the last-taken `main` as an ancestor. A rewritten history refuses
  (`text::not_newer`) and changes nothing.
- **The files Voice wrote** are the entries of the last-taken tree under `crew/<slug>/` as `opening_entries` lists
  them (so never `private.json`, never `.DS_Store`). Only those, plus the new tree's entries, are ever written or
  removed. Every other file in the member folder (`private.json` above all) is never read, written or removed.
- **A file changed on this computer stops it, named.** Before anything is fetched or written: for each last-taken
  entry, the file in `~/.claude/crew/<slug>/` must be either missing (fine: the new version writes it if it still has
  it) or a regular file (by `symlink_metadata`; a link or a folder counts as changed) whose bytes equal the
  last-taken blob. A path that the **new** tree adds and that already exists on disk (not in the last-taken tree)
  also stops it. The sentence names the first such path, relative to the member folder, in sorted order.
- Consent is **not** asked again (the person already agreed on this computer, PB8.9b1 cleared it on open).
- Validation of the new version is exactly PB8.9b1's (`opening_entries`' refusals and limits, `private.json` never
  arriving, `read_member` with the same slug and the same private folder, `check_name` on `name` and `principal`,
  the privacy check). Factor that block of `stage_open` into one function both use; `stage_open`'s behaviour and
  every existing test stay unchanged.
- **Refresh in Claude Code:** after a successful update that changed at least one file, if `install::status` says
  this assistant is the one turned on (`use_it::is_this`, made `pub(crate)`), call `install::turn_on(member_dir,
  places)`, which already refreshes the same slug in place. A refresh failure doesn't undo the update; the message
  says so (below).
- The state lock: the API holds `voice_ingest::private::lock_state(home, slug)` for the whole take, as `pack` and
  `forget` do in `use_it.rs`.
- The record: if the new head's `Saved-on:` trailer is a valid computer name and differs from the record's
  `builder`, rewrite `save.json` with the new builder (same `write_record`); otherwise leave it byte for byte.

## What to build

### 1. `voice-save`

**a. `git.rs`.** `pub fn fetch_main_into(git, git_dir, token, dest_ref: &str) -> Result<()>` with refspec
`+refs/heads/main:<dest_ref>`; `fetch_main` becomes a one-line call with `"refs/heads/main"`. Accept only
`dest_ref` values `refs/heads/main` and `refs/voice/incoming` (anything else → `Error::Invalid`), so no caller can
pass an option-shaped string. The existing `fetch_checks_transferred_objects` test keeps passing.

**b. `flow.rs`.** `opening_entries` takes the revision it reads (`"refs/heads/main"` or `"refs/voice/incoming"`)
instead of hard-coding it. Add:

```rust
pub enum Took { AlreadyNewest { full: String }, Updated { full: String, changed: usize } }
impl Saver<'_> {
    pub fn take_newest(&self, home: &Path, member: &MemberPaths, stamp: &str, privacy: &dyn PrivacyCheck) -> Result<Took>;
}
```

`take_newest`, in this order (any error before step 7 changes nothing in `~/.claude/crew/<slug>/` or `save.json`):
1. Read the record from `member.state_dir`. Not `Github { role: Member, .. }` → `Err(Invalid(text::not_a_member(name)))`
   before any git or HTTP call. Validate `stamp` (`voice_core::backup::valid_stamp`). `self.github` required
   (`NOT_SET_UP`), `check_version`, token (`SIGN_IN_FIRST`), `github.account`, `github.confirm_private(repo)`.
2. `git_dir = state_dir/backup.git`; `old = git rev-parse --verify refs/heads/main^{commit}`.
3. The local-change check (Decisions) against `opening_entries(git, git_dir, repo, slug, "refs/heads/main")`.
   Compare bytes in Rust: open with `O_NOFOLLOW` (`std::os::unix::fs::OpenOptionsExt::custom_flags(libc::O_NOFOLLOW)`),
   read at most 10 MiB + 1, compare with `git cat-file blob <oid>`. Stop → `text::changed_here(name, rel)`.
4. `fetch_main_into(.., "refs/voice/incoming")`; `new = rev-parse --verify refs/voice/incoming^{commit}`.
   `new == old` → delete the incoming ref (`update-ref -d`) and return `AlreadyNewest`.
   `git merge-base --is-ancestor <old> <new>`: exit 1 → delete the ref, `Err(Invalid(text::not_newer(full)))`;
   any other non-zero exit → the usual git error. (Add a small `Git` helper returning the exit status if `run`
   can't; keep the scrubbed-error behaviour.)
5. Stage the new version: a 0700 folder `~/.claude/crew/.<slug>.voice-updating-<stamp>-<pid>` (made with
   `DirBuilder::mode(0o700).create`, failing if it exists; the dot keeps `assistants::list` away), its `member/`
   written from `opening_entries(.., "refs/voice/incoming")` with `write_opened_blob`, then the shared validation.
   Re-run the step 3 check for paths the new tree **adds** (exists on disk, not in the old tree → `changed_here`).
6. The plan: `changed` = new entries whose oid or executable bit differs from the old entry or that the old tree
   lacks; `removed` = old entries the new tree lacks. Both empty (only `private.json`-free metadata moved) → still a
   valid update with `changed: 0`; go on to step 8.
7. Apply. For each `changed` path, sorted: make missing parent folders 0700 one component at a time without
   following a link (the helper `write_opened_blob` uses), re-check the target as in step 3 (missing, or equal to
   its old blob), then `fs::rename(staging/member/<rel>, member_dir/<rel>)`. For each `removed` path: re-check, then
   `fs::remove_file`, then `fs::remove_dir` on each parent up to (not including) the member folder, stopping at the
   first that isn't empty. **Rollback on any failure here or in step 8:** for every path already applied, write back
   its old blob (mode 0644, or 0755 when the old entry was executable; through a temporary file in the same folder
   and a rename) or remove it when the old tree didn't have it; remove folders this step made, when empty. Then
   return the original error.
8. `git update-ref refs/heads/main <new> <old>`, then `git update-ref -d refs/voice/incoming`. The builder trailer
   rule (Decisions). Return `Updated { changed: changed.len() + removed.len() }`.
Always (success or failure, from step 5 on): remove the staging folder, and delete `refs/voice/incoming` if it
exists.

**c. `text.rs`** (each recognised by `is_said`, the way PB8.9b1's sentences are):
- `not_a_member(name)`: "This computer saves <Name>, so there's no newer version to take here."
- `changed_here(name, rel)`: "<rel> in <Name>'s folder changed on this computer, so Voice took nothing. Put it back
  as it was, or move it somewhere else, then try again."
- `not_newer(full)`: "The copy saved in <full> doesn't build on the version on this computer, so Voice took nothing."
`rel` and `full` are already validated plain names; still pass them through the same control-character scrub the
other sentences use.

### 2. `voice-api` (`builder/api/src/save.rs`)

`pub fn take_newest_in(home, claude_home, request: dto::SlugRequest, git, github, tokens) -> Result<dto::TookNewest>`
(and `take_newest(request)` using the real places, as the file's other pairs do):
1. `lock_state(home, slug)`; `assistants::find(home, slug)` (none → the existing "no such assistant" failure the
   file uses); `MemberPaths` for it.
2. `saver.take_newest(home, &member, &local_stamp(now), &FolderPrivacy)`.
3. Messages (`<n>` with "1 file" / "<n> files"):
   - `Updated`: "Took the newest version of <Name> from <full>: <n> changed." When the refresh ran: append
     " Voice refreshed <Name> in Claude Code too." When it failed: append " Voice couldn't refresh <Name> in Claude
     Code; turn <Name> on again to do it."
   - `AlreadyNewest`: "<Name> is already the newest version saved in <full>."
4. Errors pass through only when `text::is_said`; anything else → "Voice couldn't take the newest version of <Name>.
   Try again."
DTO `TookNewest { assistant: dto::Assistant, changed: u32, message: String }` (camelCase, `#[ts(export)]`); the
request is the existing `SlugRequest`. Regenerate `builder/ui/src/types/generated/` the way the repo does.

### 3. App wiring, no screens

One command `take_newest`, placed right after `open_saved` in every list PB8.9b1 touched: `command_names.rs`,
`commands.rs`, `main.rs`, `capabilities/default.json`, `permissions/autogenerated/take_newest.toml`,
`app/tests/lockdown.rs`, `api/src/parity.rs` (`&[&["open", "--update"]]`), `backend.ts`
(`takeNewest(request: SlugRequest): Promise<TookNewest>`), `app/tauri-backend.ts` and its test row,
`test-support.ts`'s fake.

### 4. CLI

`voice open --update <slug>`: takes the newest version and prints the message; a failure prints the sentence and
exits 1. `--update` with an `<owner>/<repo>` argument, or with no slug, is a usage error. Help text in plain words.

## Tests (red first; each must fail on today's code for the right reason)

`voice-save` (`builder/save/tests/open.rs` or a new `take.rs`; scratch homes; the PB8.9b1 fixture: a local bare
repo standing in for GitHub through the wrapper's `insteadOf`; `FakeHttp`; never the network):
- open Wren in home B from repo R; save a new version in home A (a changed `SKILL.md`, a new file `notes/b.md`, a
  removed file); `take_newest` in B → files byte-equal to A's, the removed one gone (and its emptied folder),
  `private.json` in B untouched (write one first, check its bytes), `refs/heads/main` at the new head, no
  `refs/voice/incoming`, no staging folder, `Updated { changed: 3 }`.
- run it again → `AlreadyNewest`, nothing changed (compare a full listing with bytes before and after).
- a building computer's record, a local record, no record → `not_a_member`, the wrapper log empty, `FakeHttp` saw
  nothing.
- a changed file in B → `changed_here` naming it; a link in its place → `changed_here`; a missing file → takes it
  (the file comes back); a new-tree path already on disk → `changed_here`. Each refusal leaves B byte-identical.
- a force-pushed `main` in R not containing B's head → `not_newer`, B unchanged, no incoming ref.
- each PB8.9b1 refusal reached through the new version (a symlink entry, a `private.json`, a privacy hit, a
  `member.json` whose state folder points elsewhere) → refused, B unchanged, no staging folder.
- rollback: make one apply step fail (a read-only parent folder for the second changed path, chmod restored at the
  end of the test) → the error, and B byte-identical to before (including the first path, put back).
- the builder trailer: a new head saved on "Studio" → `save.json` names Studio; same builder → `save.json`
  byte-identical.
`voice-api` (`builder/api/tests/save.rs`): `take_newest_in` with the assistant turned on (as the `use_it` tests turn
one on) refreshes the installed `SKILL.md` to the new bytes and the message says so; turned off → no refresh, no
refresh words; a non-member → the `not_a_member` sentence.
CLI: `voice open --help` mentions `--update`; parity passes.
Mutation-check each claim (`cp` backup, `cmp`; never `git checkout --`).

## Never

- No real person's name, handle or text anywhere.
- Never write outside `~/.claude/crew/<slug>/`, `~/.claude/state/<slug>/backup.git`, `save.json`, the staging
  folder and (through `install::turn_on` only) Claude Code's install. Never follow a link while writing. Never read,
  write or remove `private.json` or any file the last-taken tree doesn't list.
- Never push, never force, never call any GitHub endpoint PB8.9b1 doesn't. No new dependency. No network in tests.
- A token never reaches argv, a file or an error message.

## Gate

`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-save -p voice-api -p voice-cli` plus the app
crate's lockdown test, `pnpm -C builder/ui test` and `pnpm -C builder/ui run typecheck`. Your sandbox blocks sockets,
so the Ask tests may fail there; say which. Don't commit; Claude reviews and commits.
