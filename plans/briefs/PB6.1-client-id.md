# PB6.1 brief: the GitHub sign-in's client ID

Persona Builder signs in to GitHub with the device flow (PB6, `builder/save/src/github.rs`). Until
now `CLIENT_ID` was `None`, so every GitHub step stopped at `text::NOT_SET_UP`. The OAuth App is
now registered (device flow on, expiring tokens off, no client secret made); its client ID is in
the slice's tracker card, not in this brief.

Read first: `AGENTS.md`, `.ai/SECURITY.md`, `plans/briefs/PB6-saving.md`,
`builder/save/src/github.rs`, `builder/cli/tests/save.rs`. Skills:
`~/.codex/skills/tdd-skill/SKILL.md`, `~/.codex/skills/security-and-hardening/SKILL.md`.

## Source (checked 2026-10-24)

GitHub, *Authorizing OAuth apps*, § Device flow,
<https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow>:
`POST https://github.com/login/device/code` takes `client_id` (required) and `scope`; "The
`client_secret` is not needed for the device flow"; the flow must be enabled in the app's settings.

## The change

1. Red first, in `builder/cli/tests/save.rs`:
   - `clean_github_save_passes_privacy_before_sign_in` expects `text::SIGN_IN_FIRST` instead of
     `text::NOT_SET_UP` (privacy passes, then the missing sign-in stops it);
   - rename `choose_github_without_built_in_client_id_fails_before_anything_else` to
     `choose_github_asks_for_sign_in_before_anything_else`, drop its `client_id().is_none()`
     assertion, and expect `text::SIGN_IN_FIRST`; it still checks no `save.json` was written.
   Run them and see both fail on the assertion (they print `NOT_SET_UP`).
2. Green: `const CLIENT_ID: Option<&str> = Some("<the card's client ID>");`, with its doc comment
   citing the source above. Keep the `Option` and the `PERSONA_BUILDER_GITHUB_CLIENT_ID` build-time
   override: a fork that has no app of its own sets `None` and gets the same plain refusal.
3. The tests stay offline: they run with a temporary `HOME`, and `/usr/bin/security` then lists
   only the System keychain, so the token lookup finds nothing and no request reaches GitHub. Do
   not add a test that runs `save sign-in` (it would call GitHub).

## Done when

`bash scripts/ci-local.sh` is green; `gitleaks git --staged` finds nothing in the diff. The live
walk (sign in through GitHub's device page, choose a private repo for an invented assistant, push,
and a public repo refused) is attended and comes after the land.
