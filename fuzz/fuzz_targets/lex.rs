//! The lexer must not panic; its tokens must cover the input without a gap,
//! an empty token, adjacent whitespace tokens, or a split `\r\n`; and its
//! errors must match its `Error` tokens one to one, each inside its token.

#![no_main]

use libfuzzer_sys::fuzz_target;
use nuten_parser::lexer::{Mode, lex};
use nuten_syntax::SyntaxKind;
use rowan::{TextRange, TextSize};

fuzz_target!(|src: &str| {
    let lexed = lex(src, TextRange::up_to(TextSize::of(src)), Mode::block());
    let mut end = TextSize::new(0);
    for token in &lexed.tokens {
        assert_eq!(token.range.start(), end, "gap or overlap in {src:?}");
        assert!(!token.range.is_empty(), "empty token in {src:?}");
        end = token.range.end();
    }
    assert_eq!(end, TextSize::of(src), "tokens end early in {src:?}");

    for pair in lexed.tokens.windows(2) {
        let (left, right) = (pair[0], pair[1]);
        assert!(
            left.kind != SyntaxKind::Whitespace || right.kind != SyntaxKind::Whitespace,
            "adjacent whitespace in {src:?}"
        );
        assert!(
            !(src[left.range].ends_with('\r') && src[right.range].starts_with('\n')),
            "split \\r\\n in {src:?}"
        );
    }

    let error_tokens: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| token.kind == SyntaxKind::Error)
        .collect();
    assert_eq!(
        error_tokens.len(),
        lexed.errors.len(),
        "errors do not match Error tokens in {src:?}"
    );
    for (token, error) in error_tokens.into_iter().zip(&lexed.errors) {
        assert!(
            token.range.contains_range(error.range),
            "misplaced {error:?} in {token:?} of {src:?}"
        );
    }
});
