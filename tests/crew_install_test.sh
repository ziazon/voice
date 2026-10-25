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

# A Mac whose name a source's file would take mines under host-<name>, so withdraw never deletes it.
for host in live claudeai; do
  run "$extra" "$host" --role extra --no-copy
  [ -f "$extra/.claude/state/ari/decisions-host-$host.jsonl" ] || fail "a Mac named $host did not mine under host-$host"
  rm "$extra/.claude/state/ari/decisions-host-$host.jsonl"
done

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

# Robin adds two extra sources: sent mail from a Takeout mbox, and Ari's live log. Neither is read
# until private.json gives it a consent date; the live answers always train, even the newest.
state="$main/.claude/state/ari"
python3 -I - "$state" "$main/projects/voice/crew/ari/private.json" <<'PYSRC'
import json, sys
from email.message import EmailMessage
from pathlib import Path
state, private = Path(sys.argv[1]), Path(sys.argv[2])
mail = EmailMessage()
mail["From"], mail["To"], mail["Date"] = "Robin <robin@example.com>", "sam@example.org", "Tue, 01 Dec 2026 10:00:00 +0000"
mail["X-Gmail-Labels"] = "Sent"
mail.set_content("Open the term with the fieldwork story.\n\nOn Mon, Sam <sam@example.org> wrote:\n> hi\n")
(state / "sources").mkdir(exist_ok=True)
(state / "sources/mail.mbox").write_bytes(b"From x Tue Dec 01 10:00:00 2026\n" + mail.as_bytes())
(state / "live.jsonl").write_text(json.dumps({
    "timestamp": "2026-12-02T10:00:00Z", "work": "x.md", "kind": "escalated", "category": "scope",
    "tier": "medium", "question": "Drop the second reading?", "ari_pick": "Keep it", "rule": None,
    "robin_answer": "drop it, the first one carries the week"}) + "\n")
config = json.loads(private.read_text())
config["sources"] = [
    {"name": "mail", "kind": "mbox", "path": str(state / "sources/mail.mbox"), "own_addresses": ["robin@example.com"]},
    {"name": "live", "kind": "live"}]
private.write_text(json.dumps(config))
PYSRC
run "$main" main --role main
check "$scratch/out" "== 6. Extra sources" "missing extra sources step header"
check "$scratch/out" "mail: no consent yet, not read" "an unconsented source was not reported"
grep -q "fieldwork story" "$corpus" && fail "an unconsented source was mined"
python3 -I - "$main/projects/voice/crew/ari/private.json" <<'PYSRC'
import json, sys
from pathlib import Path
path = Path(sys.argv[1])
config = json.loads(path.read_text())
for source in config["sources"]:
    source["consent"] = "2026-10-25"
path.write_text(json.dumps(config))
PYSRC
run "$main" main --role main
check "$scratch/out" "mail: read 1 messages; kept 1 you sent" "the mbox was not read"
check "$scratch/out" "decisions-live.jsonl (always training)" "the live log was not merged as training"
[ "$(file_mode "$state/decisions-src-mail.jsonl")" = 600 ] || fail "the mail corpus is not owner-only"
check "$corpus" "fieldwork story" "the sent mail was not merged"
grep -q "sam@example.org" "$corpus" && fail "a recipient reached the corpus"
python3 -I - "$corpus" <<'PYSRC'
import json, sys
records = [json.loads(line) for line in open(sys.argv[1])]
live = [r for r in records if r["project"] == "live"]
assert [(r["own_words"], r["split"]) for r in live] == [("drop it, the first one carries the week", "train")], live
PYSRC


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
# A kit never brings sources: each is the principal's to name and consent to at step 5b.
for config in private.json crew/private.json crew/member.json; do
  printf '{"sources": [{"name": "live", "kind": "live", "consent": "2026-10-25"}]}\n' > "$badkit/$config"
  install_refused "kit must not supply sources" --kit "$badkit"
  rm "$badkit/$config"
done
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

