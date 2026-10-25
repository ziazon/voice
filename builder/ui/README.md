# Persona Builder screens

`@persona-builder/screens` is the app's Vue screens as a source package that another app can mount.
It uses Tabler's CSS and a host-provided `Backend` for the app's commands.

The screens so far: assistants, a new one, permission, connect Claude, your history, save a copy.

Node and pnpm 10 or later are needed. From the repo root, run the desktop app with:

```sh
pnpm -C builder/ui tauri dev
```

A host calls `provideBackend(backend)` in its parent component's setup, then mounts
`PersonaBuilder` as a child. It loads Tabler's CSS and chooses its theme. The standalone app
provides the Tauri backend and follows the system's light or dark theme.

The webview never names a file or folder. A request carries an assistant's slug or an id the Rust
side handed out; Rust finds every path itself and opens every native picker itself. A host app must
keep the same rule.

Run the screens' checks from the repo root:

```sh
pnpm -C builder/ui typecheck
pnpm -C builder/ui test
pnpm -C builder/ui build
```

The types in `src/types/generated/` come from `cargo test`; never edit them by hand.
