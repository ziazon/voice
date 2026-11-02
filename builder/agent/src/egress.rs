//! One CONNECT proxy per sandboxed Claude Code launch.
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use voice_core::error::{Error, Result};

struct Rules {
    /// Claude Code 2.1.292 uses platform.claude.com/v1/oauth/token and
    /// api.anthropic.com/api/oauth/claude_cli/roles. The sign-in page on claude.com
    /// is opened by the browser, never the child, so claude.com is not in the list.
    hosts: &'static [&'static str],
    resolve: fn(&str) -> io::Result<Vec<IpAddr>>,
    public: fn(IpAddr) -> bool,
    upstream_port: u16,
    header_time: Duration,
    header_bytes: usize,
    connections: usize,
    dial_time: Duration,
    idle: Duration,
}

impl Rules {
    fn claude_code() -> Self {
        Self {
            hosts: &["api.anthropic.com", "platform.claude.com", "claude.ai"],
            resolve: |host| Ok((host, 443).to_socket_addrs()?.map(|a| a.ip()).collect()),
            public,
            upstream_port: 443,
            header_time: Duration::from_secs(10),
            header_bytes: 8 * 1024,
            connections: 16,
            dial_time: Duration::from_secs(10),
            idle: Duration::from_secs(5 * 60),
        }
    }
}

/// Global unicast only, with the special-purpose ranges excluded:
/// https://www.iana.org/assignments/iana-ipv4-special-registry/ and
/// https://www.iana.org/assignments/iana-ipv6-special-registry/.
fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let bits = u32::from(ip);
            let denied = [
                (Ipv4Addr::new(0, 0, 0, 0), 8),
                (Ipv4Addr::new(10, 0, 0, 0), 8),
                (Ipv4Addr::new(100, 64, 0, 0), 10),
                (Ipv4Addr::new(127, 0, 0, 0), 8),
                (Ipv4Addr::new(169, 254, 0, 0), 16),
                (Ipv4Addr::new(172, 16, 0, 0), 12),
                (Ipv4Addr::new(192, 0, 0, 0), 24),
                (Ipv4Addr::new(192, 0, 2, 0), 24),
                (Ipv4Addr::new(192, 88, 99, 0), 24),
                (Ipv4Addr::new(192, 168, 0, 0), 16),
                (Ipv4Addr::new(198, 18, 0, 0), 15),
                (Ipv4Addr::new(198, 51, 100, 0), 24),
                (Ipv4Addr::new(203, 0, 113, 0), 24),
                (Ipv4Addr::new(224, 0, 0, 0), 4),
                (Ipv4Addr::new(240, 0, 0, 0), 4),
            ];
            !denied.into_iter().any(|(network, prefix)| {
                let mask = u32::MAX << (32 - prefix);
                bits & mask == u32::from(network) & mask
            })
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public(IpAddr::V4(ip));
            }
            let bits = u128::from(ip);
            bits >> 125 == 1
                && bits >> 105 != (0x2001_u128 << 112) >> 105
                && bits >> 96 != 0x2001_0db8
                && bits >> 112 != 0x2002
        }
    }
}

/// One running proxy. Dropping it stops it.
pub struct Egress {
    port: u16,
    state: Arc<State>,
    accept: Option<JoinHandle<Vec<JoinHandle<()>>>>,
}

struct State {
    stopped: AtomicBool,
    sockets: Mutex<BTreeMap<usize, Vec<TcpStream>>>,
    #[cfg(test)]
    live: std::sync::atomic::AtomicUsize,
}

impl Egress {
    /// Listens on 127.0.0.1, on a port the system picks, with the rules for Claude Code.
    pub fn start() -> Result<Egress> {
        start_with(Rules::claude_code())
    }

    /// The port to hand to the sandbox (`sandbox::Launch`'s proxy port).
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Closes every socket and joins the proxy's threads. A dial already under way
    /// finishes within its deadline, so this returns within about `dial_time` at worst.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        let Some(accept) = self.accept.take() else {
            return;
        };
        self.state.stopped.store(true, Ordering::SeqCst);
        // Wake blocking accept. The accept thread drops this connection immediately.
        let _ = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port));
        let handlers = accept.join().unwrap_or_default();
        for sockets in self.state.sockets.lock().unwrap().values() {
            for socket in sockets {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
        for handler in handlers {
            let _ = handler.join();
        }
    }
}

