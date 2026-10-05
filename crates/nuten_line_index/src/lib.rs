#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

pub fn located<T>(src: &str, items: impl IntoIterator<Item = (usize, T)>) -> Vec<(LineColumn, T)> {
    let (offsets, items): (Vec<_>, Vec<_>) = items.into_iter().unzip();
    line_columns(src, &offsets).into_iter().zip(items).collect()
}

/// No offset is counted from the start of `src` or of its line, as a line can hold many offsets.
fn line_columns(src: &str, offsets: &[usize]) -> Vec<LineColumn> {
    let places = places(src, offsets);
    offsets
        .iter()
        .map(|&offset| places[places.partition_point(|place| place.offset < offset)])
        .map(|place| LineColumn {
            line: place.line,
            column: place.column,
        })
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
    use super::{LineColumn, line_columns, located};

    fn pairs(src: &str, offsets: &[usize]) -> Vec<(usize, usize)> {
        line_columns(src, offsets)
            .into_iter()
            .map(|place| (place.line, place.column))
            .collect()
    }

    #[test]
    fn located_pairs_each_item_with_line_and_column_of_its_offset() {
        let at = |line, column| LineColumn {
            line,
            column,
        };
        assert_eq!(
            located("ls\nx y", [(5, "b"), (0, "a")]),
            [(at(2, 3), "b"), (at(1, 1), "a")]
        );
    }

    #[test]
    fn line_columns_are_one_based_in_characters() {
        assert_eq!(pairs("ls |", &[3]), [(1, 4)]);
        assert_eq!(pairs("ls\n\nls |", &[7]), [(3, 4)]);
        assert_eq!(pairs("ls\r\nls |", &[7]), [(2, 4)]);
        assert_eq!(pairs("echo 日本 |", &[12]), [(1, 9)]);
        assert_eq!(pairs("ls\necho 日本 |", &[15]), [(2, 9)]);
        assert_eq!(pairs("", &[0]), [(1, 1)]);
        assert_eq!(pairs("ls |", &[4]), [(1, 5)]);
        assert_eq!(pairs("ls |\n", &[5]), [(2, 1)]);
    }

    #[test]
    fn line_columns_count_lone_carriage_return_as_character() {
        assert_eq!(pairs("ls\r|", &[3]), [(1, 4)]);
    }

    #[test]
    fn line_columns_come_in_order_of_offsets() {
        let src = "a b\nc 日 d\ne";
        assert_eq!(
            pairs(src, &[12, 0, 10, 2, 4, 2]),
            [(3, 1), (1, 1), (2, 5), (1, 3), (2, 1), (1, 3)]
        );
    }
}
