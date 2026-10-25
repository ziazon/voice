# PB7.1 brief: the app's frame and its locked-down bridge

Written by Claude (lane V1), 2026-10-25. Spec: lane V's `PB7.1` entry, split from PB7 of the
Persona Builder plan (§ Shape, § Phases PB7, PB-D12). Read the repo's `AGENTS.md` and
`.ai/PROJECT.md` first, then `builder/cli/src/main.rs` (`agents()` and the `--version` flag) and
`builder/agent/src/agents.rs`. Work test-first (`~/.codex/skills/tdd-skill/SKILL.md`); security
rules: `~/.codex/skills/security-and-hardening/SKILL.md`; structure: `~/.codex/skills/frontend/SKILL.md`
and `~/.codex/skills/tabler-ui/SKILL.md`; looks: `~/.codex/skills/design-guide/SKILL.md`; every
sentence a person sees: `~/.codex/skills/plain-language/SKILL.md`; review bar:
`~/.codex/skills/code-quality/SKILL.md`.

The stack copies the Flightdeck desktop app, another open-source Tauri and Vue app, called *the
reference app* below: its `desktop/src-tauri/` and `desktop/ui/` (read-only, in the checkout Claude
names when dispatching) and its root `.cargo/config.toml`. Mirror its shapes; never copy a part only
it needs (e2e plugins, the PTY, its stores), and never name it in this repo's code or in anything a
person reads.

Run cargo with `CARGO_NET_OFFLINE=true`: every crate below is already in the local registry cache.
Run pnpm with `--offline`. If either says a package is missing, stop and name it; Claude fetches it.

## What this slice does

It starts the Persona Builder app: a window with the steps a person goes through, one Welcome screen
that shows the app's version, and the bridge between the screens and the Rust code, locked down so
the screens can call only the commands we list. Every later screen (PB7.2 to PB7.6) adds itself to
this frame and its commands to this bridge.

## Decisions (made, do not reopen)

### Three new parts

1. **`builder/api/`**, package `persona-builder-api`, library `persona_builder_api`. The app's
   commands as plain Rust functions over the existing crates. **No Tauri dependency**, so the Tauri
   app and, later, another app's pane call the same code. Dependencies: `persona-builder-agent`
   (path), `serde` (derive) and `serde_json` as the other crates pin them, and
   `ts-rs = { version = "=12.0.1", features = ["no-serde-warnings"] }` (as the reference app's
   `desktop/core`).
2. **`builder/app/`**, package `persona-builder-app`, binary `persona-builder-app`. The Tauri 2 shell:
   `tauri = { version = "=2.12.0", features = [] }`, build-dependency
   `tauri-build = { version = "=2.7.0", features = [] }`, plus `persona-builder-api` (path),
   `serde_json`. No `tauri-plugin-*` crate, now or in this slice's tests.
3. **`builder/ui/`**, the pnpm package `@persona-builder/screens` (`"private": true`,
   `"version": "0.1.0"`, `"type": "module"`). Exact versions, as the reference app's `desktop/ui` pins them:
   dependencies `vue` `3.5.43`, `@tabler/core` `1.6.1`, `@tauri-apps/api` `2.12.0`; devDependencies
   `@tauri-apps/cli` `2.12.0`, `@vitejs/plugin-vue` `6.0.9`, `@vue/test-utils` `2.5.1`, `happy-dom`
   `20.14.5`, `typescript` `5.9.3`, `vite` `8.3.1`, `vitest` `5.0.2`, `vue-tsc` `3.3.11`. Nothing else
   (no icon package until a screen uses one). No `packageManager` field. `pnpm-workspace.yaml` as
   the reference app's (`allowBuilds: esbuild: false`). Commit `pnpm-lock.yaml`.

Add `builder/api` and `builder/app` to the root `Cargo.toml`'s `members`. Root `.gitignore` gains
`builder/ui/node_modules/`, `builder/ui/dist/` and `builder/app/gen/`.