impl Drop for Egress {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn start_with(rules: Rules) -> Result<Egress> {
    let error = |source| Error::Io {
        path: PathBuf::from("127.0.0.1"),
        source,
    };
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(error)?;
    let port = listener
        .local_addr()
        .expect("bound TCP listener has an address")
        .port();
    let state = Arc::new(State {
        stopped: AtomicBool::new(false),
        sockets: Mutex::new(BTreeMap::new()),
        #[cfg(test)]
        live: std::sync::atomic::AtomicUsize::new(0),
    });
    let shared = state.clone();
    let accept = thread::Builder::new()
        .spawn(move || accept(listener, rules, shared))
        .map_err(error)?;
    Ok(Egress {
        port,
        state,
        accept: Some(accept),
    })
}

// The guard keeps a slot until both copying directions have ended.
struct Connection {
    id: usize,
    state: Arc<State>,
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Some(sockets) = self.state.sockets.lock().unwrap().remove(&self.id) {
            for socket in sockets {
                let _ = socket.shutdown(Shutdown::Both);
            }
        }
    }
}

// A per-proxy test counter includes accept, handlers and copying threads. DNS is
// deliberately excluded: an overdue resolver owns no socket and is abandoned.
struct Running {
    #[cfg(test)]
    state: Arc<State>,
}
impl Running {
    fn new(state: &Arc<State>) -> Self {
        #[cfg(test)]
        state.live.fetch_add(1, Ordering::SeqCst);
        #[cfg(not(test))]
        let _ = state;
        Self {
            #[cfg(test)]
            state: state.clone(),
        }
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        #[cfg(test)]
        self.state.live.fetch_sub(1, Ordering::SeqCst);
    }
}

fn accept(listener: TcpListener, rules: Rules, state: Arc<State>) -> Vec<JoinHandle<()>> {
    let _running = Running::new(&state);
    let rules = Arc::new(rules);
    let mut handlers: Vec<JoinHandle<()>> = Vec::new();
    for (id, accepted) in listener.incoming().enumerate() {
        let mut child = match accepted {
            Ok(child) => child,
            Err(_) => {
                if state.stopped.load(Ordering::SeqCst) {
                    break;
                }
                continue;
            }
        };
        let accepted = Instant::now();
        if state.stopped.load(Ordering::SeqCst) {
            break;
        }
        // Reap completed handlers during a long launch as well as on stop.
        let mut index = 0;
        while index < handlers.len() {
            if handlers[index].is_finished() {
                let _ = handlers.swap_remove(index).join();
            } else {
                index += 1;
            }
        }
        if state.sockets.lock().unwrap().len() >= rules.connections {
            let _ = child.set_write_timeout(Some(rules.header_time));
            answer(&mut child, 503);
            continue;
        }
        let Ok(registered) = child.try_clone() else {
            continue;
        };
        state.sockets.lock().unwrap().insert(id, vec![registered]);
        let connection = Connection {
            id,
            state: state.clone(),
        };
        let rules = rules.clone();
        if let Ok(handler) = thread::Builder::new().spawn(move || {
            let _running = Running::new(&connection.state);
            handle(child, accepted, rules, &connection);
        }) {
            handlers.push(handler);
        }
        // If spawning fails, dropping its closure releases the socket and slot.
    }
    handlers
}

fn answer(socket: &mut TcpStream, status: u16) {
    let line = match status {
        200 => b"HTTP/1.1 200 Connection Established\r\n\r\n".as_slice(),
        400 => b"HTTP/1.1 400 Bad Request\r\n\r\n",
        403 => b"HTTP/1.1 403 Forbidden\r\n\r\n",
        405 => b"HTTP/1.1 405 Method Not Allowed\r\n\r\n",
        502 => b"HTTP/1.1 502 Bad Gateway\r\n\r\n",
        503 => b"HTTP/1.1 503 Service Unavailable\r\n\r\n",
        _ => unreachable!(),
    };
    let _ = socket.write_all(line);
}

fn header(
    child: &mut TcpStream,
    accepted: Instant,
    rules: &Rules,
) -> std::result::Result<(Vec<u8>, usize), Option<u16>> {
    let deadline = accepted + rules.header_time;
    let mut data = Vec::new();
    loop {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or(None)?;
        child.set_read_timeout(Some(left)).map_err(|_| None)?;
        let mut buf = [0; 4096];
        let n = match child.read(&mut buf) {
            Ok(0) | Err(_) => return Err(None),
            Ok(n) => n,
        };
        if Instant::now() >= deadline {
            return Err(None);
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(end) = data.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let end = end + 4;
            return if end > rules.header_bytes {
                Err(Some(400))
            } else {
                Ok((data, end))
            };
        }
        if data.len() > rules.header_bytes || !line_endings(&data) {
            return Err(Some(400));
        }
    }
}

