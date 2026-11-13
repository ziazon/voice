use crate::{
    assistants,
    error::{Error, Result},
    forget,
    install_fs::{exists, io},
    install_record::{self, Added},
    member::Member,
};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug, Default, PartialEq, Eq)]
pub struct State {
    pub path: PathBuf,
    pub files: u64,
    pub link: bool,
    pub not_folder: bool,
    pub corpus: bool,
    pub live_log: bool,
}
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Found {
    pub slug: String,
    pub state: Option<State>,
    pub skill: Option<PathBuf>,
    pub claude_md: Option<PathBuf>,
    pub lines: Vec<usize>,
    pub old_folder: Option<PathBuf>,
}
impl Found {
    pub fn any(&self) -> bool {
        self.state.is_some()
            || self.skill.is_some()
            || !self.lines.is_empty()
            || self.old_folder.is_some()
    }
}

pub fn find(home: &Path, claude_home: &Path, slug: &str) -> Result<Found> {
    let dir = assistants::assistant_dir(home, slug)?;
    if fs::symlink_metadata(&dir).is_ok() {
        return Ok(Found {
            slug: slug.into(),
            ..Found::default()
        });
    }
    let state = find_state(home.join(".claude/state").join(slug))?;
    let skill_path = claude_home.join("skills").join(slug);
    let skill = if exists(&skill_path)? && !skill_claimed(claude_home, slug, &skill_path) {
        Some(skill_path)
    } else {
        None
    };
    let old_folder = skill
        .as_deref()
        .and_then(|path| find_old_folder(home, path, slug));
    let claude_md_path = claude_home.join("CLAUDE.md");
    let lines = find_lines(&claude_md_path, slug)?;
    let claude_md = (!lines.is_empty()).then_some(claude_md_path);
    Ok(Found {
        slug: slug.into(),
        state,
        skill,
        claude_md,
        lines,
        old_folder,
    })
}

fn find_state(path: PathBuf) -> Result<Option<State>> {
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        metadata => io(&path, metadata)?,
    };
    let link = metadata.is_symlink();
    let not_folder = !link && !metadata.is_dir();
    let mut state = State {
        path,
        link,
        not_folder,
        ..State::default()
    };
    if !link && !not_folder {
        state.files = forget::count_files(&state.path)? as u64;
        if state.files == 0 {
            return Ok(None);
        }
        state.corpus = exists(&state.path.join("decisions.jsonl"))?;
        state.live_log = exists(&state.path.join("live.jsonl"))?;
    }
    Ok(Some(state))
}

/// Reads a regular file of at most `cap` bytes through one open: never blocks on a FIFO or device,
/// never follows a final link unless `follow`. `Ok(None)` when it is missing, `Err` names why not.
pub(crate) fn read_capped(path: &Path, cap: u64, follow: bool) -> Result<Option<Vec<u8>>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | if follow { 0 } else { libc::O_NOFOLLOW })
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        file => io(path, file)?,
    };
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    if !io(path, file.metadata())?.is_file() {
        return Err(Error::Invalid(format!(
            "{name} isn't a plain file, so Voice can't read it"
        )));
    }
    let mut bytes = Vec::new();
    io(path, file.take(cap + 1).read_to_end(&mut bytes))?;
    if bytes.len() as u64 > cap {
        return Err(Error::Invalid(format!(
            "{name} is too big for Voice to read"
        )));
    }
    Ok(Some(bytes))
}

pub(crate) fn skill_claimed(claude_home: &Path, slug: &str, skill_path: &Path) -> bool {
    install_record::read(claude_home)
        .ok()
        .flatten()
        .is_some_and(|record| {
            record.slug == slug
                && record
                    .added
                    .iter()
                    .any(|added| matches!(added, Added::Dir { path } if path == skill_path))
        })
}

fn find_lines(path: &Path, slug: &str) -> Result<Vec<usize>> {
    Ok(old_lines(path, slug)?
        .into_iter()
        .map(|(number, _)| number)
        .collect())
}

