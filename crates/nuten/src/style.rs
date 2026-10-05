use usage::diagnostic;

/// Not usage's, whose painting is private, though argument errors and these must look alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Style {
    colored: bool,
}

impl Style {
    #[cfg(test)]
    pub(crate) const COLORED: Style = Style {
        colored: true,
    };
    #[cfg(test)]
    pub(crate) const PLAIN: Style = Style {
        colored: false,
    };

    pub(crate) fn stderr() -> Style {
        Style {
            colored: diagnostic::Style::auto() == diagnostic::Style::COLOURED,
        }
    }

    pub(crate) fn error(self, text: &str) -> String {
        self.wrap("\u{1b}[1m\u{1b}[31m", text)
    }

    pub(crate) fn bold(self, text: &str) -> String {
        self.wrap("\u{1b}[1m", text)
    }

    fn wrap(self, start: &str, text: &str) -> String {
        if self.colored {
            format!("{start}{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Style;

    #[test]
    fn error_is_bold_red_only_when_colored() {
        assert_eq!(
            Style::COLORED.error("error:"),
            "\u{1b}[1m\u{1b}[31merror:\u{1b}[0m"
        );
        assert_eq!(Style::PLAIN.error("error:"), "error:");
    }
}
