use usage::{diagnostic, help};

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

    pub(crate) fn stdout() -> Style {
        Style {
            colored: help::Style::auto() == help::Style::COLOURED,
        }
    }

    pub(crate) fn colored(self) -> bool {
        self.colored
    }

    pub(crate) fn error(self, text: &str) -> String {
        self.wrap("\u{1b}[1m\u{1b}[31m", text)
    }

    pub(crate) fn bold(self, text: &str) -> String {
        self.wrap("\u{1b}[1m", text)
    }

    pub(crate) fn tone(self, tone: Tone, text: &str) -> String {
        self.wrap(tone.code(), text)
    }

    pub(crate) fn strong(self, tone: Tone, text: &str) -> String {
        self.wrap(&format!("\u{1b}[1m{}", tone.code()), text)
    }

    pub(crate) fn faint(self, tone: Tone, text: &str) -> String {
        self.wrap(&format!("\u{1b}[2m{}", tone.code()), text)
    }

    fn wrap(self, start: &str, text: &str) -> String {
        if self.colored {
            format!("{start}{text}\u{1b}[0m")
        } else {
            text.to_string()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Dim,
    Removed,
    Added,
}

impl Tone {
    fn code(self) -> &'static str {
        match self {
            Tone::Dim => "\u{1b}[2m",
            Tone::Removed => "\u{1b}[31m",
            Tone::Added => "\u{1b}[32m",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Style, Tone};

    #[test]
    fn error_is_bold_red_only_when_colored() {
        assert_eq!(
            Style::COLORED.error("error:"),
            "\u{1b}[1m\u{1b}[31merror:\u{1b}[0m"
        );
        assert_eq!(Style::PLAIN.error("error:"), "error:");
    }

    #[test]
    fn tone_colors_text_and_strong_makes_it_bold_too() {
        assert_eq!(
            Style::COLORED.tone(Tone::Removed, "a"),
            "\u{1b}[31ma\u{1b}[0m"
        );
        assert_eq!(
            Style::COLORED.strong(Tone::Added, "a"),
            "\u{1b}[1m\u{1b}[32ma\u{1b}[0m"
        );
        assert_eq!(Style::PLAIN.strong(Tone::Dim, "a"), "a");
    }

    #[test]
    fn faint_dims_tone() {
        assert_eq!(
            Style::COLORED.faint(Tone::Added, "a"),
            "\u{1b}[2m\u{1b}[32ma\u{1b}[0m"
        );
        assert_eq!(Style::PLAIN.faint(Tone::Added, "a"), "a");
    }
}
