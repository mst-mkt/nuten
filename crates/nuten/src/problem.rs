use std::{
    fmt,
    path::{Path, PathBuf},
};

use nuten_config::LookupError;
use nuten_line_index::LineColumn;

use crate::picture::pictured;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Problem {
    pub(crate) path: Option<PathBuf>,
    pub(crate) position: Option<Position>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Position {
    pub(crate) line: usize,
    pub(crate) column: Option<usize>,
}

impl Problem {
    pub(crate) fn new(message: impl fmt::Display) -> Problem {
        Problem {
            path: None,
            position: None,
            message: message.to_string(),
        }
    }

    pub(crate) fn in_file(path: &Path, message: impl fmt::Display) -> Problem {
        Problem {
            path: Some(path.to_path_buf()),
            ..Problem::new(message)
        }
    }

    pub(crate) fn at(path: &Path, position: Position, message: impl fmt::Display) -> Problem {
        Problem {
            position: Some(position),
            ..Problem::in_file(path, message)
        }
    }
}

impl From<LineColumn> for Position {
    fn from(line_column: LineColumn) -> Position {
        Position {
            line: line_column.line,
            column: Some(line_column.column),
        }
    }
}

impl From<LookupError> for Problem {
    fn from(error: LookupError) -> Problem {
        Problem {
            path: Some(error.path),
            position: error.line_column.map(Position::from),
            message: error.message,
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            f.write_str(&pictured(&path.display().to_string()))?;
            if let Some(position) = self.position {
                write!(f, ":{}", position.line)?;
                if let Some(column) = position.column {
                    write!(f, ":{column}")?;
                }
            }
            write!(f, ": ")?;
        }
        // Pictured too, since a message can quote a path or a config file.
        f.write_str(&pictured(&self.message))
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use nuten_config::LookupError;
    use nuten_line_index::LineColumn;

    use super::{Position, Problem};

    #[test]
    fn problem_reads_as_path_position_and_message() {
        let path = Path::new("a.nu");
        let line = Position {
            line: 2,
            column: None,
        };
        let column = Position {
            line: 2,
            column: Some(3),
        };
        assert_eq!(Problem::new("x").to_string(), "x");
        assert_eq!(Problem::in_file(path, "x").to_string(), "a.nu: x");
        assert_eq!(Problem::at(path, line, "x").to_string(), "a.nu:2: x");
        assert_eq!(Problem::at(path, column, "x").to_string(), "a.nu:2:3: x");
    }

    #[test]
    fn problem_shows_control_characters_of_path_and_message_as_pictures() {
        assert_eq!(
            Problem::in_file(Path::new("a\u{1b}[2J.nu"), "b\u{1b}[0m\nc").to_string(),
            "a␛[2J.nu: b␛[0m␊c"
        );
    }

    #[test]
    fn problem_of_lookup_error_keeps_its_path_and_position() {
        let error = LookupError {
            path: PathBuf::from("nuten.nuon"),
            line_column: Some(LineColumn {
                line: 2,
                column: 3,
            }),
            message: "e".to_string(),
        };
        let position = Position {
            line: 2,
            column: Some(3),
        };
        assert_eq!(Problem::from(error.clone()).to_string(), error.to_string());
        assert_eq!(
            Problem::from(error),
            Problem::at(Path::new("nuten.nuon"), position, "e")
        );
    }
}
