use voice_keystore::{
    System,
    keyring_core::{Error, mock},
};

#[cfg(not(any(windows, target_os = "linux")))]
#[test]
fn no_system_store_off_windows_and_linux() {
    assert!(System::open().is_err());
}

#[test]
fn entry_over_a_given_store_round_trips() {
    let system = System::over(mock::Store::new().unwrap());
    let entry = system.entry("voice.test", "voice").unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)));
    entry.set_password("invented-token").unwrap();
    let again = system.entry("voice.test", "voice").unwrap();
    assert_eq!(again.get_password().unwrap(), "invented-token");
    again.delete_credential().unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)));
}

#[test]
fn entries_are_kept_apart_by_service() {
    let system = System::over(mock::Store::new().unwrap());
    system
        .entry("voice.one", "voice")
        .unwrap()
        .set_password("invented-one")
        .unwrap();
    let other = system.entry("voice.two", "voice").unwrap();
    assert!(matches!(other.get_password(), Err(Error::NoEntry)));
}

#[cfg(any(windows, target_os = "linux"))]
#[test]
#[ignore = "writes to this computer's live password store; run by hand on a desktop"]
fn live_system_round_trip() {
    let system = System::open().expect("a password store on this computer");
    let entry = system.entry("voice.test", "voice").unwrap();
    entry.set_password("invented-live-test").unwrap();
    assert_eq!(entry.get_password().unwrap(), "invented-live-test");
    entry.delete_credential().unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)));
}
