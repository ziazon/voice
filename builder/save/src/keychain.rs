use crate::{secret::Secret, text};
use persona_builder::error::{Error, Result};
use std::sync::Mutex;
use zeroize::Zeroize;

pub trait TokenStore {
    fn load(&self) -> Result<Option<Secret>>;
    fn save(&self, token: &Secret) -> Result<()>;
    fn forget(&self) -> Result<()>;
}
pub const ACCOUNT: &str = "persona-builder";
pub struct Keychain {
    service: String,
}
impl Keychain {
    pub fn github() -> Self {
        Self {
            service: "persona-builder.github".into(),
        }
    }
    pub fn with_service(service: &str) -> Result<Self> {
        if !valid_service(service) {
            return Err(failed());
        }
        Ok(Self {
            service: service.into(),
        })
    }

    #[cfg(target_os = "macos")]
    fn command(&self) -> std::process::Command {
        use std::process::{Command, Stdio};
        let mut command = Command::new("/usr/bin/security");
        command.env_clear().stderr(Stdio::null());
        if let Some(home) = std::env::var_os("HOME") {
            command.env("HOME", home);
        }
        command
    }
}

#[cfg(target_os = "macos")]
impl TokenStore for Keychain {
    fn load(&self) -> Result<Option<Secret>> {
        use std::process::Stdio;
        let output = self
            .command()
            .args([
                "find-generic-password",
                "-s",
                &self.service,
                "-a",
                ACCOUNT,
                "-w",
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(|_| failed())?;
        match output.status.code() {
            Some(0) => {
                let mut value = String::from_utf8(output.stdout).map_err(|_| failed())?;
                if value.ends_with('\n') {
                    value.pop();
                }
                Ok(Some(Secret::new(value)))
            }
            Some(44) => Ok(None),
            _ => Err(failed()),
        }
    }
    fn save(&self, token: &Secret) -> Result<()> {
        use std::{io::Write, process::Stdio};
        let line = store_line(&self.service, token)?;
        let mut child = self
            .command()
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .map_err(|_| failed())?;
        let written = match child.stdin.take() {
            Some(mut stdin) => stdin.write_all(line.expose().as_bytes()),
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(failed());
            }
        };
        let status = child.wait().map_err(|_| failed())?;
        if written.is_err() || !status.success() {
            return Err(failed());
        }
        match self.load()? {
            Some(saved) if saved.expose() == token.expose() => Ok(()),
            _ => Err(failed()),
        }
    }
    fn forget(&self) -> Result<()> {
        use std::process::Stdio;
        let status = self
            .command()
            .args([
                "delete-generic-password",
                "-s",
                &self.service,
                "-a",
                ACCOUNT,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|_| failed())?;
        match status.code() {
            Some(0 | 44) => Ok(()),
            _ => Err(failed()),
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl TokenStore for Keychain {
    fn load(&self) -> Result<Option<Secret>> {
        Err(Error::Invalid(text::MAC_ONLY.into()))
    }
    fn save(&self, _token: &Secret) -> Result<()> {
        Err(Error::Invalid(text::MAC_ONLY.into()))
    }
    fn forget(&self) -> Result<()> {
        Err(Error::Invalid(text::MAC_ONLY.into()))
    }
}

fn failed() -> Error {
    Error::Invalid(text::KEYCHAIN_FAILED.into())
}
fn valid_service(service: &str) -> bool {
    (1..=64).contains(&service.len())
        && service
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b))
}
pub fn storable(token: &Secret) -> bool {
    (1..=1024).contains(&token.expose().len())
        && token
            .expose()
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._~:+/=-".contains(&b))
}
pub fn store_line(service: &str, token: &Secret) -> Result<Secret> {
    if !valid_service(service) || !storable(token) {
        return Err(failed());
    }
    let prefix = "add-generic-password -U -s ";
    let middle = " -a persona-builder -w \"";
    let suffix = "\"\n";
    let mut line = String::with_capacity(
        prefix.len() + service.len() + middle.len() + token.expose().len() + suffix.len(),
    );
    line.push_str(prefix);
    line.push_str(service);
    line.push_str(middle);
    line.push_str(token.expose());
    line.push_str(suffix);
    Ok(Secret::new(line))
}
pub struct MemoryStore {
    token: Mutex<Option<String>>,
}
impl MemoryStore {
    pub fn new() -> Self {
        Self {
            token: Mutex::new(None),
        }
    }
}
impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}
impl TokenStore for MemoryStore {
    fn load(&self) -> Result<Option<Secret>> {
        Ok(self
            .token
            .lock()
            .map_err(|_| failed())?
            .as_ref()
            .map(|value| Secret::new(value.clone())))
    }
    fn save(&self, token: &Secret) -> Result<()> {
        let mut stored = self.token.lock().map_err(|_| failed())?;
        stored.zeroize();
        *stored = Some(token.expose().to_owned());
        Ok(())
    }
    fn forget(&self) -> Result<()> {
        self.token.lock().map_err(|_| failed())?.zeroize();
        Ok(())
    }
}
impl Drop for MemoryStore {
    fn drop(&mut self) {
        self.token
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .zeroize();
    }
}
