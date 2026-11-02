#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::{
    fs::{self, File},
    io::{self, Write},
    net::TcpStream,
    sync::{Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
use voice_agent::egress::Egress;

// The descriptor limit is process-wide, so these tests must run one at a time.
static LIMITS: Mutex<()> = Mutex::new(());

struct Limit(libc::rlimit);

impl Limit {
    fn current() -> Self {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: limit points to an initialized, writable rlimit.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) },
            0
        );
        Self(limit)
    }

    fn set(limit: &libc::rlimit) {
        // SAFETY: limit points to a valid rlimit for the duration of the call.
        assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, limit) }, 0);
    }
}

impl Drop for Limit {
    fn drop(&mut self) {
        Self::set(&self.0);
    }
}

fn descriptors() -> usize {
    let directory = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };
    fs::read_dir(directory).unwrap().count()
}

#[test]
fn stop_with_exhausted_descriptors() {
    let _limits = LIMITS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let proxy = Egress::start().unwrap();
    let before = descriptors();
    let mut clients = Vec::new();
    for _ in 0..4 {
        let mut client = TcpStream::connect(("127.0.0.1", proxy.port())).unwrap();
        client.write_all(b"CONNECT api.").unwrap();
        clients.push(client);
    }
    // Each client adds its own descriptor and the proxy's accepted socket and clone.
    let deadline = Instant::now() + Duration::from_secs(2);
    while descriptors() < before + 12 {
        assert!(Instant::now() < deadline, "clients were not accepted");
        thread::yield_now();
    }
    let limit = Limit::current();
    Limit::set(&libc::rlimit {
        rlim_cur: (descriptors() as libc::rlim_t + 4).min(limit.0.rlim_cur),
        rlim_max: limit.0.rlim_max,
    });
    let mut fillers = Vec::new();
    loop {
        match File::open("/dev/null") {
            Ok(file) => fillers.push(file),
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(libc::EMFILE));
                break;
            }
        }
    }
    let (send, receive) = mpsc::channel();
    let stop = thread::spawn(move || {
        proxy.stop();
        let _ = send.send(());
    });
    let result = receive.recv_timeout(Duration::from_secs(3));
    drop(fillers);
    drop(limit);
    assert_eq!(result, Ok(()), "stop hung with exhausted descriptors");
    stop.join().unwrap();
    for mut client in clients {
        use io::Read;
        client
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        assert_eq!(client.read(&mut [0]).unwrap(), 0);
    }
}

#[test]
fn stop_with_no_connections_and_no_descriptors() {
    let _limits = LIMITS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let proxy = Egress::start().unwrap();
    let limit = Limit::current();
    Limit::set(&libc::rlimit {
        rlim_cur: (descriptors() as libc::rlim_t + 4).min(limit.0.rlim_cur),
        rlim_max: limit.0.rlim_max,
    });
    let mut fillers = Vec::new();
    loop {
        match File::open("/dev/null") {
            Ok(file) => fillers.push(file),
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(libc::EMFILE));
                break;
            }
        }
    }
    let (send, receive) = mpsc::channel();
    let stop = thread::spawn(move || {
        proxy.stop();
        let _ = send.send(());
    });
    let result = receive.recv_timeout(Duration::from_secs(3));
    drop(fillers);
    drop(limit);
    assert_eq!(
        result,
        Ok(()),
        "stop hung with no connections or descriptors"
    );
    stop.join().unwrap();
}
