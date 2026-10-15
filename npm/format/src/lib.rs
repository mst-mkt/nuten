use std::{fmt::Display, ops::RangeInclusive};

use nuten_format::{Commas, Config, FormatError, IndentStyle, LineEnding};
use nuten_line_index::located;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(getter_with_clone)]
#[derive(Debug)]
pub struct ParseError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[wasm_bindgen(js_name = IndentStyle)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyleName {
    Space = "space",
    Tab = "tab",
}

impl From<IndentStyle> for IndentStyleName {
    fn from(indent_style: IndentStyle) -> Self {
        match indent_style {
            IndentStyle::Space => IndentStyleName::Space,
            IndentStyle::Tab => IndentStyleName::Tab,
        }
    }
}

#[wasm_bindgen(js_name = LineEnding)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEndingName {
    Lf = "lf",
    CrLf = "crlf",
    Auto = "auto",
}

impl From<LineEnding> for LineEndingName {
    fn from(line_ending: LineEnding) -> Self {
        match line_ending {
            LineEnding::Lf => LineEndingName::Lf,
            LineEnding::CrLf => LineEndingName::CrLf,
            LineEnding::Auto => LineEndingName::Auto,
        }
    }
}

#[wasm_bindgen(js_name = Commas)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommasName {
    None = "none",
    Inline = "inline",
    Always = "always",
    Trailing = "trailing",
}

impl From<Commas> for CommasName {
    fn from(commas: Commas) -> Self {
        match commas {
            Commas::None => CommasName::None,
            Commas::Inline => CommasName::Inline,
            Commas::Always => CommasName::Always,
            Commas::Trailing => CommasName::Trailing,
        }
    }
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct DefaultOptions {
    #[wasm_bindgen(js_name = lineWidth)]
    pub line_width: u32,
    #[wasm_bindgen(js_name = indentStyle)]
    pub indent_style: IndentStyleName,
    #[wasm_bindgen(js_name = indentWidth)]
    pub indent_width: u8,
    #[wasm_bindgen(js_name = lineEnding)]
    pub line_ending: LineEndingName,
    pub commas: CommasName,
}

#[wasm_bindgen(js_name = defaultOptions)]
pub fn default_options() -> DefaultOptions {
    let defaults = Config::default();
    DefaultOptions {
        line_width: defaults.line_width,
        indent_style: defaults.indent_style.into(),
        indent_width: defaults.indent_width,
        line_ending: defaults.line_ending.into(),
        commas: defaults.commas.into(),
    }
}

#[wasm_bindgen]
pub fn format(
    source: &str,
    line_width: &JsValue,
    indent_style: &JsValue,
    indent_width: &JsValue,
    line_ending: &JsValue,
    commas: &JsValue,
) -> Result<String, JsValue> {
    let config = config_of(line_width, indent_style, indent_width, line_ending, commas)?;
    nuten_format::format(source, &config).map_err(|error| parse_errors(source, &error).into())
}

fn parse_errors(source: &str, error: &FormatError) -> Vec<ParseError> {
    located(
        source,
        error
            .errors
            .iter()
            .map(|error| (error.range.start().into(), error.kind)),
    )
    .into_iter()
    .map(|(at, kind)| ParseError {
        line: at.line,
        column: at.column,
        message: kind.to_string(),
    })
    .collect()
}

fn config_of(
    line_width: &JsValue,
    indent_style: &JsValue,
    indent_width: &JsValue,
    line_ending: &JsValue,
    commas: &JsValue,
) -> Result<Config, String> {
    let defaults = Config::default();
    Ok(Config {
        line_width: integer_option(
            "lineWidth",
            line_width,
            &Config::LINE_WIDTHS,
            defaults.line_width,
        )?,
        indent_style: choice_option(
            "indentStyle",
            indent_style,
            &IndentStyle::ALL,
            IndentStyle::name,
            defaults.indent_style,
        )?,
        indent_width: integer_option(
            "indentWidth",
            indent_width,
            &Config::INDENT_WIDTHS,
            defaults.indent_width,
        )?,
        line_ending: choice_option(
            "lineEnding",
            line_ending,
            &LineEnding::ALL,
            LineEnding::name,
            defaults.line_ending,
        )?,
        commas: choice_option(
            "commas",
            commas,
            &Commas::ALL,
            Commas::name,
            defaults.commas,
        )?,
    })
}

fn integer_option<T>(
    key: &str,
    value: &JsValue,
    range: &RangeInclusive<T>,
    default: T,
) -> Result<T, String>
where
    T: Copy + Display + Into<f64> + TryFrom<i64>,
{
    if value.is_undefined() {
        Ok(default)
    } else {
        integer(key, value.as_f64(), range)
    }
}

fn choice_option<T: Copy>(
    key: &str,
    value: &JsValue,
    all: &[T],
    name_of: fn(T) -> &'static str,
    default: T,
) -> Result<T, String> {
    if value.is_undefined() {
        Ok(default)
    } else {
        choice(key, value.as_string().as_deref(), all, name_of)
    }
}

fn integer<T>(key: &str, number: Option<f64>, range: &RangeInclusive<T>) -> Result<T, String>
where
    T: Copy + Display + Into<f64> + TryFrom<i64>,
{
    number
        .filter(|number| number.fract() == 0.0)
        .filter(|number| ((*range.start()).into()..=(*range.end()).into()).contains(number))
        .and_then(|number| {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "the number is a whole one within the range"
            )]
            T::try_from(number as i64).ok()
        })
        .ok_or_else(|| {
            format!(
                "`{key}` takes an integer from {} to {}",
                range.start(),
                range.end()
            )
        })
}

