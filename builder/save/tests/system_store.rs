use persona_builder_keystore::{
    System,
    keyring_core::{
        self, Credential, CredentialStore, Entry, Error,
        api::{CredentialApi, CredentialStoreApi},
        mock,
    },
};
use persona_builder_save::{
    keychain::{ACCOUNT, SystemStore, TokenStore},
    secret::Secret,
    text,
};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

const SERVICE: &str = "persona-builder.test";

fn mock_store() -> Arc<CredentialStore> {
    mock::Store::new().unwrap()
}
fn store_over(store: &Arc<CredentialStore>) -> SystemStore {
    SystemStore::over(SERVICE, System::over(store.clone())).unwrap()
}
fn entry(store: &Arc<CredentialStore>) -> Entry {
    store.build(SERVICE, ACCOUNT, None).unwrap()
}
fn fail_next(store: &Arc<CredentialStore>, error: Error) {
    let entry = entry(store);
    let cred: &mock::Cred = entry.as_any().downcast_ref().unwrap();
    cred.set_error(error);
}
fn platform() -> Error {
    Error::PlatformFailure("invented platform failure".into())
}
fn no_access() -> Error {
    Error::NoStorageAccess("invented locked store".into())
}

#[test]
fn round_trip_replace_and_forget() {
    let backing = mock_store();
    let store = store_over(&backing);
    assert!(store.load().unwrap().is_none());
    store.save(&Secret::new("invented-first".into())).unwrap();
    assert_eq!(store.load().unwrap().unwrap().expose(), "invented-first");
    store.save(&Secret::new("invented-second".into())).unwrap();
    assert_eq!(store.load().unwrap().unwrap().expose(), "invented-second");
    assert_eq!(entry(&backing).get_password().unwrap(), "invented-second");
    store.forget().unwrap();
    assert!(store.load().unwrap().is_none());
    store.forget().unwrap();
}

#[test]
fn errors_on_load_are_the_plain_failure() {
    for error in [
        platform(),
        no_access(),
        Error::BadEncoding(b"invented\xff".to_vec()),
        Error::BadDataFormat(b"invented".to_vec(), "invented".into()),
    ] {
        let backing = mock_store();
        let store = store_over(&backing);
        fail_next(&backing, error);
        assert_eq!(store.load().unwrap_err().to_string(), text::KEYCHAIN_FAILED);
    }
}

#[test]
fn errors_on_save_are_the_plain_failure() {
    for error in [platform(), no_access()] {
        let backing = mock_store();
        let store = store_over(&backing);
        fail_next(&backing, error);
        assert_eq!(
            store
                .save(&Secret::new("invented".into()))
                .unwrap_err()
                .to_string(),
            text::KEYCHAIN_FAILED
        );
        assert!(matches!(
            entry(&backing).get_password(),
            Err(Error::NoEntry)
        ));
    }
}

#[test]
fn errors_on_forget_are_the_plain_failure() {
    for error in [platform(), no_access()] {
        let backing = mock_store();
        let store = store_over(&backing);
        store.save(&Secret::new("invented".into())).unwrap();
        fail_next(&backing, error);
        assert_eq!(
            store.forget().unwrap_err().to_string(),
            text::KEYCHAIN_FAILED
        );
        assert_eq!(entry(&backing).get_password().unwrap(), "invented");
    }
}

#[test]
fn a_token_the_keychain_refuses_never_reaches_the_store() {
    for value in ["", "a b", "a\"b", "a\nb", "é"] {
        let backing = mock_store();
        let store = store_over(&backing);
        assert_eq!(
            store
                .save(&Secret::new(value.into()))
                .unwrap_err()
                .to_string(),
            text::KEYCHAIN_FAILED
        );
        assert!(matches!(
            entry(&backing).get_password(),
            Err(Error::NoEntry)
        ));
    }
}

/// A store that keeps something other than what it was given, so only a read back catches it.
#[derive(Debug, Default)]
struct Garbled(Mutex<Option<Vec<u8>>>);
impl CredentialApi for Garbled {
    fn set_secret(&self, secret: &[u8]) -> keyring_core::Result<()> {
        let mut kept = secret.to_vec();
        kept.push(b'x');
        *self.0.lock().unwrap() = Some(kept);
        Ok(())
    }
    fn get_secret(&self) -> keyring_core::Result<Vec<u8>> {
        self.0.lock().unwrap().clone().ok_or(Error::NoEntry)
    }
    fn delete_credential(&self) -> keyring_core::Result<()> {
        self.0
            .lock()
            .unwrap()
            .take()
            .map(|_| ())
            .ok_or(Error::NoEntry)
    }
    fn get_credential(&self) -> keyring_core::Result<Option<Arc<Credential>>> {
        Ok(None)
    }
    fn get_specifiers(&self) -> Option<(String, String)> {
        Some((SERVICE.into(), ACCOUNT.into()))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
#[derive(Debug, Default)]
struct GarbledStore(Arc<Garbled>);
impl CredentialStoreApi for GarbledStore {
    fn vendor(&self) -> String {
        "garbled test store".into()
    }
    fn id(&self) -> String {
        "garbled".into()
    }
    fn build(
        &self,
        _: &str,
        _: &str,
        _: Option<&HashMap<&str, &str>>,
    ) -> keyring_core::Result<Entry> {
        Ok(Entry::new_with_credential(self.0.clone()))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn a_read_back_that_differs_is_a_failed_save() {
    let store =
        SystemStore::over(SERVICE, System::over(Arc::new(GarbledStore::default()))).unwrap();
    assert_eq!(
        store
            .save(&Secret::new("invented".into()))
            .unwrap_err()
            .to_string(),
        text::KEYCHAIN_FAILED
    );
}

#[test]
fn service_is_checked_as_the_keychain_checks_it() {
    for service in ["", "Persona", "a b", "a_b"] {
        let error = SystemStore::over(service, System::over(mock_store()))
            .err()
            .expect("invalid service");
        assert_eq!(error.to_string(), text::KEYCHAIN_FAILED);
    }
    assert!(SystemStore::over("persona-builder.github", System::over(mock_store())).is_ok());
}

#[cfg(not(any(windows, target_os = "linux")))]
#[test]
fn no_store_is_the_plain_refusal() {
    let error = SystemStore::open(SERVICE).err().expect("no store on a Mac");
    assert_eq!(error.to_string(), text::NO_SAFE_STORE);
}
