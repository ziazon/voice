# PB1 brief: the member format in Rust, the neutral base, one field pack

Persona Builder is a standalone app that builds a person's AI assistant from their own Claude
history. PB0 laid an empty Cargo workspace (`builder/core`, library `persona_builder`;
`builder/cli`, binary `persona-builder`). This slice teaches the core library to read and write an
assistant's files exactly as the existing Python tooling does, rewrites the two template files a
person reads into plain language, and adds the first field pack.

Read first: `AGENTS.md`, `.ai/PROJECT.md`, `CONTRIBUTING.md`, `bootstrap/install.sh` (step 3, the
Python heredoc that fills `crew/<slug>/`), `scripts/crew_decisions.py` (`validate_config`,
`load_member`, `read_member`, lines 70 to 190), `tests/crew_install_test.sh` (how it builds a
scratch fork and stubs `claude`). Skills: `~/.codex/skills/tdd-skill/SKILL.md`,
`~/.codex/skills/code-quality/SKILL.md`, `~/.codex/skills/plain-language/SKILL.md`,
`~/.codex/skills/security-and-hardening/SKILL.md`.

Work test-first: write each test, run it and see it fail on its assertion, then write the code.

## Rules

- Public repo: no real person's name, handle, path or data anywhere. Invented names only, the ones
  given below. No fixed home paths: tests build every path under a scratch folder.
