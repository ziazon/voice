//! A reader child with only stdin, stdout and stderr across exec.
use std::{
    ffi::OsString, fs::File, io, os::unix::process::ExitStatusExt, path::Path, process::ExitStatus,
};

pub struct Spawned {
    pid: libc::pid_t,
    reaped: bool,
    pub stdin: Option<File>,
    pub stdout: Option<File>,
    status: Option<ExitStatus>,
}

impl Spawned {
    pub fn id(&self) -> u32 {
        self.pid as u32
    }
    pub fn kill(&mut self) -> io::Result<()> {
        if self.reaped {
            return Ok(());
        }
        // SAFETY: the unreaped child's pid cannot yet be reused.
        if unsafe { libc::kill(self.pid, libc::SIGKILL) } == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    fn wait_with(&mut self, flags: libc::c_int) -> io::Result<Option<ExitStatus>> {
        if self.reaped {
            return Ok(self.status);
        }
        loop {
            let mut status = 0;
            // SAFETY: status is writable and pid identifies this child.
            let result = unsafe { libc::waitpid(self.pid, &mut status, flags) };
            if result > 0 {
                let status = ExitStatus::from_raw(status);
                self.reaped = true;
                self.status = Some(status);
                return Ok(Some(status));
            }
            if result == 0 {
                return Ok(None);
            }
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EINTR) {
                return Err(error);
            }
        }
    }
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        self.wait_with(libc::WNOHANG)
    }
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        self.wait_with(0)?
            .ok_or_else(|| io::Error::other("child was not reaped"))
    }
}
impl Drop for Spawned {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.kill();
            let _ = self.wait();
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn spawn(
    _program: &Path,
    _args: &[OsString],
    _env: &[(OsString, OsString)],
    _dir: &Path,
) -> io::Result<Spawned> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "reader spawn needs macOS",
    ))
}

