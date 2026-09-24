use super::{IndentStyle, LineEnding, PrinterOptions, print};
use crate::{
    Element, GroupId, align, align_if_group_breaks, empty_line, empty_lines, expand_parent, group,
    group_with_id, hard_line_break, indent, line_suffix, line_suffix_boundary, soft_line_break,
    soft_line_break_or_space, space, text,
};

/// Wide enough that nothing breaks for want of room.
const WIDE: PrinterOptions = PrinterOptions {
    line_width: 80,
    indent_style: IndentStyle::Space,
    indent_width: 2,
    line_ending: LineEnding::Lf,
};

const TABS: PrinterOptions = PrinterOptions {
    indent_style: IndentStyle::Tab,
    ..WIDE
};

const CRLF: PrinterOptions = PrinterOptions {
    line_ending: LineEnding::CrLf,
    ..WIDE
};

fn narrow(line_width: u32) -> PrinterOptions {
    PrinterOptions {
        line_width,
        ..WIDE
    }
}

fn comment() -> Element<'static> {
    line_suffix(vec![space(), text("# c")])
}

#[test]
fn text_and_space_go_out_as_they_are() {
    let doc = vec![text("echo"), space(), text("a")];
    assert_eq!(print(&doc, &WIDE), "echo a");
}

#[test]
fn hard_line_breaks_indent_the_next_text() {
    let doc = vec![
        text("a"),
        indent(vec![
            hard_line_break(),
            text("b"),
            indent(vec![hard_line_break(), text("c")]),
        ]),
        hard_line_break(),
        text("d"),
    ];
    assert_eq!(print(&doc, &WIDE), "a\n  b\n    c\nd");
    assert_eq!(print(&doc, &TABS), "a\n\tb\n\t\tc\nd");
}

#[test]
fn indent_without_a_line_break_prints_nothing() {
    let doc = vec![indent(vec![text("a")])];
    assert_eq!(print(&doc, &WIDE), "a");
}

#[test]
fn space_at_start_of_line_comes_after_indent() {
    let doc = vec![
        text("a"),
        indent(vec![hard_line_break(), space(), text("b")]),
    ];
    assert_eq!(print(&doc, &WIDE), "a\n   b");
}

#[test]
fn spaces_and_tabs_at_the_end_of_a_line_go() {
    let doc = vec![
        text("a "),
        space(),
        hard_line_break(),
        text("b\t"),
        empty_line(),
        text("c"),
    ];
    assert_eq!(print(&doc, &WIDE), "a\nb\n\nc");
}

#[test]
fn line_breaks_in_a_row_collapse() {
    let doc = vec![
        text("a"),
        hard_line_break(),
        hard_line_break(),
        text("b"),
        hard_line_break(),
        empty_line(),
        text("c"),
        empty_line(),
        hard_line_break(),
        text("d"),
        empty_line(),
        empty_line(),
        text("e"),
    ];
    assert_eq!(print(&doc, &WIDE), "a\nb\n\nc\n\nd\n\ne");
}

#[test]
fn line_breaks_collapse_with_crlf_too() {
    let doc = vec![
        text("a "),
        hard_line_break(),
        hard_line_break(),
        text("b"),
        empty_line(),
        empty_line(),
        text("c"),
    ];
    assert_eq!(print(&doc, &CRLF), "a\r\nb\r\n\r\nc");
}

#[test]
fn empty_line_prints_blank_lines_it_carries() {
    let doc = vec![
        text("a"),
        empty_lines(2),
        empty_lines(2),
        text("b"),
        empty_line(),
        empty_lines(2),
        text("c"),
    ];
    assert_eq!(print(&doc, &WIDE), "a\n\n\nb\n\n\nc");
}

#[test]
fn only_the_last_line_of_a_text_is_trimmed() {
    let doc = vec![text("a \nb "), hard_line_break(), text("c")];
    assert_eq!(print(&doc, &WIDE), "a \nb\nc");
}

#[test]
fn carriage_returns_before_printed_line_break_go_only_with_lf() {
    let doc = vec![text("a \r\r"), hard_line_break(), text("b\r \r")];
    assert_eq!(print(&doc, &WIDE), "a\nb\r \r");
    assert_eq!(print(&doc, &CRLF), "a \r\r\r\nb\r \r");
}