- Stable Rust, edition 2024, the workspace's settings. `cargo fmt`, `cargo clippy --all-targets
  --locked -- -D warnings` and `cargo test --locked` must pass, and so must `bash
  scripts/ci-local.sh`.
- Dependencies, added to `builder/core/Cargo.toml` only: `serde = { version = "1", features =
  ["derive"] }` and `serde_json = { version = "1", features = ["preserve_order"] }` (already added,
  with `Cargo.lock` updated and the crates fetched: your sandbox has no network, so pass
  `--offline` to cargo if it tries to reach the registry). Nothing else,
  no dev-dependencies (write a small scratch-dir helper in the tests with `std::env::temp_dir()`,
  the process id and a counter, removed at the end of each test). Commit the updated `Cargo.lock`.
- Do not change `scripts/*.py`, `bootstrap/install.sh` or `bootstrap/PROMPT.md`. Do not touch
  `crew/_template/` except `IDENTITY.md` and `autonomy.md`, which you replace with the exact text
  below.
- No regex compiling in Rust in this slice (the filters are applied in PB2). No network, no child
  processes in library code (tests may run `bash`, `git` and `python3`).

## 1. `builder/core/src/error.rs`

`pub enum Error { Io { path: PathBuf, source: std::io::Error }, Invalid(String) }` with `Display`
(`Io`: `<path>: <source>`; `Invalid`: the message as is), `std::error::Error` (with `source()` for
`Io`), and `pub type Result<T> = std::result::Result<T, Error>`. Messages for a bad member config
start `member config: ` exactly as Python's do.

## 2. `builder/core/src/json.rs`: Python-identical JSON

`pub fn to_python_json(value: &impl Serialize) -> Result<String>`: the bytes Python's
`json.dumps(value, indent=2) + "\n"` writes. Serialize with `serde_json::to_string_pretty` (two
spaces, `": "` and `,` separators, `[]` and `{}` for empties, all as Python), then replace every
`char` outside `' '..='~'` that is still in the output (non-ASCII, and DEL U+007F) with `\uXXXX`
in lowercase hex, as a UTF-16 surrogate pair above U+FFFF (Python's default `ensure_ascii`). This
is safe on the whole string because such characters only occur inside JSON strings. Append `"\n"`.

## 3. `builder/core/src/member.rs`: `member.json`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub pronouns: Option<String>,
    pub principal: String,
    pub principal_slug: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub principal_pronouns: Option<String>,
    pub state_dir: String,
    pub mark_word: String,
    pub filters: Filters,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub codebook: Option<Codebook>,
    #[serde(flatten)] pub extra: serde_json::Map<String, serde_json::Value>,
}
pub struct Filters { excluded_folders: String, personal: String, secret: String,
                     pasted_prefixes: Vec<String>, #[serde(flatten)] extra: Map }
pub struct Codebook { categories, domains, escalates, signals: Vec<String>,
                      groups: serde_json::Map<String, Value>, #[serde(flatten)] extra: Map }
```

(All fields `pub`; same derives. In `Codebook`, every field is `#[serde(default, skip_serializing_if
= "…is_empty")]`.) Field order is the template's, so the template round-trips byte for byte.

- `Member::from_value(value: serde_json::Value, source: &str) -> Result<Member>`: first Python's
  checks in Python's order, with Python's messages: not an object → `member config: <source> must
  be a JSON object`; `filters` not an object → `member config: <source>: filters must be a JSON
  object`; `filters.pasted_prefixes` not a list of strings → `member config: <source>:
  filters.pasted_prefixes must be a list of strings`; `queue_answerer` present and not a non-empty
  string → `member config: <source>: queue_answerer must be a non-empty string`. Then the required
  keys `name, slug, principal, principal_slug, state_dir, mark_word, filters` and
  `filters.excluded_folders, filters.personal, filters.secret, filters.pasted_prefixes`, all missing
  ones listed in that order in one message: `member config: missing name, filters.secret`. Then
  deserialize (a wrong type there → `member config: <source>: <serde message>`).
- `pub fn read_member(path: &Path) -> Result<Member>`: Python's `read_member`. Read `member.json`
  (a parse error → `member config: <path> is not JSON (<serde message>)`), run the object checks,
  and when `private.json` beside it is a file, lay it over: it gets the same object checks with
  source `<its path>`; every key but `filters` replaces the member's; for each `filters` key in
  order: `pasted_prefixes` is appended to the member's list; any other key must be a string
  (`member config: private.json: filters.<key> must be a string (a regex)`), and becomes
  `(?:<member's>)|(?:<private's>)` when the member's value is a non-empty string, else the
  private value. Then the required-keys check and deserialize. Refuse (`Invalid`) a `member.json`
  or `private.json` that is a symlink (`symlink_metadata`), naming the path.
- `pub fn write_member(path: &Path, member: &Member) -> Result<()>`: `to_python_json`, written in
  place.
- `impl Member { pub fn pick_field(&self) -> String /* "<slug>_pick" */; pub fn answer_field(&self)
  -> String /* "<principal_slug>_answer" */; pub fn principal_subject(&self) -> &str /* the part of
  principal_pronouns before "/", "they" when absent */ }`.

## 4. `builder/core/src/crew.rs`: the `crew/<slug>/` folder

- `pub fn slugify(text: &str) -> Option<String>`: what install.sh's `tr | tr -cs | sed` does:
  ASCII-lowercase, each run of bytes outside `a-z0-9` becomes one `-` (a non-ASCII letter is such a
  byte), then one leading and one trailing `-` dropped; `None` when empty.
- `pub fn shown_path(repo: &Path, home: &Path) -> String`: both canonicalized; `~/<relative>`
  when the repo is under home, else the repo's absolute path (install.sh's `shown`).
- `pub struct NewMember<'a> { name, pronouns, principal, principal_pronouns, repo_shown, date: &'a
  str }` and `pub fn new_member(template: &Path, target: &Path, new: &NewMember) -> Result<Member>`,
  install.sh step 3 without a kit: refuse when `target` exists (`Invalid("<target> exists")`); slugs
  from `slugify` (`--name must contain a letter or digit` / `--principal must contain a letter or
  digit` style messages, without the `--`); copy the template tree recursively, refusing any
  symlink in it, skipping the top-level `README.md`; in every copied file but `member.json`, replace
  the placeholders in this order, each with `str::replace` over the whole text: `NAME, SLUG,
  PRONOUNS, PRINCIPAL, PRINCIPAL_SLUG, PRINCIPAL_PRONOUNS, REPO, DATE` (as `{{KEY}}`); for
  `member.json`, parse the template's, fill every string value and every object key the same way
  (recursively, lists included), and write it with `to_python_json`. Return the filled `Member`.
  Files are written with the default mode; folders are created with `create_dir_all`.
- `pub struct MemberDir { pub dir: PathBuf, pub member: Member, pub has_lens: bool }` and
  `MemberDir::open(dir: &Path) -> Result<MemberDir>`: `read_member(dir/member.json)`, then require
  `IDENTITY.md, SKILL.md, autonomy.md, decisions.md` (`Invalid("<dir>: missing <file>")`, the first
  missing one), `has_lens` from `lens.md`. `pub fn autonomy(&self) -> Result<Autonomy>` reads and
  parses `autonomy.md`.

## 5. `builder/core/src/autonomy.rs`: the dial in `autonomy.md`

`pub struct Autonomy { lines: Vec<Line> }` keeping every line of the file; a dial line is one
matching `- [x] `key`: text` or `- [ ] `key`: text` exactly (key `[a-z0-9-]+`). `pub struct Dial {
pub key: String, pub principal_decides: bool, pub text: String }`. API: `Autonomy::parse(&str) ->
Result<Autonomy>` (a repeated key → `Invalid("autonomy.md: <key> appears twice")`),
`dials(&self) -> Vec<Dial>` in file order, `set(&mut self, key, principal_decides: bool) ->
Result<()>` (unknown key → `Invalid`), `render(&self) -> String`. `render(parse(s)) == s` byte for
byte, trailing newline included.

## 6. `builder/core/src/fields.rs` and `crew/_fields/general.md`: field packs

A field pack is a Markdown file `crew/_fields/<id>.md` in exactly this shape:

```markdown
---
id: <id>
name: <one line>
summary: <one line>
---

