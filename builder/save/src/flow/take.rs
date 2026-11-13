use super::*;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
};

const MAIN: &str = "refs/heads/main";
const INCOMING: &str = "refs/voice/incoming";

#[derive(Debug, PartialEq, Eq)]
pub enum Took {
    AlreadyNewest { full: String },
    Updated { full: String, changed: usize },
}

// Kept behind the existing save boundary: the caller owns the assistant's state lock.
impl Saver<'_> {
    pub fn take_newest(
        &self,
        home: &Path,
        member: &MemberPaths,
        stamp: &str,
        privacy: &dyn PrivacyCheck,
    ) -> Result<Took> {
        let record = read_record(&member.state_dir)?;
        let Some(SaveRecord::Github {
            owner,
            name,
            role: Role::Member,
            builder,
            ..
        }) = record
        else {
            return Err(Error::Invalid(text::not_a_member(&member.member.name)));
        };
        if !voice_core::backup::valid_stamp(stamp) {
            return Err(Error::Invalid("invalid update stamp".into()));
        }
        let member_dir = home.join(".claude/crew").join(&member.member.slug);
        if !fs::symlink_metadata(&member_dir).is_ok_and(|metadata| metadata.is_dir()) {
            return Err(Error::Invalid("member folder isn't plain".into()));
        }
        let repo = RepoRef { owner, name };
        let full = repo.full();
        let git_dir = member.state_dir.join("backup.git");
        let expected = format!("https://github.com/{full}.git");
        if !git_dir.exists()
            || !self
                .git
                .run(Env::Own, &git_dir, "remote", &["get-url", "--", "origin"])
                .is_ok_and(|url| url.trim_end() == expected)
        {
            return Err(Error::Invalid(
                "saved copy origin doesn't match the record".into(),
            ));
        }
        let github = self
            .github
            .ok_or_else(|| Error::Invalid(text::NOT_SET_UP.into()))?;
        self.git.check_version()?;
        let token = self
            .tokens
            .load()?
            .ok_or_else(|| Error::Invalid(text::SIGN_IN_FIRST.into()))?;
        github.account(&token)?;
        github.confirm_private(&token, &repo)?;
        let old_head = self.git.run(
            Env::Own,
            &git_dir,
            "rev-parse",
            &["--verify", "refs/heads/main^{commit}"],
        )?;
        let old_head = old_head.trim();
        let mut old_entries =
            opening_entries(self.git, &git_dir, &repo, &member.member.slug, MAIN)?;
        old_entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut old = BTreeMap::new();
        for (rel, oid, executable) in old_entries {
            let present = check_local(self.git, &git_dir, member, &rel, Some(&oid))?;
            old.insert(rel, (oid, executable, present));
        }
        let dir = home.join(".claude/crew").join(format!(
            ".{}.voice-updating-{stamp}-{}",
            member.member.slug,
            std::process::id()
        ));
        let mut staged = false;
        let result = (|| {
            git::fetch_main_into(self.git, &git_dir, &token, INCOMING)?;
            let new_head = self.git.run(
                Env::Own,
                &git_dir,
                "rev-parse",
                &["--verify", "refs/voice/incoming^{commit}"],
            )?;
            let new_head = new_head.trim();
            if new_head == old_head {
                return Ok(Took::AlreadyNewest { full: full.clone() });
            }
            if !self.git.ancestor(&git_dir, old_head, new_head)? {
                return Err(Error::Invalid(text::not_newer(&full)));
            }
            plain_parents(home, dir.parent().expect("crew folder"))?;
            open_io(&dir, fs::DirBuilder::new().mode(0o700).create(&dir))?;
            staged = true;
            let stage_member = dir.join("member");
            open_io(
                &stage_member,
                fs::DirBuilder::new().mode(0o700).create(&stage_member),
            )?;
            let entries =
                opening_entries(self.git, &git_dir, &repo, &member.member.slug, INCOMING)?;
            let mut new = BTreeMap::new();
            for (rel, oid, executable) in entries {
                write_opened_blob(self.git, &git_dir, &stage_member, &rel, &oid, executable)?;
                new.insert(rel, (oid, executable));
            }
            validate_opened(&stage_member, home, &repo, &member.member.slug, privacy)?;
            for rel in new.keys().filter(|rel| !old.contains_key(*rel)) {
                check_local(self.git, &git_dir, member, rel, None)?;
            }
            let changed: Vec<_> = new
                .iter()
                .filter(|(rel, (oid, executable))| {
                    old.get(*rel).is_none_or(|(old_oid, old_exec, _)| {
                        oid != old_oid || executable != old_exec
                    })
                })
                .map(|(rel, _)| rel.clone())
                .collect();
            // Missing unchanged files are restored too, but aren't a change in the saved tree.
            let repairs: Vec<_> = new
                .keys()
                .filter(|rel| old.get(*rel).is_some_and(|entry| !entry.2) && !changed.contains(rel))
                .cloned()
                .collect();
            let mut replacements = changed.clone();
            replacements.extend(repairs);
            replacements.sort();
            let removed: Vec<_> = old
                .keys()
                .filter(|rel| !new.contains_key(*rel))
                .cloned()
                .collect();
            let trailer = self.git.run(
                Env::Own,
                &git_dir,
                "log",
                &[
                    "-1",
                    "--format=%(trailers:key=Saved-on,valueonly)",
                    INCOMING,
                ],
            )?;
            let trailer = trailer.trim();
            let mut applied = Vec::new();
            let mut made = Vec::new();
            let mut advanced = false;
            let apply = (|| {
                for rel in &replacements {
                    make_parents(&member.dir, rel, &mut made)?;
                    check_local(
                        self.git,
                        &git_dir,
                        member,
                        rel,
                        old.get(rel).map(|entry| entry.0.as_str()),
                    )?;
                    let target = member.dir.join(rel);
                    open_io(&target, fs::rename(stage_member.join(rel), &target))?;
                    applied.push(rel.clone());
                }
                for rel in &removed {
                    if check_local(self.git, &git_dir, member, rel, Some(&old[rel].0))? {
                        let target = member.dir.join(rel);
                        open_io(&target, fs::remove_file(&target))?;
                        applied.push(rel.clone());
                        prune_parents(&member.dir, rel);
                    }
                }
                self.git.run(
                    Env::Own,
                    &git_dir,
                    "update-ref",
                    &[MAIN, new_head, old_head],
                )?;
                advanced = true;
                self.git
                    .run(Env::Own, &git_dir, "update-ref", &["-d", INCOMING])?;
                if crate::computer::valid_computer_name(trailer)
                    && builder.as_deref() != Some(trailer)
                {
                    write_record(
                        &member.state_dir,
                        &SaveRecord::Github {
                            owner: repo.owner.clone(),
                            name: repo.name.clone(),
                            branch: "main".into(),
                            role: Role::Member,
                            builder: Some(trailer.into()),
                        },
                    )?;
                }
                Ok(Took::Updated {
                    full: full.clone(),
                    changed: changed.len() + removed.len(),
                })
            })();
            if apply.is_err() {
                if advanced {
                    let _ = self.git.run(
                        Env::Own,
                        &git_dir,
                        "update-ref",
                        &[MAIN, old_head, new_head],
                    );
                }
                for rel in applied.iter().rev() {
                    let _ = restore(self.git, &git_dir, &member.dir, rel, old.get(rel));
                }
                for folder in made.iter().rev() {
                    let _ = fs::remove_dir(folder);
                }
            }
            apply
        })();
        if staged {
            let _ = fs::remove_dir_all(&dir);
        }
        let cleanup = if matches!(result, Ok(Took::Updated { .. })) {
            Ok(String::new()) // Step 8 already deleted the incoming reference.
        } else {
            self.git
                .run(Env::Own, &git_dir, "update-ref", &["-d", INCOMING])
        };
        match result {
            Ok(took) => {
                cleanup?;
                Ok(took)
            }
            Err(error) => Err(error),
        }
    }
}

