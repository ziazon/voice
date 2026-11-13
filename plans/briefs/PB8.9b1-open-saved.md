# PB8.9b1 brief: open an assistant you saved on GitHub (the engine, the app commands, `voice open`)

Lane V, runner V1, 2026-11-13. Card `wmDEW06R` (split from PB8.9b). Codex implements; Claude reviews.

Read first: `AGENTS.md` (no real person in any file, fixture or string: invented names only, such as Wren, Ari and
Rowan Vale), `.ai/PROJECT.md` (the path-to-test table), `.ai/DECISIONS.md`, `.ai/SECURITY.md`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`,
`~/.codex/skills/plain-language/SKILL.md` (every sentence the person sees), `~/.codex/skills/source-driven-development/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`.

## Why

One assistant is used on several computers. One computer builds her and saves her (paraphrase only) to a private
GitHub repo with *Save* (`builder/save/src/flow.rs`, `SaveRecord::Github`, branch `main`, folder `crew/<slug>/`).
Today nothing can open that saved copy on a second computer: there is no clone, fetch or pull anywhere. This slice
adds opening it. A computer that opened her is a **member computer**: it never saves her (only the building computer
does); taking newer versions is the next slice (PB8.9b2), and the screens are PB8.9b3. No screen changes here beyond
the generated types and the backend method stubs the app wiring needs.

## What to build

### 1. `voice-save` (`builder/save/src`)

**a. The record knows a member computer** (`record.rs`). `SaveRecord::Github` gains two fields:
`role: Role` and `builder: Option<String>`, with `pub enum Role { Builder, Member }`.
- On disk, a building computer's record is unchanged byte for byte: no `role` key, no `builder` key. A member
  computer's record adds `"role": "member"` and, when known, `"builder": "<computer name>"`.
- `read_record`'s github key set becomes: `mode`, `owner`, `name`, `branch`, plus optional `role` (only the string
  `"member"`; anything else is invalid) and optional `builder` (only with `role`, and only a string that passes
  `valid_computer_name`, below). Every other key is still refused. Keep `deny_unknown_fields` honest: make the serde
  shape match (skip-serializing the defaults), and keep the strict key check.
- An older Voice reading a member record refuses it ("not a save record"), so it can't save from a member computer.
  That is intended; say so in a code comment.

**b. A computer's name** (new `computer.rs`, `pub mod computer;`). `pub fn name() -> Option<String>`:
on macOS run `/usr/sbin/scutil --get ComputerName` (absolute path, `env_clear()`, `PATH=/usr/bin:/bin`, stdin null,
stdout piped, stderr null); elsewhere, or when that fails, `libc::gethostname` with a trailing `.local` removed.
Return it only when `valid_computer_name` holds: 1 to 64 characters after trimming, no control characters, none of
`@ < > { } [ ] ( ) # \ ` " $` and no newline. `pub fn valid_computer_name(&str) -> bool`.

