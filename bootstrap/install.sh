#!/usr/bin/env bash
# Set up an assistant's build on this Mac: check the tools, update this voice clone,
# fill crew/<slug>/ from the template, and mine this Mac's Claude Code history into the member's
# private state dir. Safe to run again: each run refreshes the corpus and keeps every mark.
#
#   install.sh --name Ari --principal Robin               mine this Mac (the main one), merge
#   install.sh --name Ari --principal Robin --role extra --to robin@main.local
#                                                         mine another Mac, copy its corpus over
#
# Options: --pronouns she/her (the assistant's; default they/them), --principal-pronouns (default
# they/them), --role main|extra (default main), --to USER@HOST or a local folder (extra role: where
# the main Mac's state dir is), --no-copy, --kit DIR (a private kit: DIR/private.json becomes the
# gitignored crew/<slug>/private.json, and DIR/crew/* replaces template files when crew/<slug>/ is
# first filled), --repo-dir DIR (default: this script's clone), --export ZIP (main role; default
# <state>/claudeai-export.zip when present), --project-field KEY (from `claudeai inspect`).
# Environment: CLAUDE_PROJECTS (default ~/.claude/projects), CREW_HOST.
#
# Nothing here reads another person's data or sends anything anywhere except an extra Mac's own
# corpus to the main Mac. The raw corpus stays in ~/.claude/state/<slug>/ and is never committed.
set -euo pipefail

name="" role="main" pronouns="they/them" principal="" principal_pronouns="they/them"
repo="" to="" copy=1 kit="" export_zip="" project_field=""

die() { echo "install: $*" >&2; exit 1; }
need_value() { [ "$#" -ge 2 ] && [ -n "$2" ] || die "$1 needs a value"; }

while [ "$#" -gt 0 ]; do
  case "$1" in
    --name) need_value "$@"; name="$2"; shift 2 ;;
    --role) need_value "$@"; role="$2"; shift 2 ;;
    --pronouns) need_value "$@"; pronouns="$2"; shift 2 ;;
    --principal) need_value "$@"; principal="$2"; shift 2 ;;
    --principal-pronouns) need_value "$@"; principal_pronouns="$2"; shift 2 ;;
    --repo-dir) need_value "$@"; repo="$2"; shift 2 ;;
    --to) need_value "$@"; to="$2"; shift 2 ;;
    --no-copy) copy=0; shift ;;
    --kit) need_value "$@"; kit="$2"; shift 2 ;;
    --export) need_value "$@"; export_zip="$2"; shift 2 ;;
    --project-field) need_value "$@"; project_field="$2"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option $1 (see --help)" ;;
  esac
done

[ -n "$name" ] || die "--name is required (the assistant's name)"
[ -n "$principal" ] || die "--principal is required (your own name: whose decisions the assistant learns)"
case "$role" in main|extra) ;; *) die "--role must be main or extra" ;; esac
[ "$role" = main ] || [ "$copy" = 0 ] || [ -n "$to" ] ||
  die "--to is needed for --role extra (the main Mac as USER@HOST, or a folder), or pass --no-copy"
[ -z "$kit" ] || [ -d "$kit" ] || die "--kit $kit is not a folder"
slug="$(printf '%s' "$name" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | sed 's/^-//; s/-$//')"
principal_slug="$(printf '%s' "$principal" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | sed 's/^-//; s/-$//')"
[ -n "$slug" ] || die "--name must contain a letter or digit"
[ -n "$principal_slug" ] || die "--principal must contain a letter or digit"
case "$to" in -*) die "--to must not start with -" ;; esac
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_repo="$(git -C "$script_dir" rev-parse --show-toplevel 2>/dev/null)" ||
  die "run this script from a clone: git clone <your fork's clone URL>; then bash <clone>/bootstrap/install.sh"
[ -n "$repo" ] || repo="$script_repo"
git -C "$repo" rev-parse --show-toplevel >/dev/null 2>&1 ||
  die "no repo at $repo: clone your fork first with git clone <your fork's clone URL>"

echo "== 1. Tools"
missing=""
for tool in python3 git claude; do
  if command -v "$tool" >/dev/null 2>&1; then echo "ok    $tool"; else echo "MISSING $tool"; missing="$missing $tool"; fi
done
if [ -n "$missing" ]; then
  echo "Install what is missing, then run this again:"
  echo "  python3, git: run 'xcode-select --install' (Apple's command line tools)"
  echo "  claude:       install Claude Code (https://claude.com/claude-code)"
  exit 1
fi
python3 -c 'import sys; sys.exit(sys.version_info < (3, 9))' || die "python3 3.9 or newer is needed"

