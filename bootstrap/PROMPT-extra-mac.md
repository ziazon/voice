# AI assistant: another Mac's history

Run this in Claude Code on any other Mac where the principal uses Claude Code, signed in as them,
when step 4 of `PROMPT.md` (on the main Mac) asks for it. It takes about ten minutes. Open Claude
Code in your clone on that Mac and say: *"Follow bootstrap/PROMPT-extra-mac.md with me, step by step."*

---

You are helping the principal collect their Claude Code history on this Mac for the AI assistant
they are building on their main Mac. Do these steps in order, with them watching, and say which step
you are on.

Rules: nothing is committed or shared from this Mac; the mined file goes only to the principal's own
main Mac. Never quote what the mined file holds, and never print what `private.json` holds. Never
write to `/tmp`.

1. **Consent.** Tell the principal that this reads their Claude Code history on this Mac, drops
   their own health, family, legal and household-money matters, secrets and any other person's
   identifier, and copies the rest to their main Mac. *Ask* (AskUserQuestion): "Go ahead?"
   (Yes / Not now). Ask for the assistant's name and pronouns, their own name and pronouns, and how
   to reach the main Mac (`user@host.local`, from System Settings → General → Sharing there), exactly
   as used on the main Mac.
2. **GitHub access.** `ssh -T -o StrictHostKeyChecking=accept-new git@github.com 2>&1 | head -1`
   must greet their GitHub username. If not, have them run `ssh-keygen -t ed25519` **in the Terminal
   app** (defaults), add `~/.ssh/id_ed25519.pub` at github.com → Settings → SSH and GPG keys, and
   check again. Your shell cannot answer password or yes/no prompts: anything that asks runs in the
   Terminal app, typed by them.
3. **Install and mine** (clone their private repo first if it is missing, into a folder of their choice; pass the same
   `--kit` as on the main Mac, if there is one):

   ```bash
   REPO=$(git rev-parse --show-toplevel)
   bash "$REPO/bootstrap/install.sh" --name <Name> --pronouns <pronouns> \
     --principal <their name> --principal-pronouns <their pronouns> --role extra --to <user@host.local> [--kit <kit folder>]
   ```

   It mines this Mac's history into `~/.claude/state/<slug>/decisions-<host>.jsonl` and tries to
   copy it to the main Mac with a key, no password. Show the counts it prints, then the folder list
   (`python3 -I "$REPO/scripts/crew_decisions.py" --member
   "$REPO/crew/<slug>/member.json" folders`) and ask whether any folder should be left out;
   if so, add it to `filters.excluded_folders` in `crew/<slug>/private.json` (the same as on the
   main Mac) and re-run step 3.
4. **If the copy failed** (the usual case the first time), the principal picks one of the two ways
   it printed: the two commands typed **in the Terminal app** (they ask for the main Mac's password;
   it must be awake, on the same network, with *Remote Login* on in System Settings → General →
   Sharing), or AirDrop. Confirm with them that the file is in `~/.claude/state/<slug>/` on the
   main Mac.

Then tell the principal this Mac is done and the rest happens on the main Mac (`PROMPT.md`).