#[test]
fn carriage_returns_before_line_break_inside_text_go_only_with_lf() {
    let doc = vec![text("a\r\r\nb \r\nc")];
    assert_eq!(print(&doc, &WIDE), "a\nb \nc");
    assert_eq!(print(&doc, &CRLF), "a\r\r\nb \r\nc");
    let doc = vec![text("a"), indent(vec![hard_line_break(), text("\r\r\nb")])];
    assert_eq!(print(&doc, &WIDE), "a\n\nb");
    assert_eq!(print(&doc, &CRLF), "a\r\n  \r\r\nb");
}

#[test]
fn first_line_of_text_is_measured_without_carriage_returns_of_its_line_break() {
    let doc = vec![
        group(vec![text("a"), soft_line_break_or_space(), text("b")]),
        text("x\r\r\ny"),
    ];
    assert_eq!(print(&doc, &narrow(4)), "a bx\ny");
    let crlf = PrinterOptions {
        line_ending: LineEnding::CrLf,
        ..narrow(4)
    };
    assert_eq!(print(&doc, &crlf), "a\r\nbx\r\r\ny");
}

#[test]
fn line_of_spaces_alone_goes() {
    let doc = vec![
        text("a"),
        hard_line_break(),
        space(),
        hard_line_break(),
        text("b"),
    ];
    assert_eq!(print(&doc, &WIDE), "a\nb");
}

#[test]
fn output_never_starts_with_line_break() {
    let doc = vec![hard_line_break(), empty_line(), text("a")];
    assert_eq!(print(&doc, &WIDE), "a");
    let doc = vec![space(), hard_line_break(), text("a")];
    assert_eq!(print(&doc, &WIDE), "a");
    let doc = vec![indent(vec![hard_line_break(), text("a")])];
    assert_eq!(print(&doc, &WIDE), "  a");
}

#[test]
fn empty_lines_carry_no_indent() {
    let doc = vec![text("a"), indent(vec![empty_line(), text("b")])];
    assert_eq!(print(&doc, &WIDE), "a\n\n  b");
}

#[test]
fn final_line_break_leaves_no_indent_behind() {
    let doc = vec![indent(vec![text("a"), hard_line_break()])];
    assert_eq!(print(&doc, &WIDE), "a\n");
}

#[test]
fn line_ending_is_configurable() {
    let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
    assert_eq!(print(&doc, &CRLF), "a\r\n  b");
}

#[test]
fn indent_width_sets_the_spaces_per_level() {
    let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
    let four_column_indent = PrinterOptions {
        indent_width: 4,
        ..WIDE
    };
    assert_eq!(print(&doc, &four_column_indent), "a\n    b");
}

#[test]
fn group_stays_flat_up_to_line_width() {
    let doc = vec![group(vec![
        text("aa"),
        soft_line_break_or_space(),
        text("bb"),
        soft_line_break(),
        text("cc"),
    ])];
    assert_eq!(print(&doc, &narrow(7)), "aa bbcc");
    assert_eq!(print(&doc, &narrow(6)), "aa\nbb\ncc");
}

#[test]
fn group_past_line_width_breaks_at_once() {
    let doc = vec![text("aaaa"), group(vec![soft_line_break(), text("b")])];
    assert_eq!(print(&doc, &narrow(3)), "aaaa\nb");
}

#[test]
fn soft_lines_outside_a_group_break() {
    let doc = vec![text("a"), soft_line_break_or_space(), text("b")];
    assert_eq!(print(&doc, &WIDE), "a\nb");
}

#[test]
fn group_inside_flat_group_stays_flat() {
    let doc = vec![group(vec![
        text("a"),
        soft_line_break_or_space(),
        group(vec![text("b"), soft_line_break(), text("c")]),
    ])];
    assert_eq!(print(&doc, &WIDE), "a bc");
}

#[test]
fn forced_break_breaks_groups_around() {
    let hard = vec![group(vec![
        text("a"),
        soft_line_break_or_space(),
        group(vec![text("b"), soft_line_break_or_space(), text("c")]),
        soft_line_break_or_space(),
        group(vec![text("d"), hard_line_break(), text("e")]),
    ])];
    assert_eq!(print(&hard, &WIDE), "a\nb c\nd\ne");
    let empty = vec![group(vec![text("a"), empty_line(), text("b")])];
    assert_eq!(print(&empty, &WIDE), "a\n\nb");
    let expand = vec![group(vec![
        text("a"),
        soft_line_break_or_space(),
        group(vec![text("b"), expand_parent(), text("c")]),
    ])];
    assert_eq!(print(&expand, &WIDE), "a\nbc");
    let multiline = vec![group(vec![
        text("a\nb"),
        soft_line_break_or_space(),
        text("c"),
    ])];
    assert_eq!(print(&multiline, &WIDE), "a\nb\nc");
    let in_indent = vec![group(vec![
        text("a"),
        indent(vec![
            soft_line_break_or_space(),
            text("b"),
            hard_line_break(),
            text("c"),
        ]),
    ])];
    assert_eq!(print(&in_indent, &WIDE), "a\n  b\n  c");
}

