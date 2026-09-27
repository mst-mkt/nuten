//! The escapes nu reads in a `"..."` string.

/// `content` with its escapes decoded, or `None` where nu rejects one, or the bytes are no UTF-8.
/// A `\x` escape gives one byte, so `"\xc3\xa9"` is `é` and `"\xff"` alone is no string.
pub(crate) fn unescape(content: &str) -> Option<String> {
    if !content.contains('\\') {
        return Some(content.to_string());
    }
    let mut bytes = content.bytes().enumerate();
    let mut out = Vec::with_capacity(content.len());
    while let Some((at, byte)) = bytes.next() {
        if byte != b'\\' {
            out.push(byte);
            continue;
        }
        let (_, escaped) = bytes.next()?;
        match escaped {
            b'"' | b'\'' | b'\\' | b'/' | b'(' | b')' | b'{' | b'}' | b'$' | b'^' | b'#' | b'|'
            | b'~' => out.push(escaped),
            b'a' => out.push(0x7),
            b'b' => out.push(0x8),
            b'e' => out.push(0x1b),
            b'f' => out.push(0xc),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'0' => out.push(0),
            b'x' => {
                let digits = content
                    .get(at + 2..at + 4)
                    .filter(|digits| digits.bytes().all(|digit| digit.is_ascii_hexdigit()))?;
                out.push(u8::from_str_radix(digits, 16).ok()?);
                bytes.nth(1)?;
            },
            b'u' => {
                let rest = content.get(at + 2..)?.strip_prefix('{')?;
                let digits = &rest[..rest.find('}')?];
                let character = (1..=6)
                    .contains(&digits.len())
                    .then(|| u32::from_str_radix(digits, 16).ok())
                    .flatten()
                    .and_then(char::from_u32)?;
                out.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
                bytes.nth(digits.len() + 1)?;
            },
            _ => return None,
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::unescape;

    #[test]
    fn text_without_backslash_is_itself() {
        assert_eq!(unescape("a b"), Some("a b".to_string()));
        assert_eq!(unescape(""), Some(String::new()));
    }

    #[test]
    fn punctuation_and_letter_escapes_are_decoded() {
        assert_eq!(
            unescape(r#"\"\'\\\/\(\)\{\}\$\^\#\|\~"#),
            Some(r#""'\/(){}$^#|~"#.to_string())
        );
        assert_eq!(
            unescape(r"\a\b\e\f\n\r\t\0"),
            Some("\x07\x08\x1b\x0c\n\r\t\0".to_string())
        );
    }

    #[test]
    fn hex_escape_is_one_byte() {
        assert_eq!(unescape(r"\x41"), Some("A".to_string()));
        assert_eq!(unescape(r"\x41b"), Some("Ab".to_string()));
        assert_eq!(unescape(r"\xc3\xa9"), Some("é".to_string()));
        for text in [r"\xff", r"\x4", r"\x4g", r"\x+4", r"\x"] {
            assert_eq!(unescape(text), None, "{text}");
        }
    }

    #[test]
    fn unicode_escape_is_one_character() {
        assert_eq!(unescape(r"\u{61}"), Some("a".to_string()));
        assert_eq!(unescape(r"\u{1F600}x"), Some("😀x".to_string()));
        assert_eq!(unescape(r"\u{10FFFF}"), Some("\u{10FFFF}".to_string()));
        assert_eq!(unescape(r"\u{000061}"), Some("a".to_string()));
        assert_eq!(unescape(r"\u{+62}"), Some("b".to_string()));
        for text in [
            r"\u{0000061}",
            r"\u{d800}",
            r"\u{110000}",
            r"\u{1234567}",
            r"\u{}",
            r"\u{g}",
            r"\u61",
            r"\u{61",
        ] {
            assert_eq!(unescape(text), None, "{text}");
        }
    }

    #[test]
    fn unknown_or_unfinished_escape_is_rejected() {
        for text in [r"\q", r"\", r"a\", r"\ "] {
            assert_eq!(unescape(text), None, "{text}");
        }
    }
}
