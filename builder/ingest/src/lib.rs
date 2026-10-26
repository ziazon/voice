pub mod check;
pub mod claudeai;
pub mod corpus;
pub mod dates;
pub mod filters;
pub mod mine;
pub mod privacy;
pub mod private;
pub mod python;

/// Lib tests that hold a state lock or start a child process run one at a time: a child started
/// by another test thread holds a copy of every open descriptor until it execs, a lock included.
#[cfg(test)]
pub(crate) fn serial_test() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL.lock().unwrap_or_else(|poison| poison.into_inner())
}
