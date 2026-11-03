//! Characters that must never hide or redirect text shown to a person.
pub fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{00ad}' | '\u{061c}' | '\u{180e}' | '\u{feff}'
            | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}' | '\u{fff9}'..='\u{fffb}')
}

#[cfg(test)]
mod tests {
    use super::hidden;

    #[test]
    fn controls_and_invisible_characters_are_hidden() {
        for c in [
            '\n', '\t', '\u{1b}', '\u{00ad}', '\u{061c}', '\u{180e}', '\u{feff}', '\u{200b}',
            '\u{200f}', '\u{2028}', '\u{202e}', '\u{2060}', '\u{206f}', '\u{fff9}', '\u{fffb}',
        ] {
            assert!(hidden(c), "{c:?}");
        }
    }

    #[test]
    fn ordinary_visible_characters_and_emoji_variation_are_allowed() {
        for c in "é 中 > & <\u{fe0f}".chars() {
            assert!(!hidden(c), "{c:?}");
        }
    }
}
