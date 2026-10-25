//! The computer's own password store off a Mac: Windows' Credential Manager, or the Secret
//! Service on Linux (GNOME Keyring, KDE Wallet), through `keyring-core`.
//!
//! Kept apart from the save crate so `cargo check --target` runs for Windows and Linux from a Mac:
//! the save crate's TLS stack (`ring`) builds C code that needs the target's own system headers.
//! The macOS Keychain path stays in the save crate.

pub use keyring_core;
use keyring_core::{CredentialStore, Entry, Result};
use std::{collections::HashMap, sync::Arc};

/// This computer has no password store Persona Builder can use: no Secret Service is running on
/// Linux, or the OS is neither Windows nor Linux.
#[derive(Debug, PartialEq, Eq)]
pub struct NoStore;

pub struct System {
    store: Arc<CredentialStore>,
    modifiers: &'static [(&'static str, &'static str)],
}

impl System {
    /// Windows: `CredWriteW`/`CredReadW` generic credentials (windows-native-keyring-store).
    #[cfg(windows)]
    pub fn open() -> std::result::Result<Self, NoStore> {
        let store = windows_native_keyring_store::Store::new().map_err(|_| NoStore)?;
        // `Enterprise`, the store's default, roams with a domain profile to the person's other
        // computers; `Local` (CRED_PERSIST_LOCAL_MACHINE) keeps the token on this one.
        Ok(Self {
            store,
            modifiers: &[("persistence", "Local")],
        })
    }

    /// Linux: the Secret Service over D-Bus, in the default (login) collection. `Store::new`
    /// opens the session, so it fails here when no Secret Service runs (a headless machine).
    #[cfg(target_os = "linux")]
    pub fn open() -> std::result::Result<Self, NoStore> {
        let store = zbus_secret_service_keyring_store::Store::new().map_err(|_| NoStore)?;
        Ok(Self {
            store,
            modifiers: &[("label", "Persona Builder: GitHub sign-in")],
        })
    }

    #[cfg(not(any(windows, target_os = "linux")))]
    pub fn open() -> std::result::Result<Self, NoStore> {
        Err(NoStore)
    }

    /// Any store, built with no modifiers (the mock store refuses them).
    pub fn over(store: Arc<CredentialStore>) -> Self {
        Self {
            store,
            modifiers: &[],
        }
    }

    pub fn entry(&self, service: &str, user: &str) -> Result<Entry> {
        let modifiers: HashMap<&str, &str> = self.modifiers.iter().copied().collect();
        self.store.build(service, user, Some(&modifiers))
    }
}
