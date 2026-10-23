#!/usr/bin/env bash
# Run bootstrap/install.sh end to end in scratch HOMEs, with synthetic transcripts and a synthetic
# claude.ai export, for a made-up principal (Robin) and assistant (Ari): an extra Mac that mines and
# copies its corpus to the main Mac, then the main Mac before and after the mine / not-mine review,
# then again to show a rerun changes nothing. A private kit adds a folder filter and replaces one
# template file. Never real data.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
check() { grep -q -- "$2" "$1" || { cat "$1" >&2; fail "$3"; }; }
file_mode() { python3 -I -c 'import os, stat, sys; print(format(stat.S_IMODE(os.stat(sys.argv[1]).st_mode), "o"))' "$1"; }

# A voice repo holding this checkout's scripts, template and .gitignore, as a fork's clone would.
src="$scratch/voice-src"
mkdir -p "$src/crew"
cp -R "$here/scripts" "$src/scripts"
cp -R "$here/bootstrap" "$src/bootstrap"
cp -R "$here/crew/_template" "$src/crew/_template"
cp "$here/.gitignore" "$src/.gitignore"
git -C "$src" init --quiet -b main
git -C "$src" add -A
git -C "$src" -c user.name=t -c user.email=t@t commit --quiet -m fixture

# A stand-in for the claude CLI: install.sh only checks it exists.
mkdir -p "$scratch/bin"
printf '#!/bin/sh\nexit 0\n' > "$scratch/bin/claude"
chmod +x "$scratch/bin/claude"
export PATH="$scratch/bin:$PATH"

transcript() {  # folder session when kind text
  mkdir -p "$1/$2"
  python3 -I - "$1/$2/$3.jsonl" "$4" "$5" <<'PY'
import json, sys
path, when, text = sys.argv[1:]
human = {"type": "user", "timestamp": when, "origin": {"kind": "human"},
         "message": {"role": "user", "content": text}}
question = {"type": "user", "timestamp": when.replace("10:00", "10:05"),
            "toolUseResult": {"questions": [{"question": f"Which section first? ({text[:12]})", "header": "Order",
                                             "multiSelect": False,
                                             "options": [{"label": "Aims (Recommended)", "description": ""},
                                                         {"label": "Budget", "description": ""}]}],
                              "answers": {f"Which section first? ({text[:12]})": "Aims (Recommended)"}}}
open(path, "w").write(json.dumps(human) + "\n" + json.dumps(question) + "\n")
PY
}

extra="$scratch/extra"
main="$scratch/main"
transcript "$extra/.claude/projects" "-Users-robin-projects-course" s1 "2026-09-01T10:00:00Z" "keep four grade levels in the course"
transcript "$extra/.claude/projects" "-Users-robin-hoa-board" s2 "2026-09-02T10:00:00Z" "the hoa budget"
transcript "$main/.claude/projects" "-Users-robin-projects-grants" s3 "2026-09-03T10:00:00Z" "lead the aims with the partner need"
transcript "$main/.claude/projects" "-Users-robin" s4 "2026-09-04T10:00:00Z" "my doctor moved the appointment"

# The private kit: a folder only Robin's config should drop, and Robin's own interview.
kit="$scratch/kit"
mkdir -p "$kit/crew"
printf '{"filters": {"excluded_folders": "hoa-board"}}\n' > "$kit/private.json"
printf '# {{NAME}} interview, from the kit ({{DATE}}, {{REPO}})\n' > "$kit/crew/interview.md"

python3 -I - "$scratch/export.zip" <<'PY'
import json, sys, zipfile
def msg(sender, when, text):
    return {"uuid": when, "sender": sender, "created_at": when, "text": text,
            "content": [{"type": "text", "text": text}]}
conversations = [
    {"uuid": "c-mine", "name": "Policy edits", "created_at": "2026-08-01T09:00:00Z", "updated_at": "2026-08-01T09:00:00Z",
     "chat_messages": [msg("human", "2026-08-01T09:00:00Z", "use the plain-language version for researchers")]},
    {"uuid": "c-other", "name": "Deploy checklist", "created_at": "2026-08-02T09:00:00Z", "updated_at": "2026-08-02T09:00:00Z",
     "chat_messages": [msg("human", "2026-08-02T09:00:00Z", "ship the staging release tonight")]},
]
with zipfile.ZipFile(sys.argv[1], "w") as z:
    z.writestr("conversations.json", json.dumps(conversations))
