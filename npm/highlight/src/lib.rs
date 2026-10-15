use nuten_highlight::TokenType;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = TokenType)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenTypeName {
    Comment = "comment",
    Keyword = "keyword",
    Function = "function",
    Variable = "variable",
    Parameter = "parameter",
    Flag = "flag",
    String = "string",
    Number = "number",
    Operator = "operator",
    Type = "type",
    Property = "property",
}

impl From<TokenType> for TokenTypeName {
    fn from(token_type: TokenType) -> Self {
        match token_type {
            TokenType::Comment => TokenTypeName::Comment,
            TokenType::Keyword => TokenTypeName::Keyword,
            TokenType::Function => TokenTypeName::Function,
            TokenType::Variable => TokenTypeName::Variable,
            TokenType::Parameter => TokenTypeName::Parameter,
            TokenType::Flag => TokenTypeName::Flag,
            TokenType::String => TokenTypeName::String,
            TokenType::Number => TokenTypeName::Number,
            TokenType::Operator => TokenTypeName::Operator,
            TokenType::Type => TokenTypeName::Type,
            TokenType::Property => TokenTypeName::Property,
        }
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highlight {
    pub start: usize,
    pub end: usize,
    #[wasm_bindgen(js_name = type)]
    pub token_type: TokenTypeName,
}

#[wasm_bindgen]
pub fn highlight(source: &str) -> Vec<Highlight> {
    let root = nuten_parser::parse(source).syntax();
    let mut utf16 = Utf16Offsets::new(source);
    nuten_highlight::highlight(&root)
        .into_iter()
        .map(|(range, token_type)| Highlight {
            start: utf16.at(range.start().into()),
            end: utf16.at(range.end().into()),
            token_type: token_type.into(),
        })
        .collect()
}

struct Utf16Offsets<'a> {
    source: &'a str,
    offset: usize,
    units: usize,
}

impl<'a> Utf16Offsets<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            units: 0,
        }
    }

    fn at(&mut self, offset: usize) -> usize {
        self.units += self.source[self.offset..offset]
            .chars()
            .map(char::len_utf16)
            .sum::<usize>();
        self.offset = offset;
        self.units
    }
}

#[cfg(test)]
mod tests {
    use super::{Highlight, TokenType, TokenTypeName, highlight};

    #[test]
    fn token_type_name_is_name_in_nuten_highlight() {
        for token_type in TokenType::ALL {
            assert_eq!(TokenTypeName::from(token_type).to_str(), token_type.name());
        }
    }

    #[test]
    fn highlight_counts_utf16_code_units() {
        let at = |start, end, token_type| Highlight {
            start,
            end,
            token_type,
        };
        assert_eq!(
            highlight("echo 'é' '🧶'"),
            [
                at(0, 4, TokenTypeName::Function),
                at(5, 8, TokenTypeName::String),
                at(9, 13, TokenTypeName::String),
            ]
        );
    }
}