fn choice<T: Copy>(
    key: &str,
    name: Option<&str>,
    all: &[T],
    name_of: fn(T) -> &'static str,
) -> Result<T, String> {
    all.iter()
        .copied()
        .find(|&value| Some(name_of(value)) == name)
        .ok_or_else(|| {
            let choices: Vec<_> = all
                .iter()
                .map(|&value| format!("`{}`", name_of(value)))
                .collect();
            format!("`{key}` takes one of {}", choices.join(", "))
        })
}

#[cfg(test)]
mod tests {
    use super::{
        Commas, CommasName, IndentStyle, IndentStyleName, LineEnding, LineEndingName, choice,
        integer,
    };

    #[test]
    fn indent_style_name_is_name_in_nuten_format() {
        for indent_style in IndentStyle::ALL {
            assert_eq!(
                IndentStyleName::from(indent_style).to_str(),
                indent_style.name()
            );
        }
    }

    #[test]
    fn line_ending_name_is_name_in_nuten_format() {
        for line_ending in LineEnding::ALL {
            assert_eq!(
                LineEndingName::from(line_ending).to_str(),
                line_ending.name()
            );
        }
    }

    #[test]
    fn commas_name_is_name_in_nuten_format() {
        for commas in Commas::ALL {
            assert_eq!(CommasName::from(commas).to_str(), commas.name());
        }
    }

    #[test]
    fn integer_takes_whole_number_within_range() {
        assert_eq!(integer("lineWidth", Some(1.0), &(1..=5u32)), Ok(1));
        assert_eq!(integer("lineWidth", Some(5.0), &(1..=5u32)), Ok(5));
    }

    #[test]
    fn integer_rejects_what_is_not_whole_number_within_range() {
        let message = Err("`lineWidth` takes an integer from 1 to 5".to_string());
        for number in [
            None,
            Some(0.0),
            Some(6.0),
            Some(1.5),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            assert_eq!(
                integer("lineWidth", number, &(1..=5u32)),
                message,
                "{number:?}"
            );
        }
    }

    #[test]
    fn choice_takes_listed_name() {
        assert_eq!(
            choice(
                "indentStyle",
                Some("tab"),
                &IndentStyle::ALL,
                IndentStyle::name
            ),
            Ok(IndentStyle::Tab)
        );
    }

    #[test]
    fn choice_rejects_what_is_not_listed_name_listing_choices() {
        let message = Err("`indentStyle` takes one of `space`, `tab`".to_string());
        for name in [None, Some("Tab")] {
            assert_eq!(
                choice("indentStyle", name, &IndentStyle::ALL, IndentStyle::name),
                message,
                "{name:?}"
            );
        }
    }
}