#[test]
fn forced_break_inside_line_suffix_breaks_group_around() {
    let forced_breaks = [
        vec![expand_parent()],
        vec![hard_line_break()],
        vec![text("\n")],
        vec![indent(vec![group(vec![hard_line_break()])])],
        vec![line_suffix(vec![hard_line_break()])],
    ];
    for forced_break in forced_breaks {
        let doc = vec![group(vec![
            text("a"),
            line_suffix(forced_break.clone()),
            soft_line_break_or_space(),
            text("b"),
        ])];
        assert_eq!(print(&doc, &WIDE), "a\nb", "{forced_break:?}");
    }
}

#[test]
fn boundary_inside_line_suffix_does_not_end_measurement() {
    let doc = vec![group(vec![
        text("a"),
        line_suffix(vec![line_suffix_boundary(), hard_line_break()]),
        soft_line_break_or_space(),
        text("b"),
    ])];
    assert_eq!(print(&doc, &WIDE), "a\nb");
}

#[test]
fn forced_break_inside_line_suffix_after_group_leaves_it_flat() {
    let doc = vec![
        group(vec![text("a"), soft_line_break_or_space(), text("b")]),
        line_suffix(vec![hard_line_break(), text("# c")]),
    ];
    assert_eq!(print(&doc, &WIDE), "a b\n# c");
}

#[test]
fn groups_inside_a_broken_group_fit_on_their_own() {
    let doc = vec![group(vec![
        text("call"),
        soft_line_break_or_space(),
        group(vec![text("a"), soft_line_break_or_space(), text("b")]),
        soft_line_break_or_space(),
        group(vec![text("c"), soft_line_break_or_space(), text("d")]),
    ])];
    assert_eq!(print(&doc, &narrow(6)), "call\na b\nc d");
}

#[test]
fn what_follows_a_group_on_the_line_counts() {
    let group_then = |rest: Vec<Element<'static>>| {
        let mut doc = vec![group(vec![
            text("a"),
            soft_line_break_or_space(),
            text("b"),
        ])];
        doc.extend(rest);
        doc
    };
    assert_eq!(print(&group_then(vec![text(";")]), &narrow(4)), "a b;");
    assert_eq!(print(&group_then(vec![text(";;")]), &narrow(4)), "a\nb;;");
    let then_line = vec![hard_line_break(), text("long line")];
    assert_eq!(print(&group_then(then_line), &narrow(4)), "a b\nlong line");
    let then_text = vec![text("x\r\ny")];
    assert_eq!(print(&group_then(then_text), &narrow(4)), "a bx\ny");
    let then_suffix = vec![line_suffix(vec![text("# a comment")])];
    assert_eq!(
        print(&group_then(then_suffix), &narrow(4)),
        "a b# a comment"
    );
}

#[test]
fn indent_takes_columns() {
    let doc = vec![
        text("a"),
        indent(vec![
            hard_line_break(),
            group(vec![text("aa"), soft_line_break_or_space(), text("bb")]),
        ]),
    ];
    assert_eq!(print(&doc, &narrow(7)), "a\n  aa bb");
    assert_eq!(print(&doc, &narrow(6)), "a\n  aa\n  bb");
    let narrow_tabs = PrinterOptions {
        indent_style: IndentStyle::Tab,
        ..narrow(6)
    };
    assert_eq!(print(&doc, &narrow_tabs), "a\n\taa\n\tbb");
}

#[test]
fn line_breaks_inside_a_text_come_out_as_the_line_ending() {
    let doc = vec![text("a\r\nb\nc\rd"), hard_line_break(), text("e")];
    assert_eq!(print(&doc, &WIDE), "a\nb\nc\rd\ne");
    assert_eq!(print(&doc, &CRLF), "a\r\nb\r\nc\rd\r\ne");
    let doc = vec![text("a\n"), hard_line_break(), text("b")];
    assert_eq!(print(&doc, &WIDE), "a\nb");
    assert_eq!(print(&doc, &CRLF), "a\r\nb");
}

#[test]
fn only_the_first_line_of_a_text_is_indented() {
    let doc = vec![text("a"), indent(vec![hard_line_break(), text("b\nc")])];
    assert_eq!(print(&doc, &WIDE), "a\n  b\nc");
}

