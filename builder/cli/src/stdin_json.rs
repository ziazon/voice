//! Strict, bounded JSON objects for texts supplied without shell expansion.
use clap::{Command, error::ErrorKind};
use serde::de::DeserializeOwned;
use voice_api::json_object::{ObjectError, read_object};

const LIMIT: u64 = 65536;

/// Read one object from stdin, reporting every refusal as a command usage error.
pub fn read<T: DeserializeOwned>(command: &Command) -> T {
    read_object(&mut std::io::stdin().lock(), LIMIT).unwrap_or_else(|error| {
        let message = match error {
            ObjectError::Read(error) | ObjectError::Json(error) => {
                format!("--stdin needs one JSON object: {error}")
            }
            ObjectError::TooLarge => "--stdin takes at most 64 KiB".into(),
            ObjectError::NotUtf8 => "--stdin needs UTF-8 text".into(),
        };
        command
            .clone()
            .error(ErrorKind::ValueValidation, message)
            .exit()
    })
}