echo "== 2. The voice repo ($repo)"
if git -C "$repo" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  if [ -n "$(git -C "$repo" status --porcelain --untracked-files=no)" ]; then
    echo "local changes in $repo: not pulling (your files are untouched)"
  elif [ "$(git -C "$repo" rev-parse --abbrev-ref HEAD)" != "main" ]; then
    echo "$repo is on branch $(git -C "$repo" rev-parse --abbrev-ref HEAD): not pulling"
  else
    if git -C "$repo" pull --ff-only --quiet; then
      echo "updated"
    else
      echo "could not update; using the local copy"
    fi
  fi
fi
crew="$repo/scripts/crew_decisions.py"
[ -f "$crew" ] || die "$crew not found: is $repo the voice repo?"

if [ -n "$kit" ]; then
  python3 -I - "$kit" <<'PYKIT'
import json, os, sys
from pathlib import Path
kit = Path(sys.argv[1])
try:
    for root, dirs, files in os.walk(kit, followlinks=False):
        if Path(root).is_symlink() or any((Path(root) / name).is_symlink() for name in dirs + files):
            raise ValueError("kit must not contain symlinks")
    for path in (kit / "crew/member.json", kit / "crew/private.json", kit / "private.json"):
        if not path.is_file():
            continue
        config = json.loads(path.read_text())
        if not isinstance(config, dict):
            raise ValueError("kit config must be a JSON object")
        for key in ("state_dir", "filters") if path.name == "member.json" else ("state_dir",):
            if key in config:
                raise ValueError(f"kit must not supply {key} in {path.name}")
except (ValueError, OSError) as exc:
    sys.exit("install: " + " ".join(str(exc).splitlines()))
PYKIT
fi

echo "== 3. crew/$slug"
member_dir="$repo/crew/$slug"
member="$member_dir/member.json"
if [ -f "$member" ]; then
  echo "exists: kept as is"
else
  python3 -I - "$repo/crew/_template" "$member_dir" "$name" "$slug" "$pronouns" "$principal" \
    "$principal_slug" "$principal_pronouns" "$repo" "$kit" <<'PY'
import datetime, json, shutil, sys
from pathlib import Path
template, target, name, slug, pronouns, principal, principal_slug, principal_pronouns, repo, kit = sys.argv[1:]
repo = Path(repo).resolve()
home = Path.home().resolve()
# Under the home folder the committed files say ~/…, so they name no account.
shown = "~/" + str(repo.relative_to(home)) if repo.is_relative_to(home) else str(repo)
values = {"NAME": name, "SLUG": slug, "PRONOUNS": pronouns, "PRINCIPAL": principal,
          "PRINCIPAL_SLUG": principal_slug, "PRINCIPAL_PRONOUNS": principal_pronouns,
          "REPO": shown, "DATE": datetime.date.today().isoformat()}
shutil.copytree(template, target, dirs_exist_ok=True)
(Path(target) / "README.md").unlink()
# A kit's files replace the template's before the placeholders are filled, so both are filled alike.
config = json.loads((Path(template) / "member.json").read_text())
if kit and (Path(kit) / "crew").is_dir():
    kit_config = Path(kit) / "crew/member.json"
    if kit_config.is_file():
        config.update(json.loads(kit_config.read_text()))
    shutil.copytree(Path(kit) / "crew", target, dirs_exist_ok=True)
    print("filled from the template, with the kit's files")
else:
    print("filled from the template")
def fill(text):
    for key, value in values.items():
        text = text.replace("{{" + key + "}}", value)
    return text

def fill_json(value):
    if isinstance(value, str):
        return fill(value)
    if isinstance(value, list):
        return [fill_json(item) for item in value]
    if isinstance(value, dict):
        return {fill(key): fill_json(item) for key, item in value.items()}
    return value

for path in Path(target).rglob("*"):
    if path.is_file() and path.name != "member.json":
        path.write_text(fill(path.read_text()))
(Path(target) / "member.json").write_text(json.dumps(fill_json(config), indent=2) + "\n")
PY
fi
if [ -n "$kit" ] && [ -f "$kit/private.json" ] && [ ! -f "$member_dir/private.json" ]; then
  cp "$kit/private.json" "$member_dir/private.json"
  echo "private.json from the kit (gitignored)"
fi
[ ! -f "$member_dir/private.json" ] || chmod 600 "$member_dir/private.json"

state="$HOME/.claude/state/$slug"
mkdir -p "$state"
chmod 700 "$state"
crew_run() { python3 -I "$crew" --member "$member" "$@"; }

echo "== 4. Mine this Mac's Claude Code history"
host="$(printf '%s' "${CREW_HOST:-$(hostname -s)}" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '-' | sed 's/-$//')"
projects="${CLAUDE_PROJECTS:-$HOME/.claude/projects}"
corpus="$state/decisions-$host.jsonl"
if [ -d "$projects" ]; then
  crew_run folders --projects "$projects" | tail -1
  # Nothing is held out per Mac (a cutoff after every record): the merge on the main Mac splits once.
  crew_run mine --projects "$projects" --out "$corpus" --cutoff 9999
