//! Strict, bounded JSON objects for texts supplied without shell expansion.
use clap::{Command, error::ErrorKind};
use serde::{
    Deserializer,
    de::{
        DeserializeOwned, DeserializeSeed, Error, MapAccess, Visitor,
        value::{MapAccessDeserializer, StringDeserializer},
    },
};
use std::{
    fmt,
    io::{self, Read},
    marker::PhantomData,
};

const LIMIT: u64 = 65536;

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

/// Read one object from stdin, reporting every refusal as a command usage error.
pub fn read<T: DeserializeOwned>(command: &Command) -> T {
    let fail = |message: String| -> ! {
        command
            .clone()
            .error(ErrorKind::ValueValidation, message)
            .exit()
    };
    let mut bytes = Vec::new();
    io::stdin()
        .lock()
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .unwrap_or_else(|error| fail(format!("--stdin needs one JSON object: {error}")));
    if bytes.len() > LIMIT as usize {
        fail("--stdin takes at most 64 KiB".into());
    }
    let text =
        std::str::from_utf8(&bytes).unwrap_or_else(|_| fail("--stdin needs UTF-8 text".into()));
    let mut deserializer = serde_json::Deserializer::from_str(text);
    let value = deserializer
        .deserialize_map(Object(PhantomData))
        .unwrap_or_else(|error| fail(format!("--stdin needs one JSON object: {error}")));
    deserializer
        .end()
        .unwrap_or_else(|error| fail(format!("--stdin needs one JSON object: {error}")));
    value
}
