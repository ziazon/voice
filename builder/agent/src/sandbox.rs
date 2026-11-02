//! One sandboxed launch of the person's own Claude Code. See
//! docs/spikes/VO11-os-sandbox.md, https://code.claude.com/docs/en/setup and
//! https://code.claude.com/docs/en/network-config.

use crate::text;
#[cfg(target_os = "macos")]
use libc::{c_char, c_int, c_void};
#[cfg(target_os = "macos")]
use std::{
    ffi::{CStr, CString},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, MetadataExt},
    },
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
#[cfg(unix)]
use std::{
    fs::OpenOptions,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
use voice_core::error::{Error, Result};

const PROFILE: &str = include_str!("../sandbox/claude-code.sb");
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
pub const HELPER: &str = "open-sign-in";
pub const LINK: &str = "sign-in-link";
pub const MAX_LINK: usize = 4096;
// Claude Code 2.1.292's compiled client id, measured in the link handed to BROWSER.
pub const CLAUDE_CODE_CLIENT_ID: &str = "9d1c250a-e61b-44d9-88ed-5944d1962f5e";
#[cfg(target_os = "macos")]
const HELPER_SCRIPT: &str =
    "#!/bin/bash\numask 077\nprintf '%s\\n' \"$1\" > \"${0%/*}/sign-in-link\"\n";

/// A fresh folder owned by this launch, or the caller's work folder, left in place.
/// Sign-in should use Fresh: a plain child sharing Given can write sign-in-link; valid_link limits it.
/// A helper left by a crashed sign-in makes later sign-ins in Given fail; helpers are never replaced.
/// Any ACL, even a deny-only one as on ~/Documents, is refused; VO11.1c must not offer it as Given.
pub enum RunFolder<'a> {
    Fresh,
    Given(&'a Path),
}

pub struct Launch {
    program: PathBuf,
    direct: bool,
    profile: PathBuf,
    run: PathBuf,
    port: u16,
    sign_in: bool,
    home: PathBuf,
    fresh: bool,
    clean_given: bool,
}

/// The sign-in link, as the helper left it.
#[derive(Debug, PartialEq, Eq)]
pub enum Link {
    NotYet,
    Ready(String),
    Refused,
}

/// The folder that holds fresh run folders: `runs` beside the profile folder.
pub fn runs_folder(profile: &Path) -> PathBuf {
    profile
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join("runs")
}

/// The program with links resolved, if it is something Voice can run in the sandbox.
pub fn resolve_program(program: &Path) -> Result<PathBuf> {
    let program = fs::canonicalize(program).map_err(|_| invalid(text::DOES_NOT_RUN))?;
    let metadata = fs::metadata(&program).map_err(|_| invalid(text::DOES_NOT_RUN))?;
    if !metadata.is_file() {
        return Err(invalid(text::DOES_NOT_RUN));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(invalid(text::DOES_NOT_RUN));
    }
    let mut magic = [0; 4];
    let mut file = File::open(&program).map_err(|_| invalid(text::DOES_NOT_RUN))?;
    if file.read_exact(&mut magic).is_err()
        || !matches!(
            magic,
            [0xcf, 0xfa, 0xed, 0xfe] | [0xca, 0xfe, 0xba, 0xbe] | [0xca, 0xfe, 0xba, 0xbf]
        )
    {
        return Err(invalid(text::NOT_NATIVE));
    }
    Ok(program)
}

/// Whether `link` is a sign-in page Voice may open.
pub fn valid_link(link: &str) -> bool {
    if link.is_empty()
        || link.len() > MAX_LINK
        || !link
            .bytes()
            .all(|byte| (0x21..=0x7e).contains(&byte) && !b"\"<>\\^`{|}#".contains(&byte))
        || !(link.starts_with("https://claude.com/cai/oauth/authorize?")
            || link.starts_with("https://claude.ai/oauth/authorize?"))
    {
        return false;
    }
    let mut query = std::collections::HashMap::new();
    for pair in link.split_once('?').unwrap().1.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            return false;
        };
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return false;
        }
        let Some(value) = decode_query_value(value) else {
            return false;
        };
        if query.insert(key, value).is_some() {
            return false;
        }
    }
    if query.get("client_id").map(Vec::as_slice) != Some(CLAUDE_CODE_CLIENT_ID.as_bytes())
        || query.get("response_type").map(Vec::as_slice) != Some(b"code")
    {
        return false;
    }
    let Some(callback) = query.get("redirect_uri") else {
        return false;
    };
    let Some(port) = callback
        .strip_prefix(b"http://localhost:")
        .and_then(|rest| rest.strip_suffix(b"/callback"))
    else {
        return false;
    };
    (1..=5).contains(&port.len())
        && port[0] != b'0'
        && port.iter().all(u8::is_ascii_digit)
        && std::str::from_utf8(port).unwrap().parse::<u16>().is_ok()
}

fn decode_query_value(value: &str) -> Option<Vec<u8>> {
    let mut bytes = value.bytes();
    let mut decoded = Vec::new();
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    Some(decoded)
}

