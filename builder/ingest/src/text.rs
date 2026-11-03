//! Characters that must never hide or redirect text shown to a person.
pub fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c,
            '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}' | '\u{2066}'..='\u{206f}' | '\u{feff}')
}
