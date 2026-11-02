//! Strict, bounded JSON objects shared by Voice's commands.
use serde::{
    Deserializer,
    de::{
        DeserializeOwned, DeserializeSeed, Error, MapAccess, Visitor,
        value::{MapAccessDeserializer, StringDeserializer},
    },
};
use std::{fmt, io::Read, marker::PhantomData};

#[derive(Debug)]
pub enum ObjectError {
    Read(String),
    TooLarge,
    NotUtf8,
    Json(String),
}

// Keep the key beside value errors without parsing into a map, which would hide duplicates.
struct Fields<A> {
    map: A,
    key: String,
}
impl<'de, A: MapAccess<'de>> MapAccess<'de> for Fields<A> {
    type Error = A::Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, A::Error> {
        let Some(key) = self.map.next_key::<String>()? else {
            return Ok(None);
        };
        self.key = key.clone();
        seed.deserialize(StringDeserializer::<A::Error>::new(key))
            .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, A::Error> {
        self.map
            .next_value_seed(seed)
            .map_err(|error| A::Error::custom(format!("{}: {error}", self.key)))
    }
}

struct Object<T>(PhantomData<T>);
impl<'de, T: DeserializeOwned> Visitor<'de> for Object<T> {
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("one JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
        T::deserialize(MapAccessDeserializer::new(Fields {
            map,
            key: String::new(),
        }))
    }
}

/// Read one JSON object, at most `limit` bytes, refusing duplicate and unknown keys as `T` does.
pub fn read_object<T: DeserializeOwned>(
    input: &mut dyn Read,
    limit: u64,
) -> Result<T, ObjectError> {
    let mut bytes = Vec::new();
    input
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| ObjectError::Read(error.to_string()))?;
    if bytes.len() as u64 > limit {
        return Err(ObjectError::TooLarge);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| ObjectError::NotUtf8)?;
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = deserializer
        .deserialize_map(Object(PhantomData))
        .map_err(|error| ObjectError::Json(error.to_string()))?;
    deserializer
        .end()
        .map_err(|error| ObjectError::Json(error.to_string()))?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::io::{self, Cursor};

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Request {
        text: String,
    }

    #[test]
    fn reads_one_object_at_the_exact_byte_limit() {
        let bytes = " {\"text\":\"café\"} ".as_bytes();
        assert_eq!(
            read_object::<Request>(&mut Cursor::new(bytes), bytes.len() as u64).unwrap(),
            Request {
                text: "café".into()
            }
        );
    }

    #[test]
    fn refuses_every_non_object_or_non_strict_request() {
        for bytes in [
            "",
            "null",
            "[]",
            "42",
            "\"text\"",
            "{}",
            "{",
            r#"{"text":"x","text":"y"}"#,
            r#"{"text":"x","unknown":true}"#,
            r#"{"text":4}"#,
            r#"{"text":"x"} {}"#,
            r#"{"text":"\ud800"}"#,
        ] {
            assert!(
                matches!(
                    read_object::<Request>(&mut Cursor::new(bytes), 1024),
                    Err(ObjectError::Json(_))
                ),
                "accepted or misclassified {bytes:?}"
            );
        }
    }

    #[test]
    fn refuses_invalid_utf8() {
        assert!(matches!(
            read_object::<Request>(&mut Cursor::new([0xff]), 1024),
            Err(ObjectError::NotUtf8)
        ));
    }

    #[test]
    fn reads_at_most_limit_plus_one_bytes() {
        let mut input = Cursor::new(vec![b' '; 4096]);
        assert!(matches!(
            read_object::<Request>(&mut input, 1024),
            Err(ObjectError::TooLarge)
        ));
        assert_eq!(input.position(), 1025);
    }

    #[test]
    fn preserves_read_errors() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _bytes: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("read failed"))
            }
        }
        assert!(matches!(read_object::<Request>(&mut Broken, 1024),
            Err(ObjectError::Read(message)) if message == "read failed"));
    }
}