#[test]
fn empty_first_line_of_text_takes_no_indent() {
    for multiline in ["\nb", "\r\nb"] {
        let doc = vec![text("a"), indent(vec![hard_line_break(), text(multiline)])];
        assert_eq!(print(&doc, &WIDE), "a\n\nb", "{multiline:?}");
        assert_eq!(print(&doc, &TABS), "a\n\nb", "{multiline:?}");
    }
}

#[test]
fn empty_text_leaves_indent_to_next_text() {
    let doc = vec![
        text("a"),
        indent(vec![hard_line_break(), text(""), text("b")]),
    ];
    assert_eq!(print(&doc, &WIDE), "a\n  b");
}

#[test]
fn multiline_text_resets_column() {
    let doc = vec![
        text("a\nbb"),
        group(vec![text("c"), soft_line_break_or_space(), text("d")]),
    ];
    assert_eq!(print(&doc, &narrow(5)), "a\nbbc d");
}

#[test]
fn line_suffix_keeps_indent_and_mode_of_its_place() {
    let doc = vec![
        text("a"),
        indent(vec![line_suffix(vec![hard_line_break(), text("# c")])]),
        hard_line_break(),
        text("b"),
    ];
    assert_eq!(print(&doc, &WIDE), "a\n  # c\nb");
    let doc = vec![
        group(vec![
            text("a"),
            line_suffix(vec![space(), text("# c"), soft_line_break(), text("!")]),
            space(),
            text("b"),
        ]),
        hard_line_break(),
        text("d"),
    ];
    assert_eq!(print(&doc, &WIDE), "a b # c!\nd");
}

#[test]
fn boundary_in_flat_group_keeps_suffix_group_flat() {
    let doc = vec![
        text("a"),
        line_suffix(vec![group(vec![
            text(" #"),
            soft_line_break_or_space(),
            text("c"),
        ])]),
        group(vec![line_suffix_boundary(), text("b")]),
    ];
    assert_eq!(print(&doc, &WIDE), "a # c\nb");
}

#[test]
fn line_suffix_waits_for_next_line_break() {
    let doc = vec![
        text("a"),
        comment(),
        space(),
        text("b"),
        hard_line_break(),
        text("c"),
    ];
    assert_eq!(print(&doc, &WIDE), "a b # c\nc");
    let doc = vec![text("a"), comment(), empty_line(), text("b")];
    assert_eq!(print(&doc, &WIDE), "a # c\n\nb");
    let doc = vec![group(vec![
        text("a"),
        comment(),
        soft_line_break_or_space(),
        text("b"),
    ])];
    assert_eq!(print(&doc, &WIDE), "a b # c");
    assert_eq!(print(&doc, &narrow(2)), "a # c\nb");
}

#[test]
fn line_suffixes_keep_their_order() {
    let doc = vec![
        text("a"),
        line_suffix(vec![text(" x")]),
        line_suffix(vec![text(" y")]),
        hard_line_break(),
        text("b"),
    ];
    assert_eq!(print(&doc, &WIDE), "a x y\nb");
}

#[test]
fn line_suffix_at_end_still_prints() {
    let doc = vec![text("a"), comment()];
    assert_eq!(print(&doc, &WIDE), "a # c");
}

#[test]
fn boundary_breaks_line_only_after_suffix() {
    let doc = vec![text("a"), comment(), line_suffix_boundary(), text("b")];
    assert_eq!(print(&doc, &WIDE), "a # c\nb");
    let doc = vec![text("a"), line_suffix_boundary(), text("b")];
    assert_eq!(print(&doc, &WIDE), "ab");
}

#[test]
fn line_suffix_takes_no_width() {
    let doc = vec![group(vec![
        text("aa"),
        line_suffix(vec![text(" # a long comment")]),
        soft_line_break_or_space(),
        text("bb"),
    ])];
    assert_eq!(print(&doc, &narrow(5)), "aa bb # a long comment");
}

#[test]
fn boundary_after_suffix_ends_line_when_measuring() {
    let doc = vec![
        text("aaa"),
        comment(),
        group(vec![
            line_suffix_boundary(),
            text("bbb"),
            soft_line_break_or_space(),
            text("ccc"),
        ]),
    ];
    assert_eq!(print(&doc, &narrow(7)), "aaa # c\nbbb ccc");
}