# A no-history build runs on a relay Mac: its transcripts and export belong to somebody else.
# Run both new cases even while the flag is missing, so each reports its own red.
no_history_failures=0
set +e
(
  set -e
  relay="$scratch/relay"
  transcript "$relay/.claude/projects" "-Users-robin-projects-world" relay-session \
    "2026-09-01T10:00:00Z" "TRANSCRIPT_MUST_NOT_ENTER_ROUND_CORPUS"
  mkdir -p "$relay/.claude/state/ari"
  # Even an invalid export must not be inspected in this route.
  printf 'NOT_AN_EXPORT\n' > "$relay/.claude/state/ari/claudeai-export.zip"
  run "$relay" relay --no-history
  state="$relay/.claude/state/ari"
  [ ! -e "$state/decisions-relay.jsonl" ] || fail "--no-history created a host corpus"
  check "$scratch/out" "== 4." "missing history step header"
  check "$scratch/out" "skipped (--no-history): nothing is read from this Mac's Claude Code history" "missing history skip"
  check "$scratch/out" "== 5." "missing export step header"
  check "$scratch/out" "^skipped (--no-history)$" "missing export skip"
  check "$scratch/out" "nothing to merge yet: import a question round first (PROMPT.md, the route without a Claude history)" \
    "missing empty round guidance"
  grep -q 'That is thin' "$scratch/out" && fail "history hint on the no-history route"
  [ ! -e "$state/claudeai-review.tsv" ] || fail "--no-history inspected an export"

  python3 -I - "$scratch/round-questions.json" "$scratch/round-answers.json" <<'PYROUND'
import json, sys
from pathlib import Path
Path(sys.argv[1]).write_text(json.dumps({"questions": [
    {"n": 1, "question": "Which route first?", "recommended": "A",
     "options": [{"label": "Harbour"}, {"label": "Cliff"}]}]}))
Path(sys.argv[2]).write_text(json.dumps({"answered": "2026-11-02", "answers": {"1": {"pick": "A"}}}))
PYROUND
  HOME="$relay" python3 -I "$relay/projects/voice/scripts/crew_decisions.py" \
    --member "$relay/projects/voice/crew/ari/member.json" round --name spring \
    --questions "$scratch/round-questions.json" --answers "$scratch/round-answers.json" --heldout \
    > "$scratch/round-out" 2>&1 || { cat "$scratch/round-out"; fail "round import on the relay Mac"; }
  run "$relay" relay --no-history
  [ ! -e "$state/decisions-relay.jsonl" ] || fail "rerun mined the relay Mac"
  grep -q 'That is thin' "$scratch/out" && fail "history hint after a round import"
  python3 -I - "$state" <<'PYROUND'
import json, sys
from pathlib import Path
state = Path(sys.argv[1])
merged = [json.loads(line) for line in (state / "decisions.jsonl").read_text().splitlines()]
imported = [json.loads(line) for line in (state / "decisions-round-spring.jsonl").read_text().splitlines()]
assert merged == imported, (merged, imported)
assert [r["id"] for r in merged] == ["question:spring-01"]
assert merged[0]["split"] == "heldout"
assert "TRANSCRIPT_MUST_NOT_ENTER_ROUND_CORPUS" not in (state / "decisions.jsonl").read_text()
PYROUND
  printf '%s\n' '{"id":"message:stale","kind":"message","timestamp":"2026-09-01T10:00:00Z","project":"world","text":"STALE_HISTORY_MUST_NOT_MERGE"}' \
    > "$state/decisions-old.jsonl"
  cp "$state/decisions-old.jsonl" "$scratch/stale-history.jsonl"
  printf '%s\n' '{"timestamp":"2026-09-01T10:00:00Z","question":"Q?","ari_pick":"A","robin_answer":"LIVE_MUST_NOT_MERGE"}' \
    > "$state/live.jsonl"
  cp "$state/decisions-old.jsonl" "$state/decisions-live.jsonl"
  run "$relay" relay --no-history
  check "$scratch/out" "ignored (--no-history): decisions-live.jsonl" "a live corpus was not ignored"
  check "$scratch/out" "skipped (--no-history): the extra sources" "the extra sources ran on the relay Mac"
  check "$scratch/out" "ignored (--no-history): decisions-old.jsonl" "stale history was not reported as ignored"
  cmp -s "$state/decisions-old.jsonl" "$scratch/stale-history.jsonl" || fail "--no-history changed the stale corpus"
  python3 -I - "$state" <<'PYROUND'
import json, sys
from pathlib import Path
state = Path(sys.argv[1])
corpus = (state / "decisions.jsonl").read_text()
merged = [json.loads(line) for line in corpus.splitlines()]
imported = [json.loads(line) for line in (state / "decisions-round-spring.jsonl").read_text().splitlines()]
assert merged == imported, (merged, imported)
assert "STALE_HISTORY_MUST_NOT_MERGE" not in corpus
PYROUND
)
no_history_result=$?
[ "$no_history_result" = 0 ] || no_history_failures=$((no_history_failures + 1))
(
  set -e
  refused "--no-history is for the main role only" --name Ari --principal Robin --no-history --role extra --no-copy
)
no_history_result=$?
[ "$no_history_result" = 0 ] || no_history_failures=$((no_history_failures + 1))
set -e
[ "$no_history_failures" = 0 ] || fail "$no_history_failures no-history installer cases failed"

echo "crew_install_test: all passed"
