# Contributing

Thanks for helping. This kit handles people's private history, so a few rules come first.

- **No real people.** Never add a member folder, a persona, or a real name, handle, email, Slack id,
  workspace or quote, not even in a test. Tests use fictional people (Sam, Robin, Ada, Ari, Alex, Jo)
  and `example.com` domains.
- **No raw history, no audio.** Nothing from `~/.claude/` and no voice recording ever enters the repo.
  `.gitignore` blocks audio files and `crew/*/private.json`; don't weaken it.
- **Test first.** A change to behaviour starts with a failing test in `tests/`.
- **Run the checks** before you open a pull request:

  ```bash
  bash scripts/ci-local.sh
  ```

  It needs Python 3.9 or newer and `shellcheck`. The Rust checks also need stable Rust
  (https://rustup.rs); without it they are skipped.
- **Keep the miners private by default.** Anything a miner writes goes to a state or work folder
  outside the repo, with files 0600 and folders 0700.

Open a pull request with what changed, why, and how you tested it.

## Testers' builds

The public build signs in with an Anthropic Console account only. A tester's copy also offers
Claude subscription sign-in. Build it with `bash scripts/build-mac-app.sh --testers`, or
`cargo build -p voice-cli --features claude-subscription` for the command alone. Never hand a
testers' build to anyone outside the testing group. No release passes this feature.