pub(crate) fn old_lines(path: &Path, slug: &str) -> Result<Vec<(usize, String)>> {
    let Some(bytes) = read_capped(path, 4 * 1024 * 1024, true)? else {
        return Ok(Vec::new());
    };
    let needle = format!("/{slug}");
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<_> = text.lines().collect();
    let mut ends: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some(id) = line
            .trim()
            .strip_prefix("<!-- voice:end ")
            .and_then(|marker| marker.strip_suffix(" -->"))
        {
            ends.entry(id).or_default().push(index);
        }
    }
    let mut block_end = None;
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if block_end.is_some_and(|end| index <= end) {
            continue;
        }
        let trimmed = line.trim();
        if let Some(marker) = trimmed.strip_prefix("<!-- voice:begin ") {
            if let Some(id) = marker.strip_suffix(" -->") {
                block_end = ends
                    .get(id)
                    .and_then(|indices| indices.get(indices.partition_point(|&end| end <= index)))
                    .copied();
            }
            continue;
        }
        if trimmed.starts_with("<!-- voice:end ") {
            continue;
        }
        if mentions(line.as_bytes(), needle.as_bytes()) {
            found.push((index + 1, (*line).to_owned()));
        }
    }
    Ok(found)
}

fn mentions(line: &[u8], needle: &[u8]) -> bool {
    line.windows(needle.len()).enumerate().any(|(start, word)| {
        let before = start == 0
            || line[start - 1].is_ascii_whitespace()
            || matches!(line[start - 1], b'(' | b'[' | b'`' | b'"' | b'\'');
        let end = start + needle.len();
        let after = end == line.len()
            || !(line[end].is_ascii_alphanumeric() || matches!(line[end], b'_' | b'-' | b'/'));
        before && after && word.eq_ignore_ascii_case(needle)
    })
}