PY

run() {  # home role args... -> output in $scratch/out
  local home="$1"; shift
  if [ ! -d "$home/projects/voice" ]; then
    mkdir -p "$home/projects"
    git clone --quiet "$src" "$home/projects/voice"
  fi
  HOME="$home" CREW_HOST="$1" bash "$home/projects/voice/bootstrap/install.sh" --name Ari --principal Robin --kit "$kit" \
    "${@:2}" > "$scratch/out" 2>&1 || { cat "$scratch/out" >&2; fail "install.sh $*"; }
}
refused() {  # message args... : install.sh must stop with that message
  local want="$1"; shift
  HOME="$extra" bash "$here/bootstrap/install.sh" "$@" > "$scratch/out" 2>&1 && fail "install.sh $* did not stop"
  check "$scratch/out" "$want" "install.sh $* stopped without saying: $want"
}

refused "--principal is required" --name Ari --role main
refused "--to is needed" --name Ari --principal Robin --role extra
refused "clone your fork" --name Ari --principal Robin --repo-dir "$scratch/nowhere"

# The extra Mac mines, drops the kit's folder, and copies its corpus to the main Mac's state dir.
run "$extra" extra --role extra --to "$main/.claude/state/ari"
check "$scratch/out" "copied to" "the extra Mac's corpus was not copied"
[ -f "$main/.claude/state/ari/decisions-extra.jsonl" ] || fail "no extra corpus on the main Mac"
grep -q "hoa budget" "$main/.claude/state/ari/decisions-extra.jsonl" && fail "the kit's folder filter did not apply"
member="$extra/projects/voice/crew/ari"
[ -f "$member/member.json" ] || fail "crew/ari was not filled"
grep -rq '{{' "$member" && fail "a placeholder was left in crew/ari"
python3 -I -c 'import json,sys; m=json.load(open(sys.argv[1])); assert (m["name"], m["slug"], m["principal"], m["principal_pronouns"], m["mark_word"]) == ("Ari", "ari", "Robin", "they/them", "Ari")' "$member/member.json"
check "$member/IDENTITY.md" "Ari (they/them)" "IDENTITY.md does not name Ari"
check "$member/interview.md" "Ari interview, from the kit" "the kit did not replace interview.md"
check "$member/SKILL.md" "python3 -I ~/projects/voice/scripts/crew_decisions.py" "SKILL.md does not use a home-relative repo path"
[ "$(file_mode "$member/private.json")" = 600 ] || fail "private.json is not owner-only"
git -C "$extra/projects/voice" check-ignore -q crew/ari/private.json || fail "private.json is not gitignored"
grep -rq "$extra" "$member" && fail "a committed file names the account's home folder"
grep -q '"heldout"' "$main/.claude/state/ari/decisions-extra.jsonl" && fail "a per-Mac corpus held records out"
[ "$(file_mode "$extra/.claude/state/ari")" = 700 ] || fail "the state dir is not private"

# The main Mac, export saved but not reviewed yet: titles listed, nothing from claude.ai mined.
cp "$scratch/export.zip" "$main/.claude/state/ari/claudeai-export.zip"
run "$main" main --role main
check "$scratch/out" "ok: every field the adapter reads is present" "inspect did not pass"
check "$scratch/out" "none of the 2 conversations is marked yet" "the unreviewed export was not held back"
[ -f "$main/.claude/state/ari/decisions-claudeai.jsonl" ] && fail "an unreviewed export was mined"
corpus="$main/.claude/state/ari/decisions.jsonl"
check "$corpus" "keep four grade levels" "the extra Mac's corpus was not merged"
check "$corpus" "lead the aims" "the main Mac's corpus was not merged"
grep -q "my doctor" "$corpus" && fail "a personal record was mined"

# Robin marks theirs; the main Mac mines only that conversation.
python3 -I "$main/projects/voice/scripts/crew_decisions.py" --member "$main/projects/voice/crew/ari/member.json" \
  claudeai mark --review "$main/.claude/state/ari/claudeai-review.tsv" --uuid c-mine --as y > /dev/null