else
  echo "no Claude Code history on this Mac ($projects)"
  : > "$corpus"
fi
chmod 600 "$corpus"

if [ "$role" = extra ]; then
  echo "== 5. Copy the corpus to the main Mac"
  if [ "$copy" = 0 ]; then
    echo "skipped (--no-copy). The file is $corpus"
  elif [ "${to#/}" != "$to" ]; then
    mkdir -p "$to" && cp "$corpus" "$to/" && echo "copied to $to"
  # Key-only and no prompts: run from Claude Code there is no terminal to type a password into.
  elif ssh -o BatchMode=yes -o ConnectTimeout=10 -o StrictHostKeyChecking=accept-new "$to" \
         "mkdir -p .claude/state/$slug && chmod 700 .claude/state/$slug" 2>/dev/null &&
       scp -q -o BatchMode=yes "$corpus" "$to:.claude/state/$slug/" 2>/dev/null; then
    echo "copied to $to:.claude/state/$slug/"
  else
    echo "Could not copy to $to without a password. Either:"
    echo "  a. In the Terminal app (it can ask for the main Mac's password), run:"
    echo "       ssh $to 'mkdir -p .claude/state/$slug && chmod 700 .claude/state/$slug'"
    echo "       scp '$corpus' '$to:.claude/state/$slug/'"
    echo "  b. Or AirDrop it: in Finder, Go > Go to Folder: $state, AirDrop $(basename "$corpus") to"
    echo "     the main Mac, then there move it from Downloads into ~/.claude/state/$slug/"
  fi
  echo "Done on this Mac. Next: on the main Mac, run install.sh again (PROMPT.md step 4)."
  exit 0
fi

echo "== 5. The claude.ai export"
[ -n "$export_zip" ] || { [ -f "$state/claudeai-export.zip" ] && export_zip="$state/claudeai-export.zip"; } || true
review="$state/claudeai-review.tsv"
if [ -z "$export_zip" ]; then
  echo "no export yet: request it (Settings > Privacy > Export data), open the manifest it sends,"
  echo "  download conversations-000.zip in your browser, save it as $state/claudeai-export.zip,"
  echo "  then run this again"
else
  crew_run claudeai inspect --export "$export_zip" ||
    die "the export's shape is not what the adapter reads (see above): stop, and report the field names (no content) upstream"
  titles=(claudeai titles --export "$export_zip" --out "$review")
  [ -z "$project_field" ] || titles+=(--project-field "$project_field")
  crew_run "${titles[@]}"
  chmod 600 "$review"
  unmarked="$(awk -F'\t' 'NR > 1 && $1 == ""' "$review" | wc -l | tr -d ' ')"
  total="$(awk 'NR > 1' "$review" | wc -l | tr -d ' ')"
  if [ "$unmarked" = "$total" ] && [ "$total" != 0 ]; then
    echo "none of the $total conversations is marked yet: review $review (PROMPT.md step 5),"
    echo "then run this again. Until then no claude.ai conversation is mined."
    rm -f "$state/decisions-claudeai.jsonl"
  else
    [ "$unmarked" = 0 ] || echo "$unmarked conversations are still unmarked: they are left out"
    crew_run claudeai mine --export "$export_zip" --review "$review" --out "$state/decisions-claudeai.jsonl" --cutoff 9999
    chmod 600 "$state/decisions-claudeai.jsonl"
  fi
fi

echo "== 6. Merge every machine's corpus"
inputs=()
for f in "$state"/decisions-*.jsonl; do
  [ -e "$f" ] && [ "$(basename "$f")" != decisions-interview.jsonl ] && inputs+=("$f")
done
echo "merging (a file from a Mac that no longer exists, or an old host name, should be deleted):"
for f in "${inputs[@]}"; do echo "  $(date -r "$f" '+%Y-%m-%d %H:%M')  $(basename "$f")"; done
merge=(merge --in "${inputs[@]}" --out "$state/decisions.jsonl" --holdout 0.15)
[ ! -f "$state/decisions-interview.jsonl" ] || merge+=(--train "$state/decisions-interview.jsonl")
crew_run "${merge[@]}"
chmod 600 "$state/decisions.jsonl"
questions="$(grep -c '"kind": "question"' "$state/decisions.jsonl" || true)"
echo "Done on the main Mac. $questions answered questions in the merged corpus."
if [ "$questions" -lt 150 ]; then
  echo "That is thin: the build adds the calibration interview (PROMPT.md step 7)."
fi