# <title>

<paragraphs, ignored by the loader>

## Typical decisions

- `<codebook category>`: <one line>

## Words

- <one word or short phrase>
```

`pub struct FieldPack { pub id, pub name, pub summary: String, pub decisions: Vec<(String,
String)>, pub words: Vec<String> }`. `FieldPack::parse(text: &str, categories: &[String]) ->
Result<FieldPack>`: the front matter must be the first lines, with exactly the keys `id`, `name`,
`summary` (each once, non-empty; `id` matches `[a-z0-9-]+`); under `## Typical decisions` each
`- `cat`: text` line, `cat` in `categories` and not repeated; under `## Words` each `- text` line;
both lists non-empty; any other `## ` heading ends the section it follows. Errors name the field
pack and the problem (`field pack general: unknown category "x"`). `pub fn load(dir: &Path, id:
&str, categories: &[String]) -> Result<FieldPack>` reads `<dir>/<id>.md` (refuse a symlink; the
parsed `id` must equal the file's), and `pub fn list(dir: &Path) -> Result<Vec<String>>` the ids of
the `*.md` files there, sorted.

Write `crew/_fields/general.md` with exactly this text:

```markdown
---
id: general
name: Any line of work
summary: The decisions most people make every week, whatever their work
---

# Any line of work

A starting point for an assistant whose person has little history yet. These are the kinds of
decisions most people make every week, so the first build has somewhere sensible to begin. The
person's own answers always win over anything here.

## Typical decisions

- `scope`: what to include and what to leave out, and how big a first version should be
- `sequencing`: what to do first, what can wait, and when to start
- `methods`: how to do the work: which approach and which steps
- `framing`: how to present something: what to lead with and which angle to take
- `writing-voice`: wording and tone in their own drafts
- `communication`: who to tell, when, and how much detail to give
- `collaboration`: who to involve, who owns what, and when to ask for help
- `compliance-ethics`: the rules, policies, consent and fairness the work must respect
- `tooling`: which app, tool or template to use
- `verification`: how to check the work is right before it goes out
- `placement-naming`: where a file or a piece of work belongs, and what to call it
- `go-no-go`: whether something is ready, and whether to go ahead at all

## Words

- deadline
- draft
- feedback
- priority
- meeting
- follow-up
- schedule
- sign-off
- handover
- next step
```

## 7. `crew/_template/IDENTITY.md` and `autonomy.md`, rewritten

Replace each file with exactly this text (keep `{{NAME}} ({{PRONOUNS}})`: the install test checks
it; keep each dial key and `[x]`/`[ ]` state: the skill reads them).

`crew/_template/IDENTITY.md`:

```markdown
# {{NAME}}

{{NAME}} ({{PRONOUNS}}) is {{PRINCIPAL}}'s assistant for decisions. When a question comes up in
{{PRINCIPAL}}'s work, {{NAME}} answers it the way {{PRINCIPAL}} would, says how sure that answer
is, and hands the question back whenever it is {{PRINCIPAL}}'s to decide.

{{NAME}} learned this from {{PRINCIPAL}}'s own answers and messages to Claude, starting {{DATE}}.

## How {{NAME}} works

- **Says how sure the answer is.** Every answer comes with how confident {{NAME}} is and how often
  {{NAME}} has been right on questions like it. When {{NAME}} is not sure, {{NAME}} says so plainly.
- **Hands back what is {{PRINCIPAL}}'s.** Some decisions always stay with {{PRINCIPAL}}; the list
  is in [autonomy.md](autonomy.md). On those, {{NAME}} says what {{PRINCIPAL}} would likely pick and
  why, then leaves the choice to {{PRINCIPAL}}.
- **Talks like a colleague.** Plain, warm words, in {{PRINCIPAL}}'s own way of speaking and the
  words of {{PRINCIPAL}}'s field. A teacher's assistant sounds like someone who works in a school,
  not like an engineer.
- **Grows with {{PRINCIPAL}}.** Every answer and every correction {{PRINCIPAL}} gives teaches
  {{NAME}} something. A change to what {{NAME}} knows is shown to {{PRINCIPAL}} first, and kept only
  once {{PRINCIPAL}} approves it.

## What {{NAME}} never does

- Send, post, submit, sign or agree to anything **as {{PRINCIPAL}}**. {{NAME}} drafts and
  suggests; {{PRINCIPAL}} sends.
- Decide something [autonomy.md](autonomy.md) keeps for {{PRINCIPAL}}.
- Look into, learn from or write about {{PRINCIPAL}}'s private life (health, family, legal matters,
  household money), anyone else's private conversations, or details that could identify another
  person.
- Pretend to be sure.

## Where {{NAME}} lives

What {{NAME}} has learned is kept in the files beside this one, in {{PRINCIPAL}}'s own words
rewritten, never quoted. The history {{NAME}} learned from stays in a private folder on
{{PRINCIPAL}}'s computer. It is never copied into these files or into a backup.
```

`crew/_template/autonomy.md`:

```markdown
# What {{NAME}} may decide

Each line below is one kind of decision. **`[x]` means {{PRINCIPAL}} decides it.** {{NAME}} still
says what {{PRINCIPAL}} would likely pick and why, so the choice is quick. **`[ ]` means {{NAME}}
may decide it alone**, but only when {{NAME}} is very sure and has a good record on that kind of
question.

{{PRINCIPAL}} sets these lines while building {{NAME}}, and can change any of them later by editing
this file. Nobody else changes them.

Set by {{PRINCIPAL}} on {{DATE}}.

## {{PRINCIPAL}}'s actions (best kept `[x]`)

- [x] `send-external`: any email or message to another person: colleagues, clients, partners, anyone
- [x] `submit`: sending in or finalising an application, a document, a manuscript or a form
- [x] `commitment`: agreeing to a deadline, a role, a review, a collaboration or a request for help

## Risk (best kept `[x]`)

- [x] `irreversible`: deleting files or history, overwriting a shared document, anything that can't be undone
- [x] `money`: budgets, spending, payments, rates
- [x] `privacy`: other people's information, logins and keys, what a file or export reveals

## Judgment

- [x] `reverses-ruling`: the answer would go against a rule {{PRINCIPAL}} has set, or two of those rules disagree
- [x] `no-precedent`: a kind of question {{PRINCIPAL}} has not answered before
- [x] `below-high`: {{NAME}} is not very sure, or has not yet shown a good record on this kind of question
- [ ] `scope`: how much to include, which items of a list, what to cut
- [ ] `sequencing`: what to do first, now or later
- [ ] `writing-voice`: wording, tone and structure in {{PRINCIPAL}}'s own drafts
- [ ] `tooling`: which app, tool or file layout to use for {{PRINCIPAL}}'s own work

The four open lines are a suggested start; {{PRINCIPAL}} may close any of them, or open others.
```

## 8. The CLI: `persona-builder member show <member.json>`

A clap subcommand group `member` with one command `show <PATH>`: `read_member`, print
`to_python_json` of the result on stdout, exit 0; on error print `persona-builder: <error>` on
stderr and exit 1. `--version` keeps working.

## 9. Tests (all red first)

Unit tests beside each module, plus integration tests in `builder/core/tests/` and
`builder/cli/tests/`:

1. **JSON:** `to_python_json` of `{"a": [], "b": {}, "c": "Zoë 😀\u{7f}"}` equals the exact string
   Python prints (`"c": "Zoë 😀\u007f"`, empties as `[]` and `{}`).
2. **Template round trip:** `crew/_template/member.json` read with `read_member` and written with
   `write_member` is byte-identical to the file.
3. **Validation:** each message in section 3 (missing keys listed together; each type check), the
   private.json merge (a joined regex, an empty member value replaced, `pasted_prefixes` appended,
   another key replaced, a non-string filter refused), and a symlinked `member.json` refused.
4. **Parity with Python's `read_member`** (`builder/core/tests/python_parity.rs`): for three
   invented member folders (one plain, one with a `private.json` that extends `personal` and adds
   `pasted_prefixes` and a `codebook`, one with `queue_answerer`), run `python3 -I -c` loading
   `scripts/crew_decisions.py` by path (`importlib.util`) and printing
   `json.dumps(read_member(p), indent=2)`; Rust's `to_python_json(read_member(p))` must equal it
   byte for byte. Also two invalid configs: both sides fail, with the same message (Python's
   `Failed` text on stderr).
5. **Parity with install.sh** (`builder/core/tests/install_parity.rs`): build a scratch fork the
   way `tests/crew_install_test.sh` does (copy `scripts`, `bootstrap`, `crew/_template`,
   `.gitignore`; `git init -b main`, commit with `-c user.name=t -c user.email=t@t`) at
   `<scratch>/home/projects/voice`, a stub `claude` on `PATH`, `HOME=<scratch>/home`, then run
   `bash bootstrap/install.sh --name <n> --principal <p> [--pronouns …] [--principal-pronouns …]
   --no-history --repo-dir <fork>` for these invented pairs: `Ari`/`Robin`; `Mo Lee` (she/her) /
   `Sam O'Neil` (he/him); `Zoë` / `Kit` (to cover non-ASCII). Run `new_member` for the same values
   into `<scratch>/rust/<slug>` with `repo_shown = "~/projects/voice"` and `date` from `date +%F`
   taken before install.sh runs (if `date +%F` differs after it, run that case again once). The two
   folders must hold the same files with the same bytes.
6. **Parity skip:** tests 4 and 5 print `skipped: python3, git or bash not found` and return when
   any is missing from `PATH`; they must run in `ci-local.sh`.
7. **Autonomy:** the template's `autonomy.md` parses to 13 dials (9 `[x]`, 4 `[ ]`), round-trips
   byte for byte, `set("scope", true)` changes only that line, an unknown key and a repeated key
   are refused.
8. **Field packs:** `crew/_fields/general.md` loads against the template codebook's categories (12
   decisions, 10 words); an unknown category, a repeated category, a missing front-matter key, an
   `id` that differs from the file name and an empty `## Words` list are each refused; `list` on
   `crew/_fields` is `["general"]`.
