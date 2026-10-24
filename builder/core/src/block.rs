use crate::error::{Error, Result};

const MARKER: &str = "<!-- persona-builder:";

pub(crate) fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || id.starts_with('-')
    {
        return Err(Error::Invalid(
            "Use 1 to 64 lower-case letters, digits or -, starting with a letter or digit".into(),
        ));
    }
    Ok(())
}

pub fn find(text: &str, id: &str) -> Result<Option<(usize, usize)>> {
    validate_id(id)?;
    let begin = format!("{MARKER}begin {id} -->");
    let end = format!("{MARKER}end {id} -->");
    let mut start = None;
    let mut found = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let marker = line.trim();
        if marker == begin {
            if start.is_some() || found.is_some() {
                return Err(Error::Invalid(format!(
                    "Persona Builder block {id} appears twice; repair its markers"
                )));
            }
            start = Some(offset);
        } else if marker == end {
            let from = start.take().ok_or_else(|| {
                Error::Invalid(format!(
                    "Persona Builder block {id} has no begin; repair its markers"
                ))
            })?;
            found = Some((from, offset + line.len()));
        } else if start.is_some() && line.contains(MARKER) {
            return Err(Error::Invalid(format!(
                "Persona Builder block {id} contains another marker; repair its markers"
            )));
        }
        offset += line.len();
    }
    if start.is_some() {
        return Err(Error::Invalid(format!(
            "Persona Builder block {id} has no end; repair its markers"
        )));
    }
    Ok(found)
}

fn line_end(text: &str) -> &str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

pub fn upsert(text: &str, id: &str, body: &str) -> Result<String> {
    let found = find(text, id)?;
    if body.contains(MARKER) {
        return Err(Error::Invalid(
            "Remove Persona Builder markers from the block body".into(),
        ));
    }
    let eol = line_end(found.map_or(text, |(start, end)| &text[start..end]));
    let body = body.strip_suffix('\n').unwrap_or(body);
    let body = body
        .split('\n')
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>()
        .join(eol);
    let block = format!("{MARKER}begin {id} -->{eol}{body}{eol}{MARKER}end {id} -->{eol}");
    Ok(match found {
        Some((start, end)) => format!("{}{block}{}", &text[..start], &text[end..]),
        None if text.is_empty() => block,
        None => format!("{text}{eol}{block}"),
    })
}

pub fn remove(text: &str, id: &str) -> Result<String> {
    let Some((mut start, end)) = find(text, id)? else {
        return Ok(text.into());
    };
    let eol = line_end(&text[start..end]);
    if end == text.len() && text[..start].ends_with(eol) {
        start -= eol.len();
    }
    Ok(format!("{}{}", &text[..start], &text[end..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_round_trips_every_line_ending() {
        for original in ["", "notes", "notes\n", "notes\r\n"] {
            let eol = if original.contains("\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let prefix = if original.is_empty() {
                String::new()
            } else {
                format!("{original}{eol}")
            };
            let expected = format!(
                "{prefix}<!-- persona-builder:begin ari -->{eol}body{eol}<!-- persona-builder:end ari -->{eol}"
            );
            let updated = upsert(original, "ari", "body\r\n").unwrap();
            assert_eq!(updated, expected, "original: {original:?}");
            assert_eq!(remove(&updated, "ari").unwrap(), original);
        }
    }

    #[test]
    fn replaces_in_place_and_keeps_block_line_ends() {
        let original = "before\n  <!-- persona-builder:begin ari -->\r\nold\r\n<!-- persona-builder:end ari -->\r\nafter\n";
        assert_eq!(
            upsert(original, "ari", "new\n").unwrap(),
            "before\n<!-- persona-builder:begin ari -->\r\nnew\r\n<!-- persona-builder:end ari -->\r\nafter\n"
        );
        assert_eq!(remove(original, "ari").unwrap(), "before\nafter\n");
        assert_eq!(remove("notes", "ari").unwrap(), "notes");
        assert!(find(original, "ari").unwrap().is_some());
    }

    #[test]
    fn refuses_invalid_ids_bodies_and_markers() {
        for id in ["", "-ari", "Ari", "ari_", "a/b", &"a".repeat(65)] {
            assert!(matches!(find("", id), Err(Error::Invalid(_))), "{id}");
        }
        assert!(upsert("", "ari", "<!-- persona-builder:begin other -->").is_err());
        let begin = "<!-- persona-builder:begin ari -->\n";
        let end = "<!-- persona-builder:end ari -->\n";
        for text in [
            begin.to_owned(),
            end.to_owned(),
            format!("{begin}{end}{begin}{end}"),
            format!("{begin}<!-- persona-builder:begin mo-lee -->\n{end}"),
            format!("{begin}<!-- persona-builder:odd -->\n{end}"),
        ] {
            assert!(find(&text, "ari").is_err(), "{text}");
        }
    }
}
