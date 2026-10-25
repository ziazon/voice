//! Characters that can hide content from the build's text checks.

/// A character a point never holds: control characters, line and paragraph separators, the
/// invisible ones that could split a word past the shingle check or the filters, and `<`, which
/// would start hidden markup in the Markdown the assistant reads.
pub fn hidden(c: char) -> bool {
    c.is_control()
        || c == '<'
        || matches!(c, '\u{00AD}' | '\u{061C}' | '\u{180E}' | '\u{FEFF}')
        || ('\u{200B}'..='\u{200F}').contains(&c)
        || ('\u{2028}'..='\u{202E}').contains(&c)
        || ('\u{2060}'..='\u{206F}').contains(&c)
        || ('\u{FFF9}'..='\u{FFFB}').contains(&c)
        || matches!(
            c,
            '\u{034F}' | '\u{115F}' | '\u{1160}' | '\u{3164}' | '\u{FFA0}'
        )
        || ('\u{FE00}'..='\u{FE0F}').contains(&c)
        || ('\u{13430}'..='\u{1343F}').contains(&c)
        || ('\u{1BCA0}'..='\u{1BCA3}').contains(&c)
        || ('\u{1D173}'..='\u{1D17A}').contains(&c)
        || ('\u{E0000}'..='\u{E007F}').contains(&c)
        || ('\u{E0100}'..='\u{E01EF}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::hidden;

    #[test]
    fn ordinary_visible_characters_are_allowed() {
        for c in "They keep it short. é 中 > &".chars() {
            assert!(!hidden(c), "{c:?}");
        }
    }

    #[test]
    fn controls_markup_and_invisible_characters_are_hidden() {
        for c in [
            '\n',
            '\t',
            '\u{1b}',
            '<',
            '\u{00AD}',
            '\u{061C}',
            '\u{180E}',
            '\u{FEFF}',
            '\u{200B}',
            '\u{200F}',
            '\u{2028}',
            '\u{202E}',
            '\u{2060}',
            '\u{206F}',
            '\u{FFF9}',
            '\u{FFFB}',
            '\u{034F}',
            '\u{115F}',
            '\u{1160}',
            '\u{3164}',
            '\u{FFA0}',
            '\u{FE00}',
            '\u{FE0F}',
            '\u{13430}',
            '\u{1343F}',
            '\u{1BCA0}',
            '\u{1BCA3}',
            '\u{1D173}',
            '\u{1D17A}',
            '\u{E0000}',
            '\u{E0061}',
            '\u{E0062}',
            '\u{E007F}',
            '\u{E0100}',
            '\u{E01EF}',
        ] {
            assert!(hidden(c), "{c:?}");
        }
    }
}