// Refuse links in ancestors too: O_NOFOLLOW protects only the final component.
fn check_local(
    git: &Git,
    git_dir: &Path,
    member: &MemberPaths,
    rel: &str,
    oid: Option<&str>,
) -> Result<bool> {
    let changed = || Error::Invalid(text::changed_here(&member.member.name, rel));
    let mut path = member.dir.clone();
    for part in rel.split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(metadata) => {
                if path != member.dir.join(rel) {
                    if !metadata.is_dir() {
                        return Err(changed());
                    }
                } else if !metadata.is_file() || oid.is_none() {
                    return Err(changed());
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return open_io(&path, Err(error)),
        }
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)
        .map_err(|_| changed())?;
    let mut bytes = Vec::new();
    file.take(10 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| changed())?;
    if bytes != git.run_bytes(git_dir, "cat-file", &["blob", oid.expect("existing entry")])? {
        return Err(changed());
    }
    Ok(true)
}
fn prune_parents(root: &Path, rel: &str) {
    let target = root.join(rel);
    let mut parent = target.parent();
    while let Some(folder) = parent {
        if folder == root || fs::remove_dir(folder).is_err() {
            break;
        }
        parent = folder.parent();
    }
}
fn restore(
    git: &Git,
    git_dir: &Path,
    root: &Path,
    rel: &str,
    old: Option<&(String, bool, bool)>,
) -> Result<()> {
    if let Some((oid, executable, true)) = old {
        make_parents(root, rel, &mut Vec::new())?;
        let target = root.join(rel);
        let bytes = git.run_bytes(git_dir, "cat-file", &["blob", oid])?;
        let (temporary, mut file) = (0u64..)
            .find_map(|sequence| {
                let temporary = target
                    .with_file_name(format!(".voice-rollback-{}-{sequence}", std::process::id()));
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .mode(if *executable { 0o755 } else { 0o644 })
                    .open(&temporary)
                {
                    Ok(file) => Some(Ok((temporary, file))),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => None,
                    Err(error) => Some(Err(Error::Io {
                        path: temporary,
                        source: error,
                    })),
                }
            })
            .expect("temporary sequence cannot be exhausted")?;
        let result = open_io(&temporary, file.write_all(&bytes))
            .and_then(|()| open_io(&target, fs::rename(&temporary, &target)));
        let _ = fs::remove_file(&temporary);
        result
    } else {
        let path = root.join(rel);
        open_io(&path, fs::remove_file(&path))?;
        Ok(())
    }
}