run "$main" main --role main
check "$scratch/out" "1 conversations are still unmarked: they are left out" "the unmarked conversation was not reported"
check "$corpus" "plain-language version" "the conversation marked mine was not mined"
grep -q "staging release" "$corpus" && fail "an unmarked conversation was mined"
cp "$corpus" "$scratch/first.jsonl"

# A rerun keeps the marks and produces the same corpus.
run "$main" main --role main
cmp -s "$corpus" "$scratch/first.jsonl" || fail "a rerun changed the merged corpus"
check "$main/.claude/state/ari/claudeai-review.tsv" "^y	" "a rerun lost the review mark"


# The executable clone is authoritative, regardless of cwd, HOME, or clone folder name.
odd="$scratch/a clone elsewhere"
git clone --quiet "$src" "$odd"
install_at() {
  HOME="$scratch/new-home" CLAUDE_PROJECTS="$scratch/no-history" bash "$odd/bootstrap/install.sh" "$@" \
    > "$scratch/out" 2>&1
}
install_refused() {
  local want="$1"; shift
  install_at --name Ari --principal Robin "$@" && fail "installer accepted $*"
  check "$scratch/out" "$want" "installer did not explain rejection"
  grep -q Traceback "$scratch/out" && fail "installer printed a traceback"
  return 0
}
install_at --name 'Ari "Ace"' --principal 'Robin "Bo" \ Example' || { cat "$scratch/out"; fail "escaped names"; }
python3 -I - "$odd/crew/ari-ace/member.json" <<'PYTEST'
import json, sys
m = json.load(open(sys.argv[1]))
assert m['name'] == 'Ari "Ace"'
assert m['principal'] == 'Robin "Bo" \\ Example'
PYTEST
[ -f "$odd/crew/ari-ace/member.json" ] || fail "installer ignored its own clone"
install_refused "--principal must contain" --principal '!!!'
install_refused "--to must not start" --role extra --to '-oProxyCommand=bad'
mkdir -p "$odd/crew/ari"
printf 'keep\n' > "$odd/crew/ari/local-note.txt"
install_at --name Ari --principal Robin || { cat "$scratch/out"; fail "existing incomplete member"; }
check "$odd/crew/ari/local-note.txt" keep "recovery lost existing files"
git -C "$odd" remote remove origin
install_at --name Ari --principal Robin || fail "pull failure stopped installation"
check "$scratch/out" 'could not update; using the local copy' "missing local-copy fallback"

badkit="$scratch/badkit"
mkdir -p "$badkit/crew"
for key in filters state_dir; do
  printf '{"%s": {}}\n' "$key" > "$badkit/crew/member.json"
  install_refused "kit must not supply $key" --kit "$badkit"
done
rm "$badkit/crew/member.json"
printf '{"state_dir": "elsewhere"}\n' > "$badkit/private.json"
install_refused "kit must not supply state_dir" --kit "$badkit"
rm "$badkit/private.json"
for target in "$scratch/missing" "$kit/crew" "$kit/crew/interview.md"; do
  ln -s "$target" "$badkit/crew/link"
  install_refused "kit must not contain symlinks" --kit "$badkit"
  rm "$badkit/crew/link"
done
# An allowed partial kit config retains template filters and state_dir.
printf '{"pronouns": "she/her"}\n' > "$badkit/crew/member.json"
install_at --name Beck --principal Robin --kit "$badkit" || fail "safe partial kit config"
python3 -I - "$odd" <<'PYTEST'
import json, sys
from pathlib import Path
repo = Path(sys.argv[1])
m = json.loads((repo / 'crew/beck/member.json').read_text())
t = json.loads((repo / 'crew/_template/member.json').read_text())
assert m['filters'] == t['filters']
assert m['state_dir'] == '~/.claude/state/beck'
assert m['pronouns'] == 'she/her'
PYTEST
# Running a loose copy must explain how to clone instead of silently choosing a repo.
mkdir -p "$scratch/loose/bootstrap"
cp "$here/bootstrap/install.sh" "$scratch/loose/bootstrap/install.sh"
HOME="$scratch/new-home" bash "$scratch/loose/bootstrap/install.sh" --name Ari --principal Robin \
  > "$scratch/out" 2>&1 && fail "loose installer accepted no clone"
check "$scratch/out" 'git clone' "loose installer did not explain cloning"

echo "crew_install_test: all passed"
