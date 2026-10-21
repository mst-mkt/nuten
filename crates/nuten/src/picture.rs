use std::fmt::Write;

/// Not raw, since a control character would move the cursor, color the terminal or reorder text.
pub(crate) fn pictured(text: &str) -> String {
    let mut pictured = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\t' => pictured.push(ch),
            '\u{7f}' => pictured.push('␡'),
            ch if ch < ' ' => pictured.push(
                char::from_u32(0x2400 + u32::from(ch)).expect("control pictures are characters"),
            ),
            // By code point as `less` shows them, since Unicode has no pictures for these.
            '\u{80}'..='\u{9f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => {
                write!(pictured, "<U+{:04X}>", u32::from(ch))
                    .expect("a string takes what is written");
            },
            ch => pictured.push(ch),
        }
    }
    pictured
}

#[cfg(test)]
mod tests {
    use super::pictured;

    #[test]
    fn pictured_draws_c0_controls_and_delete_but_keeps_tab() {
        assert_eq!(
            pictured("a\u{0}\u{1b}[0m\n\u{1f} \u{7f}\tb"),
            "a␀␛[0m␊␟ ␡\tb"
        );
    }

    #[test]
    fn pictured_names_c1_and_bidi_controls_by_code_point() {
        assert_eq!(
            pictured("\u{7e}\u{80}\u{9b}2J\u{9f}\u{a0}"),
            "\u{7e}<U+0080><U+009B>2J<U+009F>\u{a0}"
        );
        assert_eq!(
            pictured("\u{2029}\u{202a}\u{202e}ab\u{202f}\u{2065}\u{2066}\u{2069}\u{206a}"),
            "\u{2029}<U+202A><U+202E>ab\u{202f}\u{2065}<U+2066><U+2069>\u{206a}"
        );
    }
}
