use std::path::Path;

use super::absolute_as_written;
use crate::{diff::Difference, problem::Problem};

const TITLE: &str = "nuten";

pub(crate) fn file_annotation(path: &Path, difference: &Difference<'_>) -> String {
    let lines = difference.changed_span();
    let properties = [
        ("file", file_property(path)),
        ("line", lines.start().to_string()),
        ("endLine", lines.end().to_string()),
    ];
    annotation(&properties, "Would be reformatted")
}

pub(crate) fn problem_annotation(problem: &Problem) -> String {
    let file = problem.path.as_deref().map(file_property);
    let line = problem.position.map(|position| position.line.to_string());
    let column = problem.position.and_then(|position| position.column);
    let properties: Vec<_> = [
        file.map(|file| ("file", file)),
        line.map(|line| ("line", line)),
        column.map(|column| ("col", column.to_string())),
    ]
    .into_iter()
    .flatten()
    .collect();
    annotation(&properties, &problem.message)
}

fn annotation(properties: &[(&str, String)], message: &str) -> String {
    let properties: Vec<_> = properties
        .iter()
        .chain([&("title", TITLE.to_string())])
        .map(|(key, value)| format!("{key}={}", escape_property(value)))
        .collect();
    format!(
        "::error {}::{}\n",
        properties.join(","),
        escape_data(message)
    )
}

/// Absolute, since the runner reads a relative path from the root of the repository.
fn file_property(path: &Path) -> String {
    // Lossy for names that are not UTF-8, as the list of `--list-different` is.
    absolute_as_written(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

fn escape_data(text: &str) -> String {
    text.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(text: &str) -> String {
    escape_data(text).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use std::{env, path::Path};

    use super::{escape_data, escape_property, file_annotation, problem_annotation};
    use crate::{
        diff::Difference,
        problem::{Position, Problem},
    };

    #[test]
    fn file_annotation_spans_lines_from_first_change_to_last() {
        let root = if cfg!(windows) { r"C:\r" } else { "/r" };
        let path = Path::new(root).join("a.nu");
        let difference = Difference::new("a\nb|c\nd\n", "a\nb | c\nd\n");
        assert_eq!(
            file_annotation(&path, &difference),
            format!(
                "::error file={},line=2,endLine=2,title=nuten::Would be reformatted\n",
                path.display().to_string().replace(':', "%3A")
            )
        );
    }

    #[test]
    fn problem_annotation_holds_path_line_and_column_that_problem_has() {
        let path = env::current_dir().unwrap().join("a.nu");
        let file = path.display().to_string().replace(':', "%3A");
        let position = Position {
            line: 2,
            column: Some(3),
        };
        let line = Position {
            line: 2,
            column: None,
        };
        assert_eq!(
            problem_annotation(&Problem::new("e")),
            "::error title=nuten::e\n"
        );
        assert_eq!(
            problem_annotation(&Problem::in_file(Path::new("a.nu"), "e")),
            format!("::error file={file},title=nuten::e\n")
        );
        assert_eq!(
            problem_annotation(&Problem::at(Path::new("a.nu"), line, "e")),
            format!("::error file={file},line=2,title=nuten::e\n")
        );
        assert_eq!(
            problem_annotation(&Problem::at(Path::new("a.nu"), position, "e")),
            format!("::error file={file},line=2,col=3,title=nuten::e\n")
        );
    }

    #[test]
    fn problem_annotation_escapes_message_onto_one_line() {
        assert_eq!(
            problem_annotation(&Problem::new("a%\nb")),
            "::error title=nuten::a%25%0Ab\n"
        );
    }

    #[test]
    fn escape_keeps_annotation_on_one_line_and_its_properties_apart() {
        assert_eq!(escape_data("a%b\r\nc:d,e"), "a%25b%0D%0Ac:d,e");
        assert_eq!(escape_property("a%b\r\nc:d,e"), "a%25b%0D%0Ac%3Ad%2Ce");
    }
}