impl Launch {
    #[cfg(not(target_os = "macos"))]
    pub fn new(
        _program: &Path,
        _profile: &Path,
        _home: &Path,
        _run: RunFolder<'_>,
        _proxy_port: u16,
        _sign_in: bool,
    ) -> Result<Launch> {
        Err(invalid(text::MAC_ONLY))
    }

    #[cfg(target_os = "macos")]
    pub fn new(
        program: &Path,
        profile: &Path,
        home: &Path,
        run: RunFolder<'_>,
        proxy_port: u16,
        sign_in: bool,
    ) -> Result<Launch> {
        if proxy_port == 0 {
            return Err(invalid("the proxy port must be set"));
        }
        #[cfg(feature = "test-stubs")]
        let (program, direct) = {
            let canonical = fs::canonicalize(program).map_err(|_| invalid(text::DOES_NOT_RUN))?;
            let metadata = fs::metadata(&canonical).map_err(|_| invalid(text::DOES_NOT_RUN))?;
            if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
                return Err(invalid(text::DOES_NOT_RUN));
            }
            let mut magic = [0; 2];
            let mut file = File::open(&canonical).map_err(|_| invalid(text::DOES_NOT_RUN))?;
            if file.read_exact(&mut magic).is_ok() && magic == *b"#!" {
                (canonical, true)
            } else {
                (resolve_program(program)?, false)
            }
        };
        #[cfg(not(feature = "test-stubs"))]
        let (program, direct) = (resolve_program(program)?, false);
        private_folder(profile).map_err(|_| invalid(text::PROFILE_FAILED))?;
        let profile = fs::canonicalize(profile).map_err(|_| invalid(text::PROFILE_FAILED))?;
        private_folder(&profile).map_err(|_| invalid(text::PROFILE_FAILED))?;
        let fresh = matches!(run, RunFolder::Fresh);
        let run = match run {
            RunFolder::Fresh => fresh_folder(&profile),
            RunFolder::Given(path) => private_folder(path).and_then(|()| {
                let path = fs::canonicalize(path)?;
                private_folder(&path)?;
                Ok(path)
            }),
        }
        .map_err(|_| invalid(text::RUN_FOLDER_FAILED))?;
        let mut launch = Launch {
            program,
            direct,
            profile,
            run,
            port: proxy_port,
            sign_in,
            home: home.to_owned(),
            fresh,
            clean_given: false,
        };
        if sign_in {
            let mut helper = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o700)
                .custom_flags(libc::O_NOFOLLOW)
                .open(launch.run.join(HELPER))
                .map_err(|_| invalid(text::RUN_FOLDER_FAILED))?;
            // A failed create must leave the caller's pre-existing files alone.
            launch.clean_given = true;
            match fs::remove_file(launch.run.join(LINK)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return Err(invalid(text::RUN_FOLDER_FAILED)),
            }
            helper
                .write_all(HELPER_SCRIPT.as_bytes())
                .map_err(|_| invalid(text::RUN_FOLDER_FAILED))?;
        }
        Ok(launch)
    }

    pub fn run_folder(&self) -> &Path {
        &self.run
    }

    /// The sandboxed command, in the run folder, with only its own environment.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut command = if self.direct {
            Command::new(&self.program)
        } else {
            let mut command = Command::new(SANDBOX_EXEC);
            command.args(["-p", PROFILE]);
            for (name, value) in [
                ("PROGRAM", &self.program),
                ("PROFILE_DIR", &self.profile),
                ("RUN_DIR", &self.run),
            ] {
                let mut parameter = std::ffi::OsString::from(format!("{name}="));
                parameter.push(value);
                command.arg("-D").arg(parameter);
            }
            command.args([
                "-D",
                &format!("PROXY_PORT={}", self.port),
                "-D",
                if self.sign_in {
                    "SIGN_IN=yes"
                } else {
                    "SIGN_IN=no"
                },
            ]);
            command.arg(&self.program);
            command
        };
        command
            .args(["--setting-sources", "project,local"])
            .args(args)
            .current_dir(&self.run);
        self.environment(&mut command);
        command
    }

    fn environment(&self, command: &mut Command) {
        // The child gets only what it needs: no credential, provider switch or endpoint of the
        // person's reaches it (PB-D9).
        command.env_clear();
        command.env("HOME", &self.home);
        if let Some(login) = login_name() {
            command.env("USER", &login).env("LOGNAME", login);
        }
        command
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LANG", language());
        command
            .env("CLAUDE_CONFIG_DIR", &self.profile)
            .env("CLAUDE_CODE_TMPDIR", &self.run);
        let mut tmp = self.run.as_os_str().to_owned();
        tmp.push("/");
        command.env("TMPDIR", tmp);
        let proxy = format!("http://127.0.0.1:{}", self.port);
        command
            .env("HTTPS_PROXY", &proxy)
            .env("https_proxy", proxy)
            .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
            .env("DISABLE_AUTOUPDATER", "1");
        if self.sign_in {
            command.env("BROWSER", self.run.join(HELPER));
        }
    }

    /// Read the link once; a ready link's file is removed before handing it out.
    #[cfg(unix)]
    pub fn sign_in_link(&self) -> Link {
        let path = self.run.join(LINK);
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Link::NotYet,
            Err(_) => return Link::Refused,
        };
        if !file.metadata().is_ok_and(|metadata| metadata.is_file()) {
            return Link::Refused;
        }
        let mut bytes = Vec::new();
        let read = file.take((MAX_LINK + 2) as u64).read_to_end(&mut bytes);
        if read.is_ok() && !bytes.ends_with(b"\n") && bytes.len() <= MAX_LINK {
            return Link::NotYet;
        }
        let _ = fs::remove_file(path);
        if read.is_err() || bytes.pop() != Some(b'\n') {
            return Link::Refused;
        }
        match String::from_utf8(bytes) {
            Ok(link) if valid_link(&link) => Link::Ready(link),
            _ => Link::Refused,
        }
    }

    #[cfg(not(unix))]
    pub fn sign_in_link(&self) -> Link {
        Link::Refused
    }
}