fn find_old_folder(home: &Path, skill: &Path, slug: &str) -> Option<PathBuf> {
    if !fs::symlink_metadata(skill).ok()?.is_dir() {
        return None;
    }
    let skill_md = skill.join("SKILL.md");
    let bytes = read_capped(&skill_md, 1024 * 1024, false).ok()??;
    let text = String::from_utf8_lossy(&bytes);
    let (_, member_option) = text.split_once("--member ")?;
    let token = member_option.split_whitespace().next()?;
    let token = token
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| token.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(token);
    if !token.ends_with("/member.json") {
        return None;
    }
    let path = token
        .strip_prefix("~/")
        .map_or_else(|| PathBuf::from(token), |rest| home.join(rest));
    if !path.is_absolute() {
        return None;
    }
    let folder = path.parent()?;
    let state = match fs::metadata(home.join(".claude/state")) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return None,
    };
    if let Some(state) = state {
        for ancestor in folder.ancestors() {
            let metadata = fs::metadata(ancestor).ok()?;
            if (metadata.dev(), metadata.ino()) == (state.dev(), state.ino()) {
                return None;
            }
        }
    }
    // The supplied home is a trusted root. Check each component below it, or every
    // component of an outside candidate, before opening its member file.
    let root = if folder.starts_with(home) {
        home
    } else {
        Path::new("/")
    };
    let mut checked = root.to_owned();
    for component in folder.strip_prefix(root).ok()?.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return None;
        }
        checked.push(component);
        if !fs::symlink_metadata(&checked).ok()?.is_dir() {
            return None;
        }
    }
    if !fs::symlink_metadata(folder).ok()?.is_dir() {
        return None;
    }
    let bytes = read_capped(&path, 1024 * 1024, false).ok()??;
    let value = serde_json::from_slice(&bytes).ok()?;
    let member = Member::from_value(value, &path.display().to_string()).ok()?;
    (member.slug == slug).then(|| folder.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        install, member,
        test_support::{Scratch, config},
    };
    use std::{fs, os::unix::fs::symlink};

    struct Fixture {
        scratch: Scratch,
        home: PathBuf,
        claude: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let scratch = Scratch::new();
            let home = scratch.path().join("home");
            let claude = home.join(".claude");
            fs::create_dir_all(&claude).unwrap();
            let home = home.canonicalize().unwrap();
            let claude = claude.canonicalize().unwrap();
            Self {
                scratch,
                home,
                claude,
            }
        }
        fn find(&self) -> Found {
            find(&self.home, &self.claude, "ari").unwrap()
        }
        fn state(&self) -> PathBuf {
            self.home.join(".claude/state/ari")
        }
        fn skill(&self, text: &str) -> PathBuf {
            let path = self.claude.join("skills/ari");
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("SKILL.md"), text).unwrap();
            path
        }
        fn old(&self) -> PathBuf {
            let path = self.home.join("old/crew/ari");
            fs::create_dir_all(&path).unwrap();
            fs::write(
                path.join("member.json"),
                serde_json::to_vec(&config()).unwrap(),
            )
            .unwrap();
            path
        }
    }
    #[test]
    fn nothing_found_and_no_folders_created() {
        let f = Fixture::new();
        assert_eq!(
            f.find(),
            Found {
                slug: "ari".into(),
                ..Found::default()
            }
        );
        assert!(!f.find().any());
        assert!(!f.claude.join("state").exists());
        assert!(!f.claude.join("skills").exists());
        assert!(!f.claude.join("voice").exists());
    }
    #[test]
    fn state_files_are_counted_recursively_with_links_counted_once() {
        let f = Fixture::new();
        fs::create_dir_all(f.state().join("nested/empty")).unwrap();
        fs::write(f.state().join("nested/notes"), "invented").unwrap();
        fs::write(f.state().join("decisions.jsonl"), "invented decisions").unwrap();
        symlink("missing", f.state().join("live.jsonl")).unwrap();
        symlink(f.state(), f.state().join("loop")).unwrap();
        let found = f.find();
        assert!(found.any());
        assert_eq!(
            found.state,
            Some(State {
                path: f.state(),
                files: 4,
                corpus: true,
                live_log: true,
                ..State::default()
            })
        );
        assert!(found.skill.is_none());
        assert!(found.lines.is_empty());
        assert!(found.old_folder.is_none());
        assert_eq!(
            fs::read(f.state().join("decisions.jsonl")).unwrap(),
            b"invented decisions"
        );
    }
    #[test]
    fn empty_state_folder_is_not_found() {
        let f = Fixture::new();
        fs::create_dir_all(f.state().join("nested/empty")).unwrap();
        assert!(!f.find().any());
    }
    #[test]
    fn state_links_and_non_folder_are_found_without_descending() {
        for target_exists in [false, true] {
            let f = Fixture::new();
            fs::create_dir_all(f.state().parent().unwrap()).unwrap();
            let target = f.scratch.path().join("outside");
            if target_exists {
                fs::create_dir(&target).unwrap();
                fs::write(target.join("decisions.jsonl"), "outside").unwrap();
            }
            symlink(&target, f.state()).unwrap();
            assert_eq!(
                f.find().state,
                Some(State {
                    path: f.state(),
                    link: true,
                    ..State::default()
                })
            );
        }
        let f = Fixture::new();
        fs::create_dir_all(f.state().parent().unwrap()).unwrap();
        fs::write(f.state(), "keep").unwrap();
        assert_eq!(
            f.find().state,
            Some(State {
                path: f.state(),
                not_folder: true,
                ..State::default()
            })
        );
    }
    #[test]
    fn unclaimed_skill_and_dangling_skill_are_found() {
        let f = Fixture::new();
        let skill = f.skill("invented skill");
        let found = f.find();
        assert_eq!(found.skill, Some(skill.clone()));
        assert!(found.any());
        assert!(found.state.is_none());
        assert!(found.lines.is_empty());
        assert!(found.old_folder.is_none());
        fs::remove_dir_all(&skill).unwrap();
        symlink("missing", &skill).unwrap();
        assert_eq!(f.find().skill, Some(skill));
    }
    #[test]
    fn voice_assistant_is_never_reported_as_older() {
        let f = Fixture::new();
        assistants::make(
            &f.home,
            &assistants::New {
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
            },
            "2026-11-12",
            "voice",
        )
        .unwrap();
        fs::create_dir_all(f.state()).unwrap();
        fs::write(f.state().join("decisions.jsonl"), "invented history").unwrap();
        f.skill("unclaimed skill");
        fs::write(f.claude.join("CLAUDE.md"), "/ari").unwrap();
        assert_eq!(
            f.find(),
            Found {
                slug: "ari".into(),
                ..Found::default()
            }
        );
    }
    #[test]
    fn every_existing_crew_entry_belongs_to_voice() {
        for kind in ["folder", "file", "link"] {
            let f = Fixture::new();
            let dir = assistants::assistant_dir(&f.home, "ari").unwrap();
            fs::create_dir_all(dir.parent().unwrap()).unwrap();
            match kind {
                "folder" => fs::create_dir(&dir).unwrap(),
                "file" => fs::write(&dir, "invented").unwrap(),
                _ => symlink("missing", &dir).unwrap(),
            }
            f.skill("unclaimed skill");
            assert_eq!(
                f.find(),
                Found {
                    slug: "ari".into(),
                    ..Found::default()
                }
            );
        }
    }
    #[test]
    fn voice_claimed_skill_is_not_found_and_unreadable_record_cannot_claim() {
        let f = Fixture::new();
        let made = crate::assistants::make(
            &f.home,
            &crate::assistants::New {
                name: "Ari",
                pronouns: "they/them",
                principal: "Robin",
                principal_pronouns: "they/them",
            },
            "2026-11-12",
            "voice",
        )
        .unwrap();
        install::turn_on(
            &made.dir,
            &install::Places {
                home: f.home.clone(),
                claude_home: f.claude.clone(),
            },
        )
        .unwrap();
        fs::remove_dir_all(&made.dir).unwrap();
        assert!(!f.find().any());
        fs::write(f.claude.join("voice/installed.json"), "not JSON").unwrap();
        assert_eq!(f.find().skill, Some(f.claude.join("skills/ari")));
    }
    #[test]
    fn claude_lines_have_word_boundaries_case_crlf_and_original_line_numbers() {
        let f = Fixture::new();
        let path = f.claude.join("CLAUDE.md");
        fs::write(
            &path,
            concat!(
                "- Ask Ari (/ari) before a decision\r\n",
                "run `/ari`.\r\n",
                "~/.claude/state/ari/x\r\n/ari/\r\n/ari-old\r\n/arik\r\n",
                " <!-- voice:begin another -->\r\n/ari\r\n<!-- voice:end another -->\r\n",
                "/ARI\r\n[/ari]\r\n\"/ari\"\r\n'/ari'\r\nx/ari\r\n/ari_2\r\n/ari9\r\n",
                "/ari\u{2026}\r\n"
            ),
        )
        .unwrap();
        let found = f.find();
        assert_eq!(found.lines, [1, 2, 10, 11, 12, 13, 17]);
        assert_eq!(found.claude_md, Some(path));
        assert!(found.any());
        assert!(found.state.is_none());
        assert!(found.skill.is_none());
    }
    #[test]
    fn claude_read_is_lossy_follows_only_claude_file_link_and_refuses_oversize() {
        let f = Fixture::new();
        let real = f.scratch.path().join("claude-real");
        let path = f.claude.join("CLAUDE.md");
        fs::write(&real, b"\xff /ari\n").unwrap();
        symlink(&real, &path).unwrap();
        assert_eq!(f.find().lines, [1]);
        fs::write(&real, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
        assert!(
            matches!(find(&f.home, &f.claude, "ari"), Err(Error::Invalid(message)) if message == "CLAUDE.md is too big for Voice to read")
        );
    }
    #[test]
    fn claude_file_at_four_mib_is_read_and_one_more_byte_is_refused() {
        let f = Fixture::new();
        let path = f.claude.join("CLAUDE.md");
        let mut bytes = vec![b' '; 4 * 1024 * 1024];
        bytes[..5].copy_from_slice(b"/ari\n");
        fs::write(&path, &bytes).unwrap();
        assert_eq!(f.find().lines, [1]);
        bytes.push(b' ');
        fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            find(&f.home, &f.claude, "ari"),
            Err(Error::Invalid(message)) if message == "CLAUDE.md is too big for Voice to read"
        ));
    }
    #[test]
    fn claude_fifo_without_a_writer_is_refused_at_once() {
        let f = Fixture::new();
        let path = f.claude.join("CLAUDE.md");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        assert!(matches!(
            find(&f.home, &f.claude, "ari"),
            Err(Error::Invalid(message)) if message == "CLAUDE.md isn't a plain file, so Voice can't read it"
        ));
    }
    #[test]
    fn unclosed_voice_marker_does_not_hide_matching_lines() {
        let f = Fixture::new();
        fs::write(
            f.claude.join("CLAUDE.md"),
            "<!-- voice:begin x --> /ari\n- Ask Ari (/ari)\n",
        )
        .unwrap();
        assert_eq!(f.find().lines, [2]);
    }
    #[test]
    fn unclosed_voice_begin_does_not_pair_with_another_blocks_end() {
        let f = Fixture::new();
        fs::write(
            f.claude.join("CLAUDE.md"),
            concat!(
                "<!-- voice:begin x -->\n",
                "- Ask Ari (/ari)\n",
                "  <!-- voice:begin y -->  \n",
                "- Ask Ari (/ari)\n",
                "  <!-- voice:end y -->  \n",
            ),
        )
        .unwrap();
        assert_eq!(f.find().lines, [2]);
    }
    #[test]
    fn many_unclosed_voice_begins_are_read_in_one_pass() {
        let f = Fixture::new();
        let mut text = String::new();
        for n in 0..120000 {
            text.push_str(&format!("<!-- voice:begin a{n} -->\n"));
        }
        text.push_str("- Ask Ari (/ari)\n");
        assert!(text.len() < 4 * 1024 * 1024);
        fs::write(f.claude.join("CLAUDE.md"), text).unwrap();
        let start = std::time::Instant::now();
        let found = f.find();
        let elapsed = start.elapsed();
        assert_eq!(found.lines, [120001]);
        assert!(elapsed.as_secs_f64() < 10.0, "find took {elapsed:?}");
    }
    #[test]
    fn a_reused_block_id_pairs_each_begin_with_its_next_end() {
        let f = Fixture::new();
        fs::write(
            f.claude.join("CLAUDE.md"),
            concat!(
                "<!-- voice:begin x -->\n",
                "- Ask Ari (/ari)\n",
                "<!-- voice:end x -->\n",
                "- Ask Ari (/ari)\n",
                "<!-- voice:begin x -->\n",
                "- Ask Ari (/ari)\n",
                "<!-- voice:end x -->\n",
                "- Ask Ari (/ari)\n",
            ),
        )
        .unwrap();
        assert_eq!(f.find().lines, [4, 8]);
    }
    #[test]
    fn only_voice_block_lines_do_not_set_claude_md() {
        let f = Fixture::new();
        fs::write(
            f.claude.join("CLAUDE.md"),
            "<!-- voice:begin x -->\n/ari\n<!-- voice:end x -->\n",
        )
        .unwrap();
        assert!(!f.find().any());
        assert!(f.find().claude_md.is_none());
    }
    #[test]
    fn old_folder_expands_home_and_accepts_one_pair_of_quotes() {
        for quote in ["", "\"", "'"] {
            let f = Fixture::new();
            let old = f.old();
            f.skill(&format!(
                "python3 -I tool --member {quote}~/old/crew/ari/member.json{quote} similar"
            ));
            assert_eq!(f.find().old_folder, Some(old));
        }
        let f = Fixture::new();
        let old = f.old();
        f.skill(&format!(
            "tool --member {}/member.json record",
            old.display()
        ));
        assert_eq!(f.find().old_folder, Some(old));
    }
    #[test]
    fn old_member_fifo_is_ignored_without_a_writer() {
        let f = Fixture::new();
        let old = f.old();
        let path = old.join("member.json");
        fs::remove_file(&path).unwrap();
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&path)
                .status()
                .unwrap()
                .success()
        );
        f.skill("--member ~/old/crew/ari/member.json");
        assert!(f.find().old_folder.is_none());
    }
    #[test]
    fn old_member_over_one_mib_is_ignored() {
        let f = Fixture::new();
        let old = f.old();
        let mut bytes = serde_json::to_vec(&config()).unwrap();
        bytes.resize(1024 * 1024 + 1, b' ');
        fs::write(old.join("member.json"), bytes).unwrap();
        f.skill("--member ~/old/crew/ari/member.json");
        assert!(f.find().old_folder.is_none());
    }
    #[test]
    fn old_member_private_json_is_never_opened() {
        use std::os::unix::fs::PermissionsExt;
        let f = Fixture::new();
        let old = f.old();
        let private = old.join("private.json");
        fs::write(&private, "not JSON: this file must never be read").unwrap();
        fs::set_permissions(&private, fs::Permissions::from_mode(0o000)).unwrap();
        f.skill("--member ~/old/crew/ari/member.json");
        let found = f.find().old_folder;
        fs::set_permissions(&private, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(found, Some(old));
    }
    #[test]
    fn private_state_is_never_an_old_folder() {
        let f = Fixture::new();
        fs::create_dir_all(f.state()).unwrap();
        fs::write(
            f.state().join("member.json"),
            serde_json::to_vec(&config()).unwrap(),
        )
        .unwrap();
        f.skill("--member ~/.claude/state/ari/member.json");
        assert!(f.find().old_folder.is_none());
        // Case-sensitive scratch filesystems cannot exercise this alias.
        if f.home.join(".CLAUDE").exists() {
            f.skill("--member ~/.Claude/state/ari/member.json");
            assert!(f.find().old_folder.is_none());
        }
    }
    #[test]
    fn private_state_through_a_linked_claude_root_is_never_an_old_folder() {
        let f = Fixture::new();
        let real = f.home.join("real/claude");
        fs::create_dir_all(real.parent().unwrap()).unwrap();
        fs::rename(&f.claude, &real).unwrap();
        symlink(&real, &f.claude).unwrap();
        fs::create_dir_all(real.join("state/ari")).unwrap();
        fs::write(
            real.join("state/ari/member.json"),
            serde_json::to_vec(&config()).unwrap(),
        )
        .unwrap();
        f.skill("--member ~/real/claude/state/ari/member.json");
        assert!(f.find().old_folder.is_none());
        let old = f.old();
        f.skill("--member ~/old/crew/ari/member.json");
        assert_eq!(f.find().old_folder, Some(old));
    }
    #[test]
    fn private_state_through_a_linked_state_folder_is_never_an_old_folder() {
        let f = Fixture::new();
        let real = f.home.join("real/state");
        fs::create_dir_all(&real).unwrap();
        symlink(&real, f.claude.join("state")).unwrap();
        fs::create_dir_all(real.join("ari")).unwrap();
        fs::write(
            real.join("ari/member.json"),
            serde_json::to_vec(&config()).unwrap(),
        )
        .unwrap();
        f.skill("--member ~/real/state/ari/member.json");
        assert!(f.find().old_folder.is_none());
        let old = f.old();
        f.skill("--member ~/old/crew/ari/member.json");
        assert_eq!(f.find().old_folder, Some(old));
    }
    #[test]
    fn invalid_old_folder_candidates_are_ignored() {
        for text in [
            "no member option",
            "--member crew/ari/member.json",
            "--member ~/old/crew/ari/not-member.json",
            "--member ~/missing/member.json",
            "--member invalid --member ~/old/crew/ari/member.json",
        ] {
            let f = Fixture::new();
            f.old();
            f.skill(text);
            assert!(f.find().old_folder.is_none(), "{text}");
        }
        let f = Fixture::new();
        let old = f.old();
        let mut member = member::read_member(&old.join("member.json")).unwrap();
        member.slug = "mo-lee".into();
        member::write_member(&old.join("member.json"), &member).unwrap();
        f.skill("--member ~/old/crew/ari/member.json");
        assert!(f.find().old_folder.is_none());
    }
    #[test]
    fn old_folder_and_skill_file_links_are_ignored() {
        let f = Fixture::new();
        let old = f.old();
        let moved = f.home.join("old-real");
        fs::rename(&old, &moved).unwrap();
        symlink(&moved, &old).unwrap();
        let skill = f.skill("--member ~/old/crew/ari/member.json");
        assert!(f.find().old_folder.is_none());
        fs::remove_file(skill.join("SKILL.md")).unwrap();
        let real = f.scratch.path().join("real-skill");
        fs::write(&real, format!("--member {}/member.json", moved.display())).unwrap();
        symlink(&real, skill.join("SKILL.md")).unwrap();
        assert!(f.find().old_folder.is_none());
    }
    #[test]
    fn old_skill_larger_than_one_mib_is_not_read() {
        let f = Fixture::new();
        f.old();
        let text = format!(
            "--member ~/old/crew/ari/member.json {}",
            " ".repeat(1024 * 1024)
        );
        f.skill(&text);
        assert!(f.find().old_folder.is_none());
    }
    #[test]
    fn linked_state_and_skills_parents_are_supported() {
        for folder in ["state", "skills"] {
            let f = Fixture::new();
            let target = f.scratch.path().join("outside");
            fs::create_dir(&target).unwrap();
            symlink(&target, f.claude.join(folder)).unwrap();
            fs::create_dir(target.join("ari")).unwrap();
            fs::write(target.join("ari/notes"), "invented").unwrap();
            let found = f.find();
            if folder == "skills" {
                assert_eq!(found.skill, Some(f.claude.join("skills/ari")));
            } else {
                assert_eq!(found.state.unwrap().files, 1);
            }
        }
    }
    #[test]
    fn linked_old_folder_ancestor_and_member_file_are_ignored() {
        let f = Fixture::new();
        f.old();
        let moved = f.home.join("old-real");
        fs::rename(f.home.join("old"), &moved).unwrap();
        symlink(&moved, f.home.join("old")).unwrap();
        f.skill("--member ~/old/crew/ari/member.json");
        assert!(f.find().old_folder.is_none());
        let f = Fixture::new();
        let old = f.old();
        let real = f.home.join("real-member.json");
        fs::rename(old.join("member.json"), &real).unwrap();
        symlink(&real, old.join("member.json")).unwrap();
        f.skill("--member ~/old/crew/ari/member.json");
        assert!(f.find().old_folder.is_none());
    }
    #[test]
    fn linked_custom_claude_root_is_supported() {
        let f = Fixture::new();
        let target = f.scratch.path().join("custom-real");
        fs::create_dir_all(target.join("skills/ari")).unwrap();
        fs::write(target.join("skills/ari/SKILL.md"), "invented skill").unwrap();
        let custom = f.scratch.path().join("custom-link");
        symlink(&target, &custom).unwrap();
        assert_eq!(
            find(&f.home, &custom, "ari").unwrap().skill,
            Some(custom.join("skills/ari"))
        );
    }

    #[test]
    fn invalid_slug_is_refused_before_reading_anything() {
        let f = Fixture::new();
        for slug in ["", "../ari", "Ari", "ari/x"] {
            assert!(
                matches!(find(&f.home, &f.claude, slug), Err(Error::Invalid(message)) if message == "invalid assistant slug")
            );
        }
    }
}