9. **`MemberDir::open`** on a `new_member` output works (`has_lens` false) and names the first
   missing required file when one is deleted.
10. **CLI** (`builder/cli/tests/member_show.rs`): `member show` on a `new_member` output prints
    exactly the file's bytes; on a missing path it exits 1 with `persona-builder: ` on stderr.

Run a mutation check for each of: the `ensure_ascii` escaping, the private-filter join, the
placeholder order (swap `PRINCIPAL` and `PRINCIPAL_SLUG`), and the skipped `README.md`. Back up the
file with `cp`, break it, see the named test go red, restore it with `cp` and confirm with `cmp`.
Never use `git checkout --` or `git stash`.

## 10. Docs

- `.ai/PROJECT.md` table: change the `builder/core/` row's "what" to `The Persona Builder core
  library (persona_builder): the member format, the dial, field packs` and its tests to
  `builder/core/src/`, `builder/core/tests/`; add a row `crew/_fields/` | `Field packs: starter
  decisions and words per kind of work` | `builder/core/tests/`.
- `README.md` folder table: after the `builder/` row add `| \`crew/_fields/\` | Field packs: a short
  starter set of decisions and words for a kind of work, used when someone's history is thin |`.
- `crew/_template/README.md`: no change.

## Done when

`bash scripts/ci-local.sh` passes with all Rust steps run; every test above went red first and is
green; the four mutations each turned their test red and were restored (`cmp` clean). Report: the
files changed, each red you saw (test name and failing assertion), each mutation's red, and the
final `ci-local.sh` tail. Do not commit; leave the changes in the worktree.