**c. Save names the computer it runs on.** Add `Saver::save_now_from(&self, member, home, privacy, remotes,
computer: Option<&str>)`; `save_now` calls it with `None` (keep `save_now`'s signature; its 20-odd call sites stay).
When `computer` is `Some` and valid, the commit message is `Save <Name>\n\nSaved-on: <computer>\n` (a git trailer),
else `Save <Name>` as today. The api and the CLI call `save_now_from(..., computer::name().as_deref())`.

**d. A member computer never saves or re-chooses.** `save_now_from`, `choose_local`, `choose_existing` and
`choose_github` first read the record; when it is `Github { role: Member, .. }` they return
`Err(Invalid(text::member_computer(&member.member.name, builder.as_deref())))` before any privacy check, git or
HTTP call:
- with a builder: "Voice saves <Name> only from <builder>, the computer that builds <Name>. This computer takes each
  new version instead."
- without: "Voice saves <Name> only from the computer that builds <Name>, the one that first saved <Name> to
  GitHub. This computer takes each new version instead."
Add its template to `is_said`'s recognition (the same way `privacy_hits` is recognised).

**e. Finding saved assistants on GitHub** (`github.rs`). Source: the REST docs for
`GET /user/repos` and `GET /repos/{owner}/{repo}/contents/{path}`
(https://docs.github.com/en/rest/repos/repos#list-repositories-for-the-authenticated-user,
https://docs.github.com/en/rest/repos/contents#get-repository-content); cite them in doc comments as the file does.
```rust
pub struct SavedAssistant { pub repo: RepoRef, pub slug: String, pub name: String }
impl Github<'_> {
    pub fn saved_assistants(&self, token: &Secret) -> Result<Vec<SavedAssistant>>;
    pub fn saved_in(&self, token: &Secret, repo: &RepoRef) -> Result<Vec<SavedAssistant>>;
}
```
- `saved_assistants`: `GET /user/repos?visibility=private&affiliation=owner,collaborator,organization_member&sort=pushed&per_page=50&page=<n>`
  for n = 1 and, only when page 1 held 50, n = 2 (at most 100 repos; the 1 MiB body cap stays). 401 →
  `SIGN_IN_FIRST`; other non-200 → `GITHUB_UNREACHABLE`. Keep a repo only when its body says `"private": true`, its
  `owner.login` and `name` pass `valid_owner`/`valid_repo_name`, and either its `description` is exactly
  `"My AI assistant, saved by Voice"` (make that a `pub const VOICE_DESCRIPTION` and use it in
  `create_private_repo` too) or its name ends with `-assistant` (Voice's suggested name). For each kept repo call
  `saved_in`. Sort the result by repo full name, then slug.
- `saved_in`: `GET /repos/<full>/contents/crew?ref=main`. 404 → empty. A 200 that isn't an array → empty. For each
  entry with `"type": "dir"` and a name that is a canonical slug (`voice_core::crew::slugify(n) == Some(n)`), at most
  20 per repo: `GET /repos/<full>/contents/crew/<slug>/member.json?ref=main`. Keep it only when the reply is 200, an
  object with `"type": "file"`, `"encoding": "base64"`, and its `content` (newlines removed, then base64-decoded,
  at most 64 KiB) is a JSON object whose `slug` equals the folder name and whose `name` passes
  `voice_core::crew::check_name("name", …)`. Anything else in one repo skips that entry, never the whole list. 401 →
  `SIGN_IN_FIRST`.

**f. Fetch only, into a fresh bare repo** (`git.rs`). Factor the auth header in `push` into one private helper
used by both, and add:
```rust
pub fn fetch_main(git: &Git, git_dir: &Path, token: &Secret) -> Result<()>;
```
running `git fetch --no-tags --no-recurse-submodules -- origin +refs/heads/main:refs/heads/main` (no
`--porcelain`: it needs Git 2.41 and Voice's floor is 2.32)
with `Env::Own` and the header, the same error scrubbing as `push` (put the shared scrubbing in one helper), and a
fixed sentence `text::fetch_failed(detail)` recognised by `is_said`.

**g. Opening** (`flow.rs`). A staged open that touches nothing of the person's until it has checked everything:
```rust
pub struct Staged { pub dir: PathBuf, pub member_dir: PathBuf, pub git_dir: PathBuf, pub member: voice_core::member::Member, pub builder: Option<String> }
impl Saver<'_> {
    pub fn stage_open(&self, home: &Path, repo: &RepoRef, slug: &str, stamp: &str, privacy: &dyn PrivacyCheck) -> Result<Staged>;
}
pub fn place_opened(staged: &Staged, home: &Path, repo: &RepoRef) -> Result<MemberPaths>;
```
`stage_open`, in this order:
1. Validate `slug` (canonical slug), `repo` (`valid_owner`, `valid_repo_name`), `stamp` (the backup stamp format;
   reuse `voice_core::backup`'s rule, made `pub`). `self.github` is required (`NOT_SET_UP`), `check_version`, token
   (`SIGN_IN_FIRST`), `github.account`, `github.confirm_private(repo)`.
2. Make the staging folder `~/.claude/crew/.<slug>.voice-opening-<stamp>-<pid>` with mode 0700 (`create_dir_all`
   for `~/.claude/crew` first; the staging folder itself with `DirBuilder::mode(0o700).create`, failing if it
   exists). Its name starts with a dot, so `assistants::list` skips it. On any later error in `stage_open`, remove it.
3. In it: `git init --bare backup.git` (`init_bare`), `remote add origin https://github.com/<full>.git`,
   `fetch_main`.
4. `git ls-tree -r -z --full-tree refs/heads/main -- crew/<slug>/`. Parse each `<mode> <type> <oid>\t<path>` entry.
   Refuse (the sentence below, nothing kept) when: there is no `crew/<slug>/member.json`; any entry's mode is not
   `100644` or `100755` or its type is not `blob` (a link, `120000`, or a submodule, `160000`); any relative path
   (below `crew/<slug>/`) has an empty, `.` or `..` component, a backslash or a control character; the builtin deny
   list (`voice_core::deny::DenyList::builtin().denied_relative`) denies it; more than 2,000 files; any blob over
   10 MiB (`git cat-file -s`) or all of them over 50 MiB. Skip (never write) `private.json` and `.DS_Store`.
   Sentence: `text::saved_copy_refused(&full, rel)`: "The saved copy in <full> holds <rel>, which Voice doesn't open
   (a link, a private file or something that isn't a plain file), so it opened nothing." For the size and count
   limits use "The saved copy in <full> is bigger than an assistant should be, so Voice opened nothing."
5. Write each blob (`git cat-file blob <oid>`) into `<staging>/member/<rel>`: folders 0700 made one component at a
   time (never following a link), files with `create_new(true)`, `O_NOFOLLOW`, mode 0644 (`100755` → 0755).
6. Check the staged member: `voice_core::member::read_member`, its `slug == slug`,
   `voice_core::member::private_state_dir(&member, home) == Some(home/.claude/state/<slug>)` (a saved copy can't
   point the private folder anywhere else), `check_name` on `name` and `principal`, then
   `privacy.hits(<staging>/member)`: any hit refuses with the same wording `save_now` uses for hits (reuse
   `text::privacy_hits`'s list-of-places shape in a new `text::saved_copy_private(&full, &places)`: "The saved copy
   in <full> holds things that look private (<places>), so Voice opened nothing.").
7. Read the builder from the newest commit's trailer: `git log -1 --format=%(trailers:key=Saved-on,valueonly)
   refs/heads/main`; keep it only when `valid_computer_name`.

`place_opened`, in this order (the caller has already backed up any older assistant, see §2):
1. Refuse when `~/.claude/crew/<slug>` exists (by `symlink_metadata`): `Err(Exists(slug))`.
2. Make the private folder `~/.claude/state/<slug>` with `record::create_state` (0700) if missing. Refuse when it
   already holds `backup.git` or `save.json` (by `symlink_metadata`): `text::state_in_use(slug)`: "~/.claude/state/<slug>/
   already holds a saved copy's files, so Voice opened nothing. Forget that assistant first, then try again."
3. Move `<staging>/backup.git` to `~/.claude/state/<slug>/backup.git`, then `<staging>/member` to
   `~/.claude/crew/<slug>`, each with a no-replace rename: macOS `libc::renamex_np(from, to, libc::RENAME_EXCL)`,
   Linux `libc::renameat2(libc::AT_FDCWD, from, libc::AT_FDCWD, to, libc::RENAME_NOREPLACE)`, other unix: refuse
   if `to` exists, then `fs::rename`. `EEXIST` on the member move → `Exists(slug)`. If the member move fails, move
   `backup.git` back.
4. `write_record(state, &SaveRecord::Github { owner, name, branch: "main", role: Member, builder })`. If it fails,
   undo both moves.
5. Remove the staging folder. Return `MemberPaths::load(crew/<slug>/member.json, home)`.

### 2. `voice-api` (`builder/api/src`)

- **Reuse PB8.7b's backup.** Refactor `assistants.rs`'s `bring_in_checked` so its back-up-then-make core is one
  `pub(crate) fn back_up_older_then(home, claude_home, slug, stamp, make: &mut dyn FnMut() -> Result<Listed>) ->
  Result<(Listed, Option<PathBuf>)>` (the `existing::find`, `bring_in_refusal`, lock, `back_up`, corpus →
  `decisions-before-voice.jsonl` move, `make`, and the rollback on error exactly as today). `bring_in` keeps its
  behaviour and every existing test passes unchanged; the old-files copy stays in `bring_in_checked` around `make`.
- New `save.rs` functions (each with an `_in` form taking `home`, `claude_home`, `git`, `github`, `tokens`, as the
  file does):
  - `saved_on_github() -> Result<dto::SavedOnGithub>`: no token → `{ signed_in: false, items: [] }`; else
    `github.saved_assistants`, mapped to `dto::SavedAssistant { owner, repo, slug, name, here: bool }` where `here`
    is true when `assistants::find(home, slug)` finds one on this computer.
  - `open_saved(dto::OpenSavedRequest { owner, repo, slug }) -> Result<dto::OpenedSaved>`:
    1. `saver.stage_open(...)` with `FolderPrivacy` and `voice_agent::ledger::local_stamp(now)`.
    2. A Voice assistant already here (`assistants::find(home, slug)` is `Some`, or `~/.claude/crew/<slug>` exists)
       → remove the staging folder and fail: "You already have an assistant called <Name> on this computer, so Voice
       opened nothing. Forget that one first, then open the saved one again."
    3. `back_up_older_then(home, claude_home, slug, stamp, make)` where `make` runs `place_opened` and then
       `clear_consent` (so permission is asked again on this computer) and returns the `Listed`. A `bring_in_refusal`
       sentence is replaced with "Voice can't open <Name> here yet: ~/.claude/state/<slug>/ holds files it can't
       back up. Move that folder somewhere else, then try again."
    4. Always remove the staging folder at the end (success or failure).
    5. Return `{ assistant: dto::Assistant, backup: Option<String> (the "~/…" form bring_in_assistant_in uses),
       message }`: "Opened <Name> from <full>. Voice asks for your permission again on this computer before <Name>
       reads anything." plus, with a backup, " Voice backed up the older <Name> first, in <~/path>/."
  - Errors pass through only when they are `text::is_said` or the sentences above; anything else becomes
    "Voice couldn't open that saved assistant. Try again." (a const).
  - `save_options` gains `member_computer: Option<String>`: the `text::member_computer` sentence when the record
    is a member record, else `None`. `save_push` uses `save_now_from` with `computer::name()`.
- DTOs in `dto.rs` (camelCase, `deny_unknown_fields` on requests, `#[ts(export)]`): `SavedOnGithub { signed_in,
  items: Vec<SavedAssistant> }`, `SavedAssistant { owner, repo, slug, name, here }`, `OpenSavedRequest { owner, repo,
  slug }`, `OpenedSaved { assistant, backup: Option<String>, message }`, and the new `SaveOptions` field. Regenerate
  `builder/ui/src/types/generated/` the way the repo does (the ts-rs export runs under `cargo test`; never hand-write
  a generated file).

### 3. The app wiring (`builder/app`, `builder/ui`), no screens

Follow commit `3b2305a` (how `ask_assistant` was wired) for two new commands, `saved_on_github` and `open_saved`,
placed right after `save_push` in every list: `command_names.rs`, `commands.rs` (`#[tauri::command(async)]`),
`main.rs` `generate_handler!`, `capabilities/default.json`, `permissions/autogenerated/<name>.toml`,
`app/tests/lockdown.rs`'s expected list, `api/src/parity.rs` (both → `&[&["open"]]`), and in the UI only
`backend.ts` (`savedOnGithub(): Promise<SavedOnGithub>`, `openSaved(request: OpenSavedRequest): Promise<OpenedSaved>`),
`app/tauri-backend.ts`, its test row, and `test-support.ts`'s fake.

### 4. CLI (`builder/cli/src/main.rs`)

`voice open` (new top-level command, help text in plain words like its neighbours):
- `voice open` with no arguments lists, one per line: `<owner>/<repo>  <slug>  <Name>` (plus `  (on this computer)`
  when `here`). Not signed in → exit 1 with "Sign in to GitHub first: voice save sign-in". None found → "Voice found
  no saved assistants in your private GitHub repos."
- `voice open <owner>/<repo> <slug>` opens it as the app does and prints the message, then `backup: <path>` on its
  own line when there was one, then "Next: voice consent agree <member folder>" (match how `voice new` points at the
  next step; read it first).
- `voice save push` uses `save_now_from` with `computer::name()`.

## Tests (red first; each must fail on today's code for the right reason)

`voice-save` (`builder/save/tests/`, scratch homes only; `FakeHttp` for GitHub; real git through the fixture
wrapper): add a `Wrapper` option that prepends `-c url.<file_url>.insteadOf=https://github.com/<full>.git` to every
git call, so `fetch_main` reads a local bare repo standing in for GitHub. Never touch the network.
- record: a member record round-trips with and without `builder`; a building record's bytes are unchanged; `role`
  other than `"member"`, `builder` without `role`, a bad builder name, and an extra key are each refused.
- `saved_assistants`: the query string; private + Voice description kept; `-assistant` name kept; a public one, a
  bad owner, a repo whose `crew` is 404 or a file skipped; a bad `member.json` (slug mismatch, bad name, not base64,
  not `type: file`) skips only that entry; page 2 fetched only when page 1 held 50; 401 → `SIGN_IN_FIRST`.
- `stage_open` + `place_opened`: a stub "GitHub" holding `crew/wren/` (made by `save_now` from a member folder) opens
  into a second scratch home: files byte-equal, `private.json` never written, `save.json` is the member record with
  the builder read from `Saved-on:`; `backup.git` is bare with origin `https://github.com/<full>.git`.
- refusals leave the second home exactly as it was and no staging folder: a symlink entry, a submodule entry, a
  denied path (pick one the builtin deny list names), `..`, a missing `member.json`, a `member.json` whose
  `state_dir` points elsewhere, a privacy hit, a public repo, an existing `crew/<slug>`, a state folder already
  holding `save.json`.
- member computer: `save_now`/`save_now_from` and all three `choose_*` refuse with the member sentence before any
  HTTP or git call (assert the fake HTTP saw nothing and the wrapper log is empty).
- `save_now_from(Some("Studio"))` writes the `Saved-on: Studio` trailer; `None` and an invalid name write the old
  message.
`voice-api` (`builder/api/tests/save.rs` and `assistants.rs`): `open_saved_in` with an older assistant from before
Voice (a skill folder and a CLAUDE.md line, as PB8.7b's tests make them) backs it up and opens; the reply names the
backup; permission is not agreed afterwards; an already-here Voice assistant is refused with the sentence; a refusal
leaves no staging folder; `saved_on_github_in` without a token says not signed in; `save_options_in` on a member
record carries the sentence. Every existing `bring_in` test still passes untouched.
CLI (`builder/cli/tests/save.rs` or a new `open.rs`): `voice open --help` works; parity passes.
Mutation-check each claim (`cp` backup, `cmp`; never `git checkout --`).

## Never

- No real person's name, handle or text anywhere.
- Never write outside `~/.claude/crew/<slug>`, `~/.claude/state/<slug>` and the staging folder; never follow a link
  while writing; never write `private.json` from a saved copy.
- Never push, and never call any GitHub endpoint but the ones above. No force anything.
- No new dependency (`base64`, `libc`, `serde_json` are already there). No shell, no network in tests.
- A token never reaches argv, a file or an error message.

## Gate

`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings`,
`RUST_TEST_THREADS=2 CARGO_BUILD_JOBS=2 cargo test --locked -p voice-save -p voice-core -p voice-api -p voice-cli -p voice-app`
(use the app crate's real package name), `pnpm -C builder/ui test` and `pnpm -C builder/ui run typecheck`. Don't
commit; Claude reviews and commits.
