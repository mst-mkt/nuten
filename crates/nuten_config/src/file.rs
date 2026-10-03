use std::{fmt::Display, fs, io, path::Path};

/// Not `fs::read_to_string` alone, which waits forever on a FIFO.
pub fn read_regular_file(path: &Path) -> io::Result<String> {
    if fs::metadata(path)?.is_file() {
        fs::read_to_string(path)
    } else {
        Err(io::Error::other("not a regular file"))
    }
}

pub fn located<M: Display>(
    path: &Path,
    src: &str,
    messages: impl IntoIterator<Item = (usize, M)>,
) -> Vec<String> {
    let (offsets, messages): (Vec<_>, Vec<_>) = messages.into_iter().unzip();
    line_columns(src, &offsets)
        .into_iter()
        .zip(messages)
        .map(|((line, column), message)| format!("{}:{line}:{column}: {message}", path.display()))
        .collect()
}

/// No offset is counted from the start of `src` or of its line, as a line can hold many errors.
fn line_columns(src: &str, offsets: &[usize]) -> Vec<(usize, usize)> {
    let places = places(src, offsets);
    offsets
        .iter()
        .map(|&offset| places[places.partition_point(|place| place.offset < offset)])
        .map(|place| (place.line, place.column))
        .collect()
}

fn places(src: &str, offsets: &[usize]) -> Vec<Place> {
    let mut ascending = offsets.to_vec();
    ascending.sort_unstable();
    ascending
        .into_iter()
        .scan(Place::START, |place, offset| {
            *place = place.moved_to(src, offset);
            Some(*place)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Place {
    offset: usize,
    line: usize,
    column: usize,
}

impl Place {
    const START: Place = Place {
        offset: 0,
        line: 1,
        column: 1,
    };

    fn moved_to(self, src: &str, offset: usize) -> Place {
        let between = &src[self.offset..offset];
        match between.rsplit_once('\n') {
            Some((before, last_line)) => Place {
                offset,
                line: self.line + before.matches('\n').count() + 1,
                column: last_line.chars().count() + 1,
            },
            None => Place {
                offset,
                column: self.column + between.chars().count(),
                ..self
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::line_columns;

    #[test]
    fn line_columns_are_one_based_in_characters() {
        assert_eq!(line_columns("ls |", &[3]), [(1, 4)]);
        assert_eq!(line_columns("ls\n\nls |", &[7]), [(3, 4)]);
        assert_eq!(line_columns("ls\r\nls |", &[7]), [(2, 4)]);
        assert_eq!(line_columns("echo 日本 |", &[12]), [(1, 9)]);
        assert_eq!(line_columns("ls\necho 日本 |", &[15]), [(2, 9)]);
        assert_eq!(line_columns("", &[0]), [(1, 1)]);
        assert_eq!(line_columns("ls |", &[4]), [(1, 5)]);
        assert_eq!(line_columns("ls |\n", &[5]), [(2, 1)]);
    }

    #[test]
    fn line_columns_count_lone_carriage_return_as_character() {
        assert_eq!(line_columns("ls\r|", &[3]), [(1, 4)]);
    }

    #[test]
    fn line_columns_come_in_order_of_offsets() {
        let src = "a b\nc 日 d\ne";
        assert_eq!(
            line_columns(src, &[12, 0, 10, 2, 4, 2]),
            [(3, 1), (1, 1), (2, 5), (1, 3), (2, 1), (1, 3)]
        );
    }
}