impl Drop for Launch {
    fn drop(&mut self) {
        if self.fresh {
            let _ = fs::remove_dir_all(&self.run);
        } else if self.clean_given {
            let _ = fs::remove_file(self.run.join(HELPER));
            let _ = fs::remove_file(self.run.join(LINK));
        }
    }
}

fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}

fn language() -> String {
    std::env::var("LANG")
        .ok()
        .filter(|value| {
            (1..=64).contains(&value.len())
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_.@-".contains(&byte))
        })
        .unwrap_or_else(|| "en_US.UTF-8".into())
}

#[cfg(target_os = "macos")]
fn login_name() -> Option<String> {
    // SAFETY: getpwuid_r receives valid pointers and the buffer's length. On success with a
    // non-null result, entry is initialised and pw_name points into the buffer, which stays
    // alive until the null-terminated name is copied.
    unsafe {
        let size = libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX);
        let mut buffer = vec![
            0u8;
            if size == -1 {
                4096
            } else {
                usize::try_from(size).ok()?
            }
        ];
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        if libc::getpwuid_r(
            libc::getuid(),
            entry.as_mut_ptr(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            &mut result,
        ) != 0
            || result.is_null()
        {
            return None;
        }
        let entry = entry.assume_init();
        if entry.pw_name.is_null() {
            return None;
        }
        CStr::from_ptr(entry.pw_name)
            .to_str()
            .ok()
            .map(str::to_owned)
    }
}

#[cfg(not(target_os = "macos"))]
fn login_name() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn acl_get_link_np(path: *const c_char, kind: c_int) -> *mut c_void;
    fn acl_free(object: *mut c_void) -> c_int;
}

#[cfg(target_os = "macos")]
const ACL_TYPE_EXTENDED: c_int = 0x100;

/// Whether the path has an extended access list, without following a final link.
#[cfg(target_os = "macos")]
pub fn has_acl(path: &Path) -> std::io::Result<bool> {
    let path = CString::new(path.as_os_str().as_bytes())?;
    // SAFETY: path is null-terminated and alive for the call; a returned ACL is freed once.
    unsafe {
        let acl = acl_get_link_np(path.as_ptr(), ACL_TYPE_EXTENDED);
        if acl.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ENOENT) {
                Ok(false)
            } else {
                Err(error)
            }
        } else {
            acl_free(acl);
            Ok(true)
        }
    }
}

#[cfg(target_os = "macos")]
fn private_folder(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    // SAFETY: geteuid takes no arguments and cannot fail.
    let owner = unsafe { libc::geteuid() };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.permissions().mode() & 0o077 != 0
        || metadata.uid() != owner
    {
        return Err(std::io::Error::other("not a private directory"));
    }
    if has_acl(path)? {
        return Err(std::io::Error::other("not a private directory"));
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn fresh_folder(profile: &Path) -> std::io::Result<PathBuf> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = runs_folder(profile);
    match fs::symlink_metadata(&parent) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            if metadata.permissions().mode() & 0o077 != 0 {
                fs::set_permissions(&parent, fs::Permissions::from_mode(0o700))?;
            }
        }
        Ok(_) => return Err(std::io::Error::other("not a directory")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match fs::DirBuilder::new().mode(0o700).create(&parent) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(error) => return Err(error),
            }
        }
        Err(error) => return Err(error),
    }
    private_folder(&parent)?;
    loop {
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = parent.join(format!("{}-{number}-{nanos}", std::process::id()));
        match fs::DirBuilder::new().mode(0o700).create(&path) {
            Ok(()) => match fs::canonicalize(&path) {
                Ok(path) => {
                    if let Err(error) = private_folder(&path) {
                        let _ = fs::remove_dir(&path);
                        return Err(error);
                    }
                    return Ok(path);
                }
                Err(error) => {
                    let _ = fs::remove_dir_all(path);
                    return Err(error);
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}
