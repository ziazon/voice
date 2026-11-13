//! The public computer label stored in a save's commit trailer.
pub fn valid_computer_name(value: &str) -> bool {
    if value != value.trim() || value.chars().any(char::is_control) {
        return false;
    }
    (1..=64).contains(&value.chars().count())
        && !value
            .chars()
            .any(|c| c.is_control() || "@<>{}[]()#\\`\"$".contains(c))
        && !value.contains(['\n', '\r'])
}
pub fn name() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::{Command, Stdio};
        if let Ok(output) = Command::new("/usr/sbin/scutil")
            .args(["--get", "ComputerName"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
            && output.status.success()
            && let Ok(value) = String::from_utf8(output.stdout)
        {
            let value = value.trim();
            if valid_computer_name(value) {
                return Some(value.to_owned());
            }
        }
    }
    let mut buffer = [0u8; 256];
    // SAFETY: the buffer is writable for its declared length.
    if unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) } != 0 {
        return None;
    }
    let length = buffer.iter().position(|b| *b == 0)?;
    let value = std::str::from_utf8(&buffer[..length]).ok()?.trim();
    let value = value.strip_suffix(".local").unwrap_or(value);
    valid_computer_name(value).then(|| value.to_owned())
}