fn parse(header: &[u8], rules: &Rules) -> std::result::Result<String, u16> {
    if !header.ends_with(b"\r\n\r\n") || !line_endings(header) {
        return Err(400);
    }
    let end = header
        .windows(2)
        .position(|bytes| bytes == b"\r\n")
        .ok_or(400u16)?;
    let line = std::str::from_utf8(&header[..end]).map_err(|_| 400u16)?;
    let words: Vec<_> = line.split(' ').collect();
    if words.len() != 3
        || words.iter().any(|word| {
            word.is_empty()
                || word
                    .bytes()
                    .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        })
    {
        return Err(400);
    }
    if words[0] != "CONNECT" {
        return Err(405);
    }
    if !matches!(words[2], "HTTP/1.0" | "HTTP/1.1") {
        return Err(400);
    }
    let host = words[1].strip_suffix(":443").ok_or(403u16)?;
    if host.is_empty()
        || host.ends_with('.')
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'))
        || !rules
            .hosts
            .iter()
            .any(|allowed| host.eq_ignore_ascii_case(allowed))
    {
        return Err(403);
    }
    Ok(host.to_ascii_lowercase())
}

fn line_endings(header: &[u8]) -> bool {
    header.iter().enumerate().all(|(i, byte)| match byte {
        b'\n' => i > 0 && header[i - 1] == b'\r',
        b'\r' => i + 1 == header.len() || header[i + 1] == b'\n',
        _ => true,
    })
}

fn dial(host: String, rules: &Rules) -> std::result::Result<TcpStream, u16> {
    let deadline = Instant::now() + rules.dial_time;
    let resolve = rules.resolve;
    let (send, receive) = mpsc::channel();
    // A resolver that outlives the deadline is abandoned. It holds no socket,
    // and is the only thread stop() does not join.
    let helper = thread::Builder::new()
        .spawn(move || {
            let _ = send.send(resolve(&host));
        })
        .map_err(|_| 502u16)?;
    let left = deadline
        .checked_duration_since(Instant::now())
        .ok_or(502u16)?;
    let resolved = match receive.recv_timeout(left) {
        Err(mpsc::RecvTimeoutError::Timeout) => return Err(502),
        resolved => {
            let _ = helper.join();
            resolved.map_err(|_| 502u16)?
        }
    };
    let addresses = resolved.map_err(|_| 502u16)?;
    if addresses.is_empty() || addresses.iter().any(|ip| !(rules.public)(*ip)) {
        return Err(403);
    }
    for (index, ip) in addresses.iter().enumerate() {
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or(502u16)?;
        let share = left / (addresses.len() - index) as u32;
        if let Ok(socket) =
            TcpStream::connect_timeout(&SocketAddr::new(*ip, rules.upstream_port), share)
        {
            return Ok(socket);
        }
    }
    Err(502)
}

fn handle(mut child: TcpStream, accepted: Instant, rules: Arc<Rules>, connection: &Connection) {
    let _ = child.set_write_timeout(Some(rules.header_time));
    let request = header(&mut child, accepted, &rules);
    if connection.state.stopped.load(Ordering::SeqCst) {
        return;
    }
    let (data, end) = match request {
        Ok(request) => request,
        Err(status) => {
            if let Some(status) = status {
                answer(&mut child, status);
            }
            return;
        }
    };
    let host = match parse(&data[..end], &rules) {
        Ok(host) => host,
        Err(status) => {
            answer(&mut child, status);
            return;
        }
    };
    let result = dial(host, &rules);
    if connection.state.stopped.load(Ordering::SeqCst) {
        return;
    }
    let mut upstream = match result {
        Ok(socket) => socket,
        Err(status) => {
            answer(&mut child, status);
            return;
        }
    };
    let Ok(registered) = upstream.try_clone() else {
        return;
    };
    {
        let mut sockets = connection.state.sockets.lock().unwrap();
        if connection.state.stopped.load(Ordering::SeqCst) {
            return;
        }
        sockets.get_mut(&connection.id).unwrap().push(registered);
    }
    let _ = upstream.set_write_timeout(Some(rules.idle));
    let _ = child.set_write_timeout(Some(rules.idle));
    answer(&mut child, 200);
    if upstream.write_all(&data[end..]).is_err() {
        return;
    }
    let origin = Instant::now();
    let last = Arc::new(AtomicU64::new(0));
    let (Ok(reader), Ok(writer)) = (upstream.try_clone(), child.try_clone()) else {
        return;
    };
    let shared_last = last.clone();
    let idle = rules.idle;
    let state = connection.state.clone();
    let Ok(copy) = thread::Builder::new().spawn(move || {
        let _running = Running::new(&state);
        tunnel(reader, writer, origin, shared_last, idle);
    }) else {
        return;
    };
    tunnel(child, upstream, origin, last, idle);
    let _ = copy.join();
}