#[cfg(target_os = "macos")]
pub fn spawn(
    program: &Path,
    args: &[OsString],
    env: &[(OsString, OsString)],
    dir: &Path,
) -> io::Result<Spawned> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd, OwnedFd},
            unix::ffi::OsStrExt,
        },
    };

    fn checked(result: libc::c_int) -> io::Result<()> {
        if result == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(result))
        }
    }
    fn string(bytes: &[u8]) -> io::Result<CString> {
        CString::new(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NUL in reader launch"))
    }
    fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
        let mut ends = [0; 2];
        // SAFETY: ends has room for both descriptors.
        if unsafe { libc::pipe(ends.as_mut_ptr()) } == -1 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: pipe succeeded, transferring ownership of both descriptors.
        let pair = unsafe { (OwnedFd::from_raw_fd(ends[0]), OwnedFd::from_raw_fd(ends[1])) };
        for end in [&pair.0, &pair.1] {
            // SAFETY: end is a live descriptor owned by pair.
            if unsafe { libc::fcntl(end.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(pair)
    }
    fn child_end(end: OwnedFd) -> io::Result<OwnedFd> {
        if end.as_raw_fd() > 2 {
            return Ok(end);
        }
        // SAFETY: end is live; the new descriptor is unused and close-on-exec.
        let fd = unsafe { libc::fcntl(end.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
        if fd == -1 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fcntl returned a newly owned descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
    struct Attr(libc::posix_spawnattr_t);
    impl Drop for Attr {
        fn drop(&mut self) {
            // SAFETY: this guard holds an initialized attribute object.
            unsafe { libc::posix_spawnattr_destroy(&mut self.0) };
        }
    }
    struct Actions(libc::posix_spawn_file_actions_t);
    impl Drop for Actions {
        fn drop(&mut self) {
            // SAFETY: this guard holds initialized file actions.
            unsafe { libc::posix_spawn_file_actions_destroy(&mut self.0) };
        }
    }

    let (input, stdin) = pipe()?;
    let input = child_end(input)?;
    let (stdout, output) = pipe()?;
    let output = child_end(output)?;
    // SAFETY: zero is valid storage for initialization of the opaque attribute pointer.
    let mut attr = unsafe { std::mem::zeroed() };
    // SAFETY: attr is writable storage for initialization.
    checked(unsafe { libc::posix_spawnattr_init(&mut attr) })?;
    let mut attr = Attr(attr);
    // SAFETY: signal sets are plain data initialized by sigemptyset below.
    let (mut mask, mut defaults) = unsafe { (std::mem::zeroed(), std::mem::zeroed()) };
    // SAFETY: both sets are writable; SIGPIPE is a valid signal.
    unsafe {
        if libc::sigemptyset(&mut mask) == -1
            || libc::sigemptyset(&mut defaults) == -1
            || libc::sigaddset(&mut defaults, libc::SIGPIPE) == -1
        {
            return Err(io::Error::last_os_error());
        }
    }
    let flags = libc::POSIX_SPAWN_CLOEXEC_DEFAULT
        | libc::POSIX_SPAWN_SETSIGMASK
        | libc::POSIX_SPAWN_SETSIGDEF;
    // SAFETY: attr and both signal sets are initialized and live.
    unsafe {
        checked(libc::posix_spawnattr_setflags(
            &mut attr.0,
            flags as libc::c_short,
        ))?;
        checked(libc::posix_spawnattr_setsigmask(&mut attr.0, &mask))?;
        checked(libc::posix_spawnattr_setsigdefault(&mut attr.0, &defaults))?;
    }
    // SAFETY: zero is valid storage for initialization of the opaque actions pointer.
    let mut actions = unsafe { std::mem::zeroed() };
    // SAFETY: actions is writable storage for initialization.
    checked(unsafe { libc::posix_spawn_file_actions_init(&mut actions) })?;
    let mut actions = Actions(actions);
    let dir = string(dir.as_os_str().as_bytes())?;
    // SAFETY: actions is initialized; descriptors and NUL-terminated paths remain live.
    unsafe {
        checked(libc::posix_spawn_file_actions_adddup2(
            &mut actions.0,
            input.as_raw_fd(),
            0,
        ))?;
        checked(libc::posix_spawn_file_actions_adddup2(
            &mut actions.0,
            output.as_raw_fd(),
            1,
        ))?;
        checked(libc::posix_spawn_file_actions_addopen(
            &mut actions.0,
            2,
            c"/dev/null".as_ptr(),
            libc::O_WRONLY,
            0,
        ))?;
        checked(libc::posix_spawn_file_actions_addchdir_np(
            &mut actions.0,
            dir.as_ptr(),
        ))?;
    }
    let program = string(program.as_os_str().as_bytes())?;
    let argv = std::iter::once(Ok(program.clone()))
        .chain(args.iter().map(|arg| string(arg.as_bytes())))
        .collect::<io::Result<Vec<_>>>()?;
    let env = env
        .iter()
        .map(|(name, value)| {
            let mut pair = name.as_bytes().to_vec();
            pair.push(b'=');
            pair.extend_from_slice(value.as_bytes());
            string(&pair)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let pointers = |strings: &[CString]| {
        strings
            .iter()
            .map(|s| s.as_ptr().cast_mut())
            .chain(std::iter::once(std::ptr::null_mut()))
            .collect::<Vec<_>>()
    };
    let argv = pointers(&argv);
    let envp = pointers(&env);
    let mut pid = 0;
    // SAFETY: all objects and terminated pointer arrays remain live through posix_spawn.
    checked(unsafe {
        libc::posix_spawn(
            &mut pid,
            program.as_ptr(),
            &actions.0,
            &attr.0,
            argv.as_ptr(),
            envp.as_ptr(),
        )
    })?;
    Ok(Spawned {
        pid,
        reaped: false,
        stdin: Some(stdin.into()),
        stdout: Some(stdout.into()),
        status: None,
    })
}