### The api crate

- `src/lib.rs`: `pub mod dto; pub mod parity;` and the commands as functions.
- `src/dto.rs`: every command's answer (and, from PB7.2, its request) is a DTO with
  `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]`, `#[serde(rename_all =
  "camelCase")]`, `#[ts(export)]`. **Requests** (none yet) will also carry
  `#[serde(deny_unknown_fields)]`; write that rule as a doc comment at the top of `dto.rs`.
- This slice's one command: `pub fn app_info() -> AppInfo`, where
  `AppInfo { version: String, agents: Vec<AgentInfo> }` and `AgentInfo { id: String, name: String }`.
  `version` is `env!("CARGO_PKG_VERSION")` (the workspace version, the same one
  `persona-builder --version` prints); `agents` is `persona_builder_agent::agents::AGENTS` in order.
- **Generated types.** A new root `.cargo/config.toml` with
  `[env] TS_RS_EXPORT_DIR = { value = "builder/ui/src/types/generated", relative = true }` and a
  comment saying why it is at the root (Cargo reads it from above its working directory). `cargo test`
  writes `AppInfo.ts` and `AgentInfo.ts` there; commit them.
- `src/parity.rs`, the CLI-parity table (the plan: the CLI does everything the app does):

  ```rust
  /// Each app command and the `persona-builder` commands that do the same thing.
  pub struct Parity { pub app: &'static str, pub cli: &'static [&'static [&'static str]] }
  pub const PARITY: &[Parity] = &[Parity { app: "app_info", cli: &[&["--version"], &["agents"]] }];
  ```

  A test in `builder/cli/tests/app_parity.rs` (the cli crate gains `persona-builder-api` as a
  dev-dependency) runs, for each argv in each row, the built `persona-builder` with that argv and
  `--help` appended, and requires exit 0; a row with an empty `cli` fails the test.

### The Tauri crate

- `src/commands.rs`: `pub const COMMANDS: &[&str] = &["app_info"];` and
  `#[tauri::command] pub fn app_info() -> persona_builder_api::dto::AppInfo { persona_builder_api::app_info() }`.
  Every command is exactly one call into `persona_builder_api`; no logic here.
- `build.rs`: `tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
  tauri_build::AppManifest::new().commands(&["app_info"])))`. Commit the generated
  `permissions/autogenerated/` files as the reference app does; `gen/` stays ignored.
- `capabilities/default.json`: identifier `default`, window `main`, and **only** `allow-app-info`.
  No `core:default` and no other `core:` permission: nothing in this slice needs one, and PB7.4 adds
  the event permissions when progress events arrive.
