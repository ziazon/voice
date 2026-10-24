use crate::error::Result;
use serde::Serialize;

pub fn to_python_json(value: &impl Serialize) -> Result<String> {
    let pretty = serde_json::to_string_pretty(value)
        .map_err(|error| crate::error::Error::Invalid(error.to_string()))?;
    let mut output = String::with_capacity(pretty.len() + 1);
    for character in pretty.chars() {
        if character < '\u{7f}' {
            output.push(character);
        } else {
            use std::fmt::Write;
            for unit in character.encode_utf16(&mut [0; 2]) {
                write!(output, "\\u{unit:04x}").expect("writing to a String cannot fail");
            }
        }
    }
    output.push('\n');
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn python_json_escapes_unicode_and_keeps_empty_containers() {
        assert_eq!(
            to_python_json(&json!({"a": [], "b": {}, "c": "Zoë 😀\u{7f}"})).unwrap(),
            "{\n  \"a\": [],\n  \"b\": {},\n  \"c\": \"Zo\\u00eb \\ud83d\\ude00\\u007f\"\n}\n"
        );
    }
}
