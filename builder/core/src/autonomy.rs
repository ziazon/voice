use crate::error::{Error, Result};

#[derive(Debug)]
pub struct Autonomy {
    lines: Vec<Line>,
}

#[derive(Debug)]
struct Line {
    raw: String,
    dial: Option<Dial>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dial {
    pub key: String,
    pub principal_decides: bool,
    pub text: String,
}

impl Autonomy {
    pub fn parse(text: &str) -> Result<Self> {
        let lines: Vec<Line> = text
            .split_inclusive('\n')
            .map(|raw| Line {
                raw: raw.to_owned(),
                dial: parse_dial(raw.trim_end_matches(['\r', '\n'])),
            })
            .collect();
        let mut keys = std::collections::HashSet::new();
        for line in &lines {
            if let Some(dial) = &line.dial
                && !keys.insert(&dial.key)
            {
                return Err(Error::Invalid(format!(
                    "autonomy.md: {} appears twice",
                    dial.key
                )));
            }
        }
        Ok(Self { lines })
    }

    pub fn dials(&self) -> Vec<Dial> {
        self.lines
            .iter()
            .filter_map(|line| line.dial.clone())
            .collect()
    }

    pub fn set(&mut self, key: &str, principal_decides: bool) -> Result<()> {
        for line in &mut self.lines {
            if let Some(dial) = &mut line.dial
                && dial.key == key
            {
                line.raw
                    .replace_range(3..4, if principal_decides { "x" } else { " " });
                dial.principal_decides = principal_decides;
                return Ok(());
            }
        }
        Err(Error::Invalid(format!("autonomy.md: unknown key {key}")))
    }

    pub fn render(&self) -> String {
        self.lines.iter().map(|line| line.raw.as_str()).collect()
    }
}

fn parse_dial(line: &str) -> Option<Dial> {
    let (principal_decides, rest) = if let Some(rest) = line.strip_prefix("- [x] `") {
        (true, rest)
    } else {
        (false, line.strip_prefix("- [ ] `")?)
    };
    let (key, text) = rest.split_once("`: ")?;
    if key.is_empty()
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return None;
    }
    Some(Dial {
        key: key.into(),
        principal_decides,
        text: text.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autonomy_template_round_trips_and_lists_dials() {
        let text = include_str!("../../../crew/_template/autonomy.md");
        let autonomy = Autonomy::parse(text).unwrap();
        assert_eq!(autonomy.render(), text);
        let dials = autonomy.dials();
        assert_eq!(dials.len(), 13);
        assert_eq!(
            dials.iter().filter(|dial| dial.principal_decides).count(),
            9
        );
        assert_eq!(
            dials.iter().filter(|dial| !dial.principal_decides).count(),
            4
        );
    }
    #[test]
    fn autonomy_set_changes_only_requested_line() {
        let text = "heading\r\n- [ ] `scope`: choose\r\n- [x] `money`: ask";
        let mut autonomy = Autonomy::parse(text).unwrap();
        let result = autonomy.set("scope", true);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(
            autonomy.render(),
            "heading\r\n- [x] `scope`: choose\r\n- [x] `money`: ask"
        );
        assert!(autonomy.dials()[0].principal_decides);
        assert!(autonomy.set("unknown", false).is_err());
    }

    #[test]
    fn autonomy_refuses_repeated_keys() {
        let result = Autonomy::parse("- [ ] `scope`: one\n- [x] `scope`: two\n");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "autonomy.md: scope appears twice"
        );
    }
}
