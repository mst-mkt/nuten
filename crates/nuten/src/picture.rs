/// Not raw, since a control character would move the cursor or color the terminal.
pub(crate) fn pictured(text: &str) -> String {
    let mut pictured = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\t' => pictured.push(ch),
            '\u{7f}' => pictured.push('␡'),
            ch if ch < ' ' => pictured.push(
                char::from_u32(0x2400 + u32::from(ch)).expect("control pictures are characters"),
            ),
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
}