#[test]
fn alignment_adds_its_columns_to_indent() {
    let doc = vec![
        text("a"),
        align(
            2,
            vec![
                hard_line_break(),
                text("b"),
                indent(vec![hard_line_break(), text("c")]),
            ],
        ),
        indent(vec![align(1, vec![hard_line_break(), text("d")])]),
    ];
    assert_eq!(print(&doc, &WIDE), "a\n  b\n    c\n   d");
}

#[test]
fn with_tabs_an_alignment_before_a_level_takes_a_tab() {
    let doc = vec![
        text("a"),
        align(
            2,
            vec![
                hard_line_break(),
                text("b"),
                indent(vec![
                    hard_line_break(),
                    text("c"),
                    align(1, vec![hard_line_break(), text("d")]),
                ]),
            ],
        ),
    ];
    assert_eq!(print(&doc, &TABS), "a\n  b\n\t\tc\n\t\t d");
}

#[test]
fn alignment_on_group_follows_how_group_printed() {
    let doc = vec![
        group_with_id(
            GroupId(0),
            vec![text("aaa"), soft_line_break_or_space(), text("|")],
        ),
        align_if_group_breaks(
            GroupId(0),
            2,
            vec![space(), text("b"), hard_line_break(), text("c")],
        ),
    ];
    assert_eq!(print(&doc, &WIDE), "aaa | b\nc");
    assert_eq!(print(&doc, &narrow(4)), "aaa\n| b\n  c");
}

#[test]
fn alignment_takes_columns() {
    let doc = vec![
        text("a"),
        align(
            3,
            vec![
                hard_line_break(),
                group(vec![text("aa"), soft_line_break_or_space(), text("bb")]),
            ],
        ),
    ];
    assert_eq!(print(&doc, &narrow(8)), "a\n   aa bb");
    assert_eq!(print(&doc, &narrow(7)), "a\n   aa\n   bb");
}

#[test]
fn alignment_takes_columns_after_tabs_too() {
    let doc = vec![
        text("a"),
        indent(vec![align(
            2,
            vec![
                hard_line_break(),
                group(vec![text("bb"), soft_line_break_or_space(), text("cc")]),
            ],
        )]),
    ];
    let four_column_tabs = |line_width| PrinterOptions {
        line_width,
        indent_width: 4,
        ..TABS
    };
    assert_eq!(print(&doc, &four_column_tabs(11)), "a\n\t  bb cc");
    assert_eq!(print(&doc, &four_column_tabs(10)), "a\n\t  bb\n\t  cc");
}

#[test]
fn zero_columns_are_no_alignment() {
    let doc = vec![
        text("a"),
        align(0, vec![indent(vec![hard_line_break(), text("b")])]),
    ];
    assert_eq!(print(&doc, &TABS), "a\n\tb");
}

#[test]
fn alignment_on_flat_or_unprinted_group_does_nothing() {
    let flat = vec![group(vec![
        group_with_id(GroupId(0), vec![text("a")]),
        align_if_group_breaks(GroupId(0), 2, vec![hard_line_break(), text("b")]),
    ])];
    assert_eq!(print(&flat, &WIDE), "a\nb");
    let in_flat_parent = vec![
        group(vec![group_with_id(
            GroupId(3),
            vec![text("x"), soft_line_break(), text("y")],
        )]),
        align_if_group_breaks(GroupId(3), 2, vec![hard_line_break(), text("b")]),
    ];
    assert_eq!(print(&in_flat_parent, &WIDE), "xy\nb");
    let unprinted = vec![
        align_if_group_breaks(GroupId(1), 2, vec![text("a"), hard_line_break(), text("b")]),
        group_with_id(GroupId(1), vec![text("c"), hard_line_break(), text("d")]),
    ];
    assert_eq!(print(&unprinted, &WIDE), "a\nbc\nd");
    let inside_own = vec![group_with_id(
        GroupId(2),
        vec![
            text("xx"),
            soft_line_break_or_space(),
            text("yy"),
            align_if_group_breaks(GroupId(2), 2, vec![hard_line_break(), text("z")]),
        ],
    )];
    assert_eq!(print(&inside_own, &WIDE), "xx\nyy\n  z");
}

#[test]
fn alignment_leaves_empty_lines_empty_and_suffixes_in_place() {
    let doc = vec![
        text("a"),
        align(
            2,
            vec![
                empty_line(),
                text("b"),
                line_suffix(vec![hard_line_break(), text("# c")]),
            ],
        ),
        hard_line_break(),
        text("d"),
    ];
    assert_eq!(print(&doc, &WIDE), "a\n\n  b\n  # c\nd");
}