- `src/navigation.rs`: the reference app's `is_allowed_navigation` and its three tests, with the dev origin
  `http://127.0.0.1:5180` (5173 is the reference app's; the two must not collide). `src/main.rs` installs it
  as a `nav-guard` plugin built with `tauri::plugin::Builder` exactly as the reference app's `main.rs` does,
  then `.invoke_handler(tauri::generate_handler![commands::app_info])` and
  `.run(tauri::generate_context!())`. On error print `Persona Builder couldn't start: <error>` to
  stderr and exit 1 (no panic, no `expect`).
- `tauri.conf.json`: `productName` `Persona Builder`, `version` `0.1.0`, `identifier`
  `dev.personabuilder.desktop` (a neutral placeholder until signing, PB-D3: the app keeps no data
  under it, so changing it later costs nothing; say so in a `README.md` line in `builder/app/`).
  `build`: `frontendDist` `../ui/dist`, `devUrl` `http://127.0.0.1:5180`, `beforeDevCommand`
  `{ "script": "pnpm dev", "cwd": "../ui" }`, `beforeBuildCommand` `{ "script": "pnpm build", "cwd":
  "../ui" }`. `app.withGlobalTauri` `false`; one window, label `main`, title `Persona Builder`, width
  1100, height 760, minWidth 860, minHeight 600, `dragDropEnabled` `false` (a dropped file would hand
  the webview a path). `app.security`: the reference app's `csp` and `devCsp` with its port changed to 5180,
  and its `dangerousDisableAssetCspModification: ["style-src"]`. No `plugins` key. `bundle`: `active`
  `true`, `targets` `["app"]`, the icon list below. No macOS entitlements file (nothing needs one).
- **Icons.** A placeholder only (the real icon is PB8's): `builder/app/icons/icon.svg`, a 1024×1024
  rounded square in a calm blue (`#2f6fdf`) with a simple white speech-bubble shape, no letters, no
  person. Generate the set with `pnpm -C builder/ui tauri icon ../app/icons/icon.svg` and keep only
  `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.png` (list those five in
  `bundle.icon`; delete the rest it writes).
- `tests/lockdown.rs` (integration test reading the crate's own files with
  `env!("CARGO_MANIFEST_DIR")`, no fixed path):
  1. `COMMANDS`, the capability's `allow-…` entries (each `allow-` plus the command with `_` as `-`),
     the `commands(&[…])` list in `build.rs`, the `generate_handler![…]` list in `src/main.rs` and
     `persona_builder_api::parity::PARITY`'s `app` names are the same set, in the same order. Parse
     the two `.rs` lists by text: the bracketed list after `commands(&[` and after
     `generate_handler![`, entries stripped of `commands::`, quotes and spaces.
  2. The capability's `permissions` hold nothing but those `allow-…` entries.
  3. `tauri.conf.json`: `withGlobalTauri` is `false`, every window has `dragDropEnabled: false`,
     there is no `plugins` key, and every source in the `csp` is one of `'self'`, `'none'`,
     `'unsafe-inline'` (in `style-src` only), `ipc:`, `http://ipc.localhost` and `data:` (in
     `img-src` only): no other scheme, host or origin.
  4. `Cargo.toml` has no dependency whose name starts with `tauri-plugin`.

### The screens package

```
builder/ui/
  index.html            # title "Persona Builder", mounts src/app/main.ts
  env.d.ts              # the .vue module shim
  package.json  pnpm-lock.yaml  pnpm-workspace.yaml  tsconfig.json  vite.config.ts
  src/
    index.ts            # the package's exports (below)
    backend.ts          # the Backend interface, provideBackend, useBackend
    steps.ts            # the step list
    PersonaBuilder.vue  # the frame
    screens/Welcome.vue
    types/generated/    # written by cargo test, never by hand
    app/
      main.ts           # the standalone app: Tabler CSS, theme, the Tauri backend, mount
      tauri-backend.ts  # the ONLY file that imports @tauri-apps/api
      theme.ts          # follow the system's light or dark
    test/setup.ts       # if vitest needs one
```

- `package.json` scripts, as the reference app's: `dev` `vite`, `build` `vite build`, `typecheck`
  `vue-tsc --noEmit`, `test` `vitest run`, and `tauri` `cd ../app && tauri`. `"exports": { ".":
  "./src/index.ts" }`: the package ships its source, so a host app's Vite compiles it.
- `tsconfig.json`: the reference app's (strict, `noUncheckedIndexedAccess`, bundler resolution).
- `vite.config.ts`: the reference app's shape without its build stamp, worker and timeout helpers:
  `plugins: [vue()]`, `clearScreen: false`, `envDir: false` (with its comment), `server: { host:
  '127.0.0.1', port: 5180, strictPort: true }`, `build: { target: 'safari16', cssTarget: 'safari18' }`
  with the reference app's comment on why, and `test: { environment: 'happy-dom' }`.
- `backend.ts`:

  ```ts
  import type { InjectionKey } from 'vue'
  import type { AppInfo } from './types/generated/AppInfo'
  /** Everything the screens ask of the app. A host gives its own (the standalone app's is tauri-backend.ts). */
  export interface Backend { appInfo(): Promise<AppInfo> }
  export const backendKey: InjectionKey<Backend> = Symbol('persona-builder-backend')
  export function provideBackend(backend: Backend): void  // provide(backendKey, backend)
  export function useBackend(): Backend                    // inject; throws "Persona Builder has no backend" when missing
  ```

  One method per api command, named in camelCase, typed only with generated types (`import type`).
- `index.ts` exports `PersonaBuilder`, `Welcome`, `steps`, `provideBackend`, `useBackend`,
  `backendKey`, and `type Backend`. Nothing from `src/app/`.
- `steps.ts`: `export const steps` in this order, each `{ id, title }`, titles exactly:
  `consent` "Your permission", `connect` "Connect Claude", `sources` "Your history", `build` "Build",
  `report` "How well it knows you", `review` "Review", `save` "Save". No step has a screen yet.
- `PersonaBuilder.vue` (the frame): a Tabler page with a narrow side column listing the steps as an
  ordered list (number and title), each marked *Not here yet* in muted text with
  `aria-disabled="true"`, and the main column showing `Welcome`. Use Tabler classes only, no custom
  colors (light and dark both come from Tabler's theme variables), no Tabler JavaScript.
- `screens/Welcome.vue`: on mount it calls `useBackend().appInfo()`. Three states:
  - loading: a Tabler spinner with the visually-hidden text "Starting…";
  - error: an alert, "Persona Builder couldn't start properly. Close it and open it again." (never the
    error's own text);
  - loaded: the heading "Persona Builder", the line "Make an AI assistant that decides the way you do,
    from your own Claude history.", the line "Your history stays on this computer. Nothing is saved
    anywhere else unless you choose it.", and a muted "Version <version>".
- `app/tauri-backend.ts`: `export const tauriBackend: Backend = { appInfo: () => invoke<AppInfo>('app_info') }`
  with `invoke` from `@tauri-apps/api/core`.
- `app/theme.ts`: `export function followSystemTheme(win: Window = window): () => void`: sets
  `document.documentElement.dataset.bsTheme` to `dark` or `light` from
  `matchMedia('(prefers-color-scheme: dark)')`, updates on its `change` event, and returns the
  function that stops listening.
- `app/main.ts`: imports `@tabler/core/dist/css/tabler.min.css` (CSS only, as the reference app: Tabler's
  JavaScript is never loaded), calls `followSystemTheme()`, then
  `createApp(PersonaBuilder)` with `app.provide(backendKey, tauriBackend)`, mounted on `#app`.

### The rule every later screen keeps (write it in `builder/ui/README.md` and as a doc comment on `Backend`)

The webview never names a file or folder. A request carries an assistant's slug or an id the Rust
side handed out; Rust finds every path itself and opens every native picker itself. A host app must
keep the same rule.

### ci-local.sh

After the Rust block (inside `if command -v cargo`, after `cargo test`), add:

```bash
  echo 'git diff --exit-code -- builder/ui/src/types/generated'
  git diff --exit-code -- builder/ui/src/types/generated
  if [ -n "$(git status --porcelain -- builder/ui/src/types/generated)" ]; then
    echo 'ci-local: builder/ui/src/types/generated has files cargo test wrote that are not committed'
    exit 1
  fi
```

Then, outside the cargo block, before `ci-local: all passed`:

```bash
if command -v pnpm >/dev/null 2>&1; then
  for step in 'install --frozen-lockfile' typecheck test build; do
    echo "pnpm -C builder/ui $step"
    # shellcheck disable=SC2086
    pnpm -C builder/ui $step
  done
else
  echo 'ci-local: pnpm not found, skipping the app screens (install Node and pnpm: https://pnpm.io/installation)'
fi
```

`shellcheck scripts/ci-local.sh` must stay clean.

### Docs

- `.ai/PROJECT.md`: three table rows, in the table's style: `builder/api/` (the app's commands, no
  Tauri, and the CLI-parity table; tested by `builder/api/tests/`, `builder/cli/tests/app_parity.rs`),
  `builder/app/` (the Persona Builder app's window and its locked-down bridge;
  `builder/app/tests/lockdown.rs`), `builder/ui/` (the app's screens as a package another app can
  mount; `pnpm -C builder/ui test`). Update the "All of it runs with" paragraph: Node and pnpm for the
  screens, skipped with a printed line when missing.
- `AGENTS.md`: its "(Python 3.9+ and shellcheck)" becomes "(Python 3.9+ and shellcheck; stable Rust,
  Node and pnpm for the rest, each skipped when missing)".
- `builder/ui/README.md`: what the package is, how to run it (`pnpm -C builder/ui tauri dev`), the
  webview rule above, and that a host mounts `PersonaBuilder` after `provideBackend`.

## Tests (write each first and watch it fail on its assertion)

Rust:
- `builder/api/tests/app_info.rs`: `app_info()` gives the workspace version and the agent list in
  order (compare with `persona_builder_agent::agents::AGENTS`); `AppInfo` serializes with camelCase
  keys (`{"version":…,"agents":[{"id":…,"name":…}]}`).
- `builder/cli/tests/app_parity.rs` as above.
- `builder/app/tests/lockdown.rs` as above; plus `navigation.rs`'s unit tests.

Vitest (`builder/ui/src/**/*.test.ts`, happy-dom):
- `Welcome.test.ts`: with a fake backend given by `provide`, the loaded state shows the version and
  both lines; a backend that never resolves shows "Starting…"; one that rejects shows the error
  sentence and not the rejection's text.
- `PersonaBuilder.test.ts`: the seven step titles in order, each marked *Not here yet* and
  `aria-disabled`; the Welcome screen is shown.
- `backend.test.ts`: `useBackend` outside a provider throws the plain message.
- `theme.test.ts`: dark and light from a fake `matchMedia`, a `change` event flips it, the returned
  function stops it.
- `tauri-backend.test.ts`: with `vi.mock('@tauri-apps/api/core')`, `appInfo()` calls
  `invoke('app_info')` and returns its answer.
- `imports.test.ts`: walking `src/` with `node:fs`, no `.ts` or `.vue` file but
  `src/app/tauri-backend.ts` contains `@tauri-apps/`; and `src/index.ts` imports nothing from
  `./app/`.

Mutation checks (each: `cp` a backup, make the edit, run the test, see it red, restore with `cp`
and confirm with `cmp`; never `git checkout --`). List each with its red result:
1. add `"core:default"` to the capability → lockdown 2 red;
2. add a second command to `COMMANDS` only → lockdown 1 red;
3. set `withGlobalTauri` to `true` → lockdown 3 red;
4. drop `dragDropEnabled: false` → lockdown 3 red;
5. allow `http://127.0.0.1:5173` in `is_allowed_navigation` → its test red;
6. make the Welcome error show the rejection text → Welcome test red;
7. import `@tauri-apps/api/core` in `Welcome.vue` → imports test red;
8. empty `PARITY[0].cli` → app_parity red.

## Done when

`CARGO_NET_OFFLINE=true bash scripts/ci-local.sh` is green (if a socket test in `builder/save`
cannot bind in your sandbox, say so and confirm everything else passed), `pnpm -C builder/ui build`
writes `dist/`, and the mutation list is reported. Don't commit; Claude reviews, commits and runs the
app.

## Fix-up pass (decided, do not reopen)

Claude's review of the first build:

1. **One command list, no `dead_code` allowance.** Move `COMMANDS` out of `src/commands.rs` into a
   new `builder/app/src/command_names.rs` holding only
   `pub const COMMANDS: &[&str] = &["app_info"];` (with a one-line doc comment saying the app manifest,
   the capability, the handler list and the parity table must name exactly these, and
   `tests/lockdown.rs` checks it). `build.rs` does `include!("src/command_names.rs");` and passes
   `.commands(COMMANDS)`. `tests/lockdown.rs` does `include!("../src/command_names.rs");` (drop its
   `#[path]` include of `commands.rs`, the `build.rs` text parse, and the
   `commands::app_info() == app_info()` line: `commands.rs` is one call, and the api test covers the
   answer). Remove `#[allow(dead_code)]` and the old const from `commands.rs`. Keep the
   `generate_handler!` text check, the capability check and the `PARITY` check.
2. **`serde_json` is a dev-dependency of `builder/app`**, not a dependency: only the tests use it.

Mutation-check 1 again (add a command to `COMMANDS` only → lockdown 1 red) with `cp`/`cmp`. Run
`cargo fmt --check`, `cargo clippy -p persona-builder-app --all-targets --locked -- -D warnings` and
`cargo test -p persona-builder-app --locked`. Don't commit.

## Fix-up pass 2: from the security review (decided, do not reopen)

1. **The dev origin follows Tauri's own dev switch.** In `builder/app/src/main.rs`, pass
   `tauri::is_dev()` to `is_allowed_navigation` instead of `cfg!(debug_assertions)`: `devUrl` and
   `devCsp` are chosen by `tauri::is_dev()` (true without the `custom-protocol` feature), so a
   `tauri build --debug` bundle must not allow `http://127.0.0.1:5180`. Add a line to
   `tests/lockdown.rs` that `src/main.rs` contains `is_allowed_navigation(url, tauri::is_dev())` and
   not `debug_assertions`; mutation-check it.
2. **No pnpm that runs dependencies' install scripts.** pnpm 10 and later don't run a dependency's
   install scripts unless allowed; older pnpm does. In `scripts/ci-local.sh`, inside the
   `command -v pnpm` branch and before the loop, read `pnpm --version`'s major number; below 10,
   print `ci-local: pnpm <version> is older than 10, skipping the app screens (update pnpm:
   https://pnpm.io/installation)` and skip the pnpm steps (not a failure). `builder/ui/README.md`
   says pnpm 10 or later is needed. Keep `shellcheck scripts/ci-local.sh` clean, and check the skip
   by running the branch with a stub `pnpm` on `PATH` that prints `9.15.0` (in a temp folder,
   removed afterwards).

Run `cargo fmt --check`, `cargo clippy -p persona-builder-app --all-targets --locked -- -D warnings`,
`cargo test -p persona-builder-app --locked` and `shellcheck scripts/ci-local.sh`. Don't commit.

## Fix-up pass 3: from the Verifier (decided, do not reopen)

1. **A renamed plugin is caught.** In `builder/app/tests/lockdown.rs`, lockdown 4 also fails on any
   `Cargo.toml` line whose value names a `tauri-plugin` package (`foo = { package =
   "tauri-plugin-x", … }`): check the whole line for `tauri-plugin`, not only the key. Add the
   renamed form to the `plugin_keys_include_both_toml_quote_styles` cases (or a sibling test).
2. **`builder/api` stays free of Tauri, by test.** A new `builder/api/tests/no_tauri.rs` reads
   `builder/api/Cargo.toml` (`env!("CARGO_MANIFEST_DIR")`) and fails when any line names `tauri`
   (a key or a `package =` value starting with `tauri`).
3. **A pnpm version that isn't a number is not run.** In `scripts/ci-local.sh`, before the `-lt`
   test, when `pnpm_major` is empty or holds anything but digits, print `ci-local: couldn't read
   pnpm's version, skipping the app screens (update pnpm: https://pnpm.io/installation)` and skip
   the pnpm steps. Check it with a stub `pnpm` printing `garbage` (temp folder, removed after).

Mutation-check 1 and 2 (`cp`/`cmp`). Run `cargo fmt --check`, `cargo clippy -p persona-builder-app
-p persona-builder-api --all-targets --locked -- -D warnings`, `cargo test -p persona-builder-app -p
persona-builder-api --locked` and `shellcheck scripts/ci-local.sh`. Don't commit.