fn tunnel(
    mut source: TcpStream,
    mut destination: TcpStream,
    origin: Instant,
    last: Arc<AtomicU64>,
    idle: Duration,
) {
    let mut buf = [0; 16 * 1024];
    loop {
        let since = origin
            .elapsed()
            .saturating_sub(Duration::from_millis(last.load(Ordering::SeqCst)));
        let left = idle.saturating_sub(since).max(Duration::from_millis(1));
        if source.set_read_timeout(Some(left)).is_err() {
            break;
        }
        match source.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                last.fetch_max(origin.elapsed().as_millis() as u64, Ordering::SeqCst);
                if destination.write_all(&buf[..n]).is_err() {
                    break;
                }
                last.fetch_max(origin.elapsed().as_millis() as u64, Ordering::SeqCst);
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                let since = origin
                    .elapsed()
                    .saturating_sub(Duration::from_millis(last.load(Ordering::SeqCst)));
                if since >= idle {
                    break;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    let _ = source.shutdown(Shutdown::Both);
    let _ = destination.shutdown(Shutdown::Both);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_rules() {
        let rules = Rules::claude_code();
        assert_eq!(
            rules.hosts,
            ["api.anthropic.com", "platform.claude.com", "claude.ai"]
        );
        assert_eq!(rules.upstream_port, 443);
        assert_eq!(rules.header_time, Duration::from_secs(10));
        assert_eq!(rules.header_bytes, 8 * 1024);
        assert_eq!(rules.connections, 16);
        assert_eq!(rules.dial_time, Duration::from_secs(10));
        assert_eq!(rules.idle, Duration::from_secs(5 * 60));
        assert!((rules.public)("1.1.1.1".parse().unwrap()));
        assert!(!(rules.public)("127.0.0.1".parse().unwrap()));
        let _ = rules.resolve;
    }

    #[test]
    fn public_ranges() {
        // The before/start/end/after of every IPv4 range in the brief.
        let ranges = [
            ("0.0.0.0", "0.255.255.255"),
            ("10.0.0.0", "10.255.255.255"),
            ("100.64.0.0", "100.127.255.255"),
            ("127.0.0.0", "127.255.255.255"),
            ("169.254.0.0", "169.254.255.255"),
            ("172.16.0.0", "172.31.255.255"),
            ("192.0.0.0", "192.0.0.255"),
            ("192.0.2.0", "192.0.2.255"),
            ("192.88.99.0", "192.88.99.255"),
            ("192.168.0.0", "192.168.255.255"),
            ("198.18.0.0", "198.19.255.255"),
            ("198.51.100.0", "198.51.100.255"),
            ("203.0.113.0", "203.0.113.255"),
            ("224.0.0.0", "239.255.255.255"),
            ("240.0.0.0", "255.255.255.255"),
        ];
        for (start, end) in ranges {
            let start = u32::from(start.parse::<std::net::Ipv4Addr>().unwrap());
            let end = u32::from(end.parse::<std::net::Ipv4Addr>().unwrap());
            for bits in [start, end] {
                let ip = std::net::Ipv4Addr::from(bits);
                assert!(!public(ip.into()), "{ip}");
                assert!(!public(ip.to_ipv6_mapped().into()), "mapped {ip}");
            }
            // Adjacent multicast/reserved ranges have no public gap.
            for bits in [start.checked_sub(1), end.checked_add(1)]
                .into_iter()
                .flatten()
            {
                let ip = std::net::Ipv4Addr::from(bits);
                let expected = bits < 0xe000_0000;
                assert_eq!(public(ip.into()), expected, "{ip}");
                assert_eq!(public(ip.to_ipv6_mapped().into()), expected, "mapped {ip}");
            }
        }
        for (ip, expected) in [
            ("::", false),
            ("::1", false),
            ("fc00::1", false),
            ("fe80::1", false),
            ("ff00::1", false),
            ("64:ff9b::1", false),
            ("::1.1.1.1", false),
            ("1fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", false),
            ("2000::", true),
            ("2000:ffff:ffff:ffff:ffff:ffff:ffff:ffff", true),
            ("2001::", false),
            ("2001:1ff:ffff:ffff:ffff:ffff:ffff:ffff", false),
            ("2001:200::", true),
            ("2001:db7:ffff:ffff:ffff:ffff:ffff:ffff", true),
            ("2001:db8::", false),
            ("2001:db8:ffff:ffff:ffff:ffff:ffff:ffff", false),
            ("2001:db9::", true),
            ("2001:ffff:ffff:ffff:ffff:ffff:ffff:ffff", true),
            ("2002::", false),
            ("2002:ffff:ffff:ffff:ffff:ffff:ffff:ffff", false),
            ("2003::", true),
            ("3fff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", true),
            ("4000::", false),
            ("1.1.1.1", true),
            ("2606:4700::1111", true),
            ("::ffff:1.1.1.1", true),
            ("::ffff:10.0.0.1", false),
            ("::fffe:ffff:ffff", false),
            ("::1:0:0:0", false),
        ] {
            assert_eq!(public(ip.parse().unwrap()), expected, "{ip}");
        }
    }

    fn loopback(_: &str) -> io::Result<Vec<IpAddr>> {
        Ok(vec!["127.0.0.1".parse().unwrap()])
    }

    fn rules(port: u16) -> Rules {
        Rules {
            resolve: loopback,
            public: |ip| ip.is_loopback(),
            upstream_port: port,
            header_time: Duration::from_millis(800),
            idle: Duration::from_millis(800),
            dial_time: Duration::from_millis(500),
            connections: 2,
            ..Rules::claude_code()
        }
    }

    struct Upstream {
        port: u16,
        seen: std::sync::mpsc::Receiver<()>,
        stop: std::sync::mpsc::Sender<()>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Upstream {
        fn start() -> Self {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let port = listener.local_addr().unwrap().port();
            let (send, seen) = std::sync::mpsc::channel();
            let (stop, stopped) = std::sync::mpsc::channel();
            let thread = std::thread::spawn(move || {
                let mut workers = Vec::new();
                for accepted in listener.incoming() {
                    let mut socket = accepted.unwrap();
                    if stopped.try_recv().is_ok() {
                        break;
                    }
                    send.send(()).unwrap();
                    workers.push(std::thread::spawn(move || {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        socket
                            .set_write_timeout(Some(Duration::from_secs(3)))
                            .unwrap();
                        let mut buf = [0; 1024];
                        while let Ok(n) = std::io::Read::read(&mut socket, &mut buf) {
                            if n == 0 || std::io::Write::write_all(&mut socket, &buf[..n]).is_err()
                            {
                                break;
                            }
                        }
                    }));
                }
                for worker in workers {
                    worker.join().unwrap();
                }
            });
            Self {
                port,
                seen,
                stop,
                thread: Some(thread),
            }
        }
        fn connected(&self) {
            self.seen.recv_timeout(Duration::from_secs(2)).unwrap();
        }
        fn untouched(&self) {
            assert!(matches!(
                self.seen.recv_timeout(Duration::from_millis(300)),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout)
            ));
        }
    }

    impl Drop for Upstream {
        fn drop(&mut self) {
            let _ = self.stop.send(());
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            self.thread.take().unwrap().join().unwrap();
        }
    }

    fn client(proxy: &Egress) -> std::net::TcpStream {
        let socket = std::net::TcpStream::connect(("127.0.0.1", proxy.port())).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        socket
    }

    fn request(socket: &mut std::net::TcpStream, request: &[u8], status: u16) {
        use std::io::{Read, Write};
        socket.write_all(request).unwrap();
        let mut response = Vec::new();
        while !response.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            assert_eq!(
                socket.read(&mut byte).unwrap(),
                1,
                "incomplete response: {response:?}"
            );
            response.push(byte[0]);
        }
        let reason = match status {
            200 => "Connection Established",
            400 => "Bad Request",
            403 => "Forbidden",
            405 => "Method Not Allowed",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            _ => panic!("unexpected status"),
        };
        assert_eq!(
            response,
            format!("HTTP/1.1 {status} {reason}\r\n\r\n").as_bytes()
        );
    }

    fn connect(socket: &mut std::net::TcpStream) {
        request(
            socket,
            b"CONNECT api.anthropic.com:443 HTTP/1.1\r\n\r\n",
            200,
        );
    }

    fn eof(socket: &mut std::net::TcpStream) {
        use std::io::Read;
        assert_eq!(socket.read(&mut [0]).unwrap(), 0);
    }

    #[test]
    fn allowed_tunnels_round_trip() {
        use std::io::{Read, Write};
        let upstream = Upstream::start();
        let proxy = start_with(rules(upstream.port)).unwrap();
        for host in [
            "api.anthropic.com",
            "platform.claude.com",
            "claude.ai",
            "API.Anthropic.COM",
        ] {
            for version in ["HTTP/1.1", "HTTP/1.0"] {
                let mut socket = client(&proxy);
                request(
                    &mut socket,
                    format!("CONNECT {host}:443 {version}\r\nX-Test: ignored\r\n\r\n").as_bytes(),
                    200,
                );
                upstream.connected();
                socket.write_all(b"round trip").unwrap();
                let mut reply = [0; 10];
                socket.read_exact(&mut reply).unwrap();
                assert_eq!(&reply, b"round trip");
                socket.shutdown(std::net::Shutdown::Both).unwrap();
                // Wait for the handler to release its slot before the next request.
                wait_slots(&proxy, 0);
            }
        }
        proxy.stop();
    }

    #[test]
    fn coalesced_bytes_are_first() {
        use std::io::{Read, Write};
        let upstream = Upstream::start();
        let proxy = start_with(rules(upstream.port)).unwrap();
        let mut socket = client(&proxy);
        request(
            &mut socket,
            b"CONNECT api.anthropic.com:443 HTTP/1.1\r\n\r\nfirst",
            200,
        );
        socket.write_all(b"second").unwrap();
        let mut reply = [0; 11];
        socket.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"firstsecond");
        proxy.stop();
    }

    const REFUSED: &[&str] = &[
        "example.com:443",
        "claude.com:443",
        "api.anthropic.com:80",
        "api.anthropic.com.:443",
        "api.anthropic.com:0443",
        "api.anthropic.com:443x",
        "user@api.anthropic.com:443",
        "127.0.0.1:443",
        "[::1]:443",
        "api%2eanthropic.com:443",
        "evil-api.anthropic.com:443",
        "api.anthropic.com.evil:443",
    ];
    const MALFORMED: &[(&str, u16)] = &[
        ("GET http://api.anthropic.com/ HTTP/1.1\r\n\r\n", 405),
        ("connect api.anthropic.com:443 HTTP/1.1\r\n\r\n", 405),
        ("CONNECT api.anthropic.com:443 HTTP/2.0\r\n\r\n", 400),
        ("CONNECT  api.anthropic.com:443 HTTP/1.1\r\n\r\n", 400),
        ("CONNECT api.anthropic.com:443 HTTP/1.1\n\r\n\r\n", 400),
        ("CONNECT api.anthropic.com:443 HTTP/1.1\n\n", 400),
        ("CONNECT api.anthropic.com:443\r\n\r\n", 400),
        (
            "CONNECT api.anthropic.com:443 HTTP/1.1\r\nX: bad\nline\r\n\r\n",
            400,
        ),
    ];

    #[test]
    fn strict_parser() {
        let rules = Rules::claude_code();
        for target in REFUSED {
            let header = format!("CONNECT {target} HTTP/1.1\r\n\r\n");
            assert_eq!(parse(header.as_bytes(), &rules), Err(403), "{target}");
        }
        for (header, code) in MALFORMED {
            assert_eq!(parse(header.as_bytes(), &rules), Err(*code), "{header:?}");
        }
        for host in rules.hosts.iter().copied().chain(["API.Anthropic.COM"]) {
            for version in ["HTTP/1.0", "HTTP/1.1"] {
                let header = format!("CONNECT {host}:443 {version}\r\n\r\n");
                assert_eq!(
                    parse(header.as_bytes(), &rules),
                    Ok(host.to_ascii_lowercase())
                );
            }
        }
    }

    #[test]
    fn forbidden_targets_never_dial() {
        let upstream = Upstream::start();
        let proxy = start_with(rules(upstream.port)).unwrap();
        for target in REFUSED {
            let mut socket = client(&proxy);
            request(
                &mut socket,
                format!("CONNECT {target} HTTP/1.1\r\n\r\n").as_bytes(),
                403,
            );
            eof(&mut socket);
            wait_slots(&proxy, 0);
            upstream.untouched();
        }
        proxy.stop();
    }

    #[test]
    fn malformed_requests_never_dial() {
        let upstream = Upstream::start();
        let proxy = start_with(rules(upstream.port)).unwrap();
        for (header, status) in MALFORMED {
            let mut socket = client(&proxy);
            request(&mut socket, header.as_bytes(), *status);
            eof(&mut socket);
            wait_slots(&proxy, 0);
            upstream.untouched();
        }
        proxy.stop();
    }

    #[test]
    fn header_size_limit() {
        let upstream = Upstream::start();
        let mut short = rules(upstream.port);
        short.header_bytes = 128;
        let proxy = start_with(short).unwrap();
        request(&mut client(&proxy), &[b'x'; 129], 400);
        wait_slots(&proxy, 0);
        let mut header = b"CONNECT api.anthropic.com:443 HTTP/1.1\r\nX: ".to_vec();
        header.resize(124, b'x');
        header.extend_from_slice(b"\r\n\r\n");
        let mut socket = client(&proxy);
        request(&mut socket, &header, 200);
        upstream.connected();
        proxy.stop();
    }

    #[test]
    fn header_deadline_is_absolute() {
        use std::io::{Read, Write};
        let upstream = Upstream::start();
        let mut short = rules(upstream.port);
        short.header_time = Duration::from_millis(800);
        let header_time = short.header_time;
        let proxy = start_with(short).unwrap();
        for trickle in [false, true] {
            let mut socket = client(&proxy);
            let started = std::time::Instant::now();
            socket.write_all(b"CONNECT api.").unwrap();
            socket.set_read_timeout(Some(header_time / 2)).unwrap();
            let writer = if trickle {
                let mut writer = socket.try_clone().unwrap();
                Some(std::thread::spawn(move || {
                    let ticker = std::sync::mpsc::channel::<()>();
                    while ticker.1.recv_timeout(Duration::from_millis(50)).is_err() {
                        if writer.write_all(b"a").is_err() {
                            break;
                        }
                    }
                }))
            } else {
                None
            };
            assert!(matches!(
                socket.read(&mut [0]).unwrap_err().kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ));
            socket
                .set_read_timeout(Some(header_time + Duration::from_millis(500)))
                .unwrap();
            eof(&mut socket);
            assert!(started.elapsed() >= header_time - Duration::from_millis(300));
            assert!(started.elapsed() < header_time + Duration::from_millis(500));
            let _ = socket.shutdown(std::net::Shutdown::Both);
            if let Some(writer) = writer {
                writer.join().unwrap();
            }
        }
        upstream.untouched();
        proxy.stop();
    }

    fn mixed(_: &str) -> io::Result<Vec<IpAddr>> {
        Ok(vec![
            "1.1.1.1".parse().unwrap(),
            "10.0.0.1".parse().unwrap(),
        ])
    }
    fn empty(_: &str) -> io::Result<Vec<IpAddr>> {
        Ok(vec![])
    }
    fn broken(_: &str) -> io::Result<Vec<IpAddr>> {
        Err(io::Error::other("resolver failed"))
    }
    fn slow(_: &str) -> io::Result<Vec<IpAddr>> {
        let (_send, receive) = std::sync::mpsc::channel::<()>();
        let _ = receive.recv_timeout(Duration::from_secs(2));
        loopback("")
    }

    static RESOLVES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    fn once(_: &str) -> io::Result<Vec<IpAddr>> {
        if RESOLVES.fetch_add(1, Ordering::SeqCst) != 0 {
            return Err(io::Error::other("resolved twice"));
        }
        Ok(vec![
            "127.0.0.2".parse().unwrap(),
            "127.0.0.1".parse().unwrap(),
        ])
    }

    #[test]
    fn resolver_result_is_pinned() {
        let upstream = Upstream::start();
        let mut short = rules(upstream.port);
        short.resolve = once;
        RESOLVES.store(0, Ordering::SeqCst);
        let proxy = start_with(short).unwrap();
        let mut socket = client(&proxy);
        connect(&mut socket);
        upstream.connected();
        socket.write_all(b"pinned").unwrap();
        let mut reply = [0; 6];
        socket.read_exact(&mut reply).unwrap();
        assert_eq!(&reply, b"pinned");
        assert_eq!(RESOLVES.load(Ordering::SeqCst), 1);
        proxy.stop();
    }

    #[test]
    fn resolution_rejects_without_dialling() {
        for (resolve, status) in [
            (mixed as fn(&str) -> io::Result<Vec<IpAddr>>, 403),
            (empty, 403),
            (broken, 502),
            (slow, 502),
        ] {
            let mut short = rules(9);
            short.resolve = resolve;
            short.public = public;
            let started = Instant::now();
            assert_eq!(dial("api.anthropic.com".into(), &short).err(), Some(status));
            assert!(started.elapsed() < short.dial_time + Duration::from_millis(500));
        }
    }

    #[test]
    fn resolution_is_checked_and_bounded() {
        let upstream = Upstream::start();
        for (resolve, status) in [
            (mixed as fn(&str) -> io::Result<Vec<IpAddr>>, 403),
            (empty, 403),
            (broken, 502),
            (slow, 502),
        ] {
            let mut short = rules(upstream.port);
            short.resolve = resolve;
            short.public = public;
            let dial_time = short.dial_time;
            let proxy = start_with(short).unwrap();
            let started = std::time::Instant::now();
            let mut socket = client(&proxy);
            request(
                &mut socket,
                b"CONNECT api.anthropic.com:443 HTTP/1.1\r\n\r\n",
                status,
            );
            assert!(started.elapsed() < dial_time + Duration::from_millis(500));
            eof(&mut socket);
            upstream.untouched();
            proxy.stop();
        }
    }

    #[test]
    fn refused_upstream() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let proxy = start_with(rules(port)).unwrap();
        request(
            &mut client(&proxy),
            b"CONNECT api.anthropic.com:443 HTTP/1.1\r\n\r\n",
            502,
        );
        proxy.stop();
    }

    fn wait_slots(proxy: &Egress, count: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        while proxy.state.sockets.lock().unwrap().len() != count {
            assert!(std::time::Instant::now() < deadline, "slots not released");
            std::thread::yield_now();
        }
    }

    #[test]
    fn connection_slots_are_released() {
        use std::io::Write;
        let upstream = Upstream::start();
        let mut short = rules(upstream.port);
        short.header_time = Duration::from_secs(5);
        short.idle = Duration::from_secs(5);
        let proxy = start_with(short).unwrap();
        let mut first = client(&proxy);
        connect(&mut first);
        let mut second = client(&proxy);
        // Half-read requests also consume a slot.
        second.write_all(b"CONNECT ").unwrap();
        wait_slots(&proxy, 2);
        request(&mut client(&proxy), b"", 503);
        first.shutdown(std::net::Shutdown::Both).unwrap();
        wait_slots(&proxy, 1);
        let mut next = client(&proxy);
        connect(&mut next);
        proxy.stop();
    }

    #[test]
    fn idle_tunnel_closes() {
        let upstream = Upstream::start();
        let proxy = start_with(rules(upstream.port)).unwrap();
        let mut socket = client(&proxy);
        connect(&mut socket);
        let started = Instant::now();
        socket
            .set_read_timeout(Some(Duration::from_millis(400)))
            .unwrap();
        assert!(matches!(
            socket.read(&mut [0]).unwrap_err().kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        eof(&mut socket);
        assert!(started.elapsed() < Duration::from_millis(1300));
        proxy.stop();
    }

    #[test]
    fn one_way_traffic_keeps_both_reads_alive() {
        use std::io::{Read, Write};
        let upstream = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let mut short = rules(upstream.local_addr().unwrap().port());
        short.idle = Duration::from_millis(600);
        let proxy = start_with(short).unwrap();
        let mut socket = client(&proxy);
        connect(&mut socket);
        let (mut sender, _) = upstream.accept().unwrap();
        sender
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let started = std::time::Instant::now();
        let ticker = std::sync::mpsc::channel::<()>();
        // Only upstream -> child moves bytes. The child's reader remains idle.
        for _ in 0..12 {
            let _ = ticker.1.recv_timeout(Duration::from_millis(200));
            sender.write_all(b"a").unwrap();
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            assert_eq!(byte, [b'a']);
        }
        assert!(started.elapsed() > Duration::from_millis(2100));
        proxy.stop();
    }

    #[test]
    fn stop_and_drop_join_and_close_everything() {
        use std::io::Write;
        let upstream = Upstream::start();
        for explicit in [true, false] {
            let mut short = rules(upstream.port);
            // Without shutdown, joining would wait much longer than the bound.
            short.header_time = Duration::from_secs(5);
            short.idle = Duration::from_secs(5);
            let mut proxy = start_with(short).unwrap();
            let port = proxy.port();
            let mut tunnel = client(&proxy);
            connect(&mut tunnel);
            upstream.connected();
            let mut half = client(&proxy);
            half.write_all(b"CONNECT ").unwrap();
            wait_slots(&proxy, 2);
            let state = proxy.state.clone();
            let started = std::time::Instant::now();
            if explicit {
                proxy.shutdown();
                assert!(proxy.accept.is_none());
                // The second shutdown and subsequent Drop must be harmless.
                proxy.shutdown();
                proxy.stop();
            } else {
                drop(proxy);
            }
            assert!(started.elapsed() < Duration::from_secs(1));
            eof(&mut tunnel);
            eof(&mut half);
            assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
            assert!(state.sockets.lock().unwrap().is_empty());
            assert_eq!(state.live.load(std::sync::atomic::Ordering::SeqCst), 0);
        }
        let proxy = start_with(rules(upstream.port)).unwrap();
        let port = proxy.port();
        proxy.stop();
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    }

    #[test]
    #[ignore = "needs the network; run attended with the production rules"]
    fn production_network() {
        let proxy = Egress::start().unwrap();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .proxy(Some(
                ureq::Proxy::new(&format!("http://127.0.0.1:{}", proxy.port())).unwrap(),
            ))
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .into();
        let response = agent.get("https://api.anthropic.com/").call().unwrap();
        assert!((100..600).contains(&response.status().as_u16()));
        assert!(agent.get("https://example.com/").call().is_err());
        proxy.stop();
    }
}
