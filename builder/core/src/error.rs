use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Invalid(String),
    Exists(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Invalid(message) => f.write_str(message),
            Self::Exists(slug) => write!(f, "{slug} exists"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid(_) | Self::Exists(_) => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn displays_errors_and_exposes_io_source() {
        let invalid = Error::Invalid("member config: invalid".into());
        assert_eq!(invalid.to_string(), "member config: invalid");
        assert!(invalid.source().is_none());
        let io = Error::Io {
            path: "member.json".into(),
            source: std::io::Error::other("read failed"),
        };
        assert_eq!(io.to_string(), "member.json: read failed");
        assert_eq!(io.source().unwrap().to_string(), "read failed");
    }
}
