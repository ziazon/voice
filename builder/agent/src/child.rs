//! A child with a time limit, its output capped, and its whole process group killed on every way
//! out; stderr is never read here.
use std::{
    io::{self, Read, Write},
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

/// How a child run ended.
pub(crate) enum Ran {
    Finished {
        status: ExitStatus,
        stdout: Vec<u8>,
    },
    /// Past its time limit, or its output could not be collected.
    CutOff,
    NotStarted,
}

/// How long the output may still take to arrive once the child has exited.
const COLLECT_GRACE: Duration = Duration::from_secs(1);

/// Run `command` in a process group of its own, with `input` on its stdin (then closed) and at
/// most `cap` bytes of its stdout kept. Stderr goes nowhere: it can echo what the child was sent.
pub(crate) fn run(command: &mut Command, input: Option<&[u8]>, timeout: Duration, cap: u64) -> Ran {
    let started = Instant::now();
    command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0);
    let Ok(mut child) = command.spawn() else {
        return Ran::NotStarted;
    };
    let stdout = drain(child.stdout.take().expect("piped stdout"), cap);
    let stdin = child.stdin.take();
    let status = thread::scope(|scope| {
        if let (Some(input), Some(mut stdin)) = (input, stdin) {
            scope.spawn(move || {
                // Dropping `stdin` closes the pipe, so the child sees the end of its input. A child
                // that exits without reading turns this into EPIPE, which is ignored.
                let _ = stdin.write_all(input);
            });
        }
        // Every way out kills the group first: descendants may still hold the pipes open, and the
        // scope waits for the writer, which a held-open, unread stdin would block.
        settle(&mut child, started, timeout)
    });
    let Some(status) = status else {
        return Ran::CutOff;
    };
    let left = timeout.saturating_sub(started.elapsed()).max(COLLECT_GRACE);
    match stdout.recv_timeout(left) {
        Ok(Ok(stdout)) => Ran::Finished { status, stdout },
        _ => Ran::CutOff,
    }
}

/// Wait for `child` until `timeout` from `started`, then kill its group either way and reap it;
/// its exit status, or `None` when it was cut off.
fn settle(child: &mut Child, started: Instant, timeout: Duration) -> Option<ExitStatus> {
    let pid = child.id();
    // The child is reaped only after its group's kill (`has_exited` leaves it unreaped), so its
    // zombie keeps the group id from being reused before the kill lands.
    loop {
        match has_exited(pid) {
            Some(true) => {
                kill_group(pid);
                return child.wait().ok();
            }
            // `waitid` failed, so the child may be reaped already and its group id free for
            // reuse: no group kill. A child still running goes on to the time limit below.
            None => match child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => {}
                Err(_) => return None,
            },
            Some(false) => {}
        }
        let elapsed = started.elapsed();
        if elapsed < timeout {
            thread::sleep(Duration::from_millis(20).min(timeout - elapsed));
        } else {
            kill_group(pid);
            let _ = child.wait();
            return None;
        }
    }
}

/// Read at most `cap` bytes of `pipe` on a thread, then discard the rest so the writer never blocks.
fn drain(mut pipe: impl Read + Send + 'static, cap: u64) -> mpsc::Receiver<io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe
            .by_ref()
            .take(cap)
            .read_to_end(&mut bytes)
            .and_then(|_| io::copy(&mut pipe, &mut io::sink()));
        let _ = sender.send(result.map(|_| bytes));
    });
    receiver
}

/// SIGKILL to the group the child leads (it was started with `process_group(0)`).
fn kill_group(pid: u32) {
    if let Ok(group) = libc::pid_t::try_from(pid)
        && group > 1
    {
        // SAFETY: killpg has no memory-safety preconditions; the group is the child's own.
        unsafe { libc::killpg(group, libc::SIGKILL) };
    }
}

/// Whether the child `pid` has exited, asked without reaping it (`WNOWAIT`); `None` when
/// `waitid` itself failed.
fn has_exited(pid: u32) -> Option<bool> {
    loop {
        // SAFETY: siginfo_t is plain data; all-zero is valid and waitid fills it.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let flags = libc::WEXITED | libc::WNOWAIT | libc::WNOHANG;
        // SAFETY: info is a valid out pointer for the duration of the call.
        if unsafe { libc::waitid(libc::P_PID, pid as libc::id_t, &mut info, flags) } == 0 {
            // With WNOHANG and nothing to report, the pid is left zero.
            // SAFETY: waitid succeeded, so the pid field is initialised.
            return Some(unsafe { info.si_pid() } != 0);
        }
        if io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return None;
        }
    }
}
