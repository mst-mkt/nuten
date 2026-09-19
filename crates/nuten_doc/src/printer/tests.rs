use super::{IndentStyle, LineEnding, PrinterOptions, print};
use crate::{
    Element, GroupId, align, align_if_group_breaks, empty_line, empty_lines, expand_parent, group,
    group_with_id, hard_line_break, indent, line_suffix, line_suffix_boundary, soft_line_break,
    soft_line_break_or_space, space, text,
};

fn options(line_width: u32) -> PrinterOptions {
    PrinterOptions {
        line_width,
        indent_style: IndentStyle::Space,
        indent_width: 2,
        line_ending: LineEnding::Lf,
    }
}

fn comment() -> Element<'static> {
    line_suffix(vec![space(), text("# c")])
}

#[test]
fn text_and_space_go_out_as_they_are() {
    let doc = vec![text("echo"), space(), text("a")];
    assert_eq!(print(&doc, &options(80)), "echo a");
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
    assert_eq!(print(&doc, &options(80)), "a\n  b\n    c\nd");
    let tabs = PrinterOptions {
        indent_style: IndentStyle::Tab,
        ..options(80)
    };
    assert_eq!(print(&doc, &tabs), "a\n\tb\n\t\tc\nd");
}

#[test]
fn indent_without_a_line_break_prints_nothing() {
    let doc = vec![indent(vec![text("a")])];
    assert_eq!(print(&doc, &options(80)), "a");
}

#[test]
fn a_space_at_the_start_of_a_line_comes_after_the_indent() {
    let doc = vec![
        text("a"),
        indent(vec![hard_line_break(), space(), text("b")]),
    ];
    assert_eq!(print(&doc, &options(80)), "a\n   b");
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
    assert_eq!(print(&doc, &options(80)), "a\nb\n\nc");
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
    assert_eq!(print(&doc, &options(80)), "a\nb\n\nc\n\nd\n\ne");
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
    let crlf = PrinterOptions {
        line_ending: LineEnding::CrLf,
        ..options(80)
    };
    assert_eq!(print(&doc, &crlf), "a\r\nb\r\n\r\nc");
}

#[test]
fn an_empty_line_prints_the_blank_lines_it_carries() {
    let doc = vec![
        text("a"),
        empty_lines(2),
        empty_lines(2),
        text("b"),
        empty_line(),
        empty_lines(2),
        text("c"),
    ];
    assert_eq!(print(&doc, &options(80)), "a\n\n\nb\n\n\nc");
}

#[test]
fn only_the_last_line_of_a_text_is_trimmed() {
    let doc = vec![text("a \nb "), hard_line_break(), text("c")];
    assert_eq!(print(&doc, &options(80)), "a \nb\nc");
}

#[test]
fn a_line_of_spaces_alone_goes() {
    let doc = vec![
        text("a"),
        hard_line_break(),
        space(),
        hard_line_break(),
        text("b"),
    ];
    assert_eq!(print(&doc, &options(80)), "a\nb");
}

#[test]
fn the_output_never_starts_with_a_line_break() {
    let doc = vec![hard_line_break(), empty_line(), text("a")];
    assert_eq!(print(&doc, &options(80)), "a");
    let doc = vec![space(), hard_line_break(), text("a")];
    assert_eq!(print(&doc, &options(80)), "a");
    let doc = vec![indent(vec![hard_line_break(), text("a")])];
    assert_eq!(print(&doc, &options(80)), "  a");
}

#[test]
fn empty_lines_carry_no_indent() {
    let doc = vec![text("a"), indent(vec![empty_line(), text("b")])];
    assert_eq!(print(&doc, &options(80)), "a\n\n  b");
}

#[test]
fn a_final_line_break_leaves_no_indent_behind() {
    let doc = vec![indent(vec![text("a"), hard_line_break()])];
    assert_eq!(print(&doc, &options(80)), "a\n");
}

#[test]
fn line_ending_is_configurable() {
    let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
    let crlf = PrinterOptions {
        line_ending: LineEnding::CrLf,
        ..options(80)
    };
    assert_eq!(print(&doc, &crlf), "a\r\n  b");
}

#[test]
fn indent_width_sets_the_spaces_per_level() {
    let doc = vec![text("a"), indent(vec![hard_line_break(), text("b")])];
    let wide = PrinterOptions {
        indent_width: 4,
        ..options(80)
    };
    assert_eq!(print(&doc, &wide), "a\n    b");
}

#[test]
fn a_group_stays_flat_up_to_the_line_width() {
    let doc = vec![group(vec![
        text("aa"),
        soft_line_break_or_space(),
        text("bb"),
        soft_line_break(),
        text("cc"),
    ])];
    assert_eq!(print(&doc, &options(7)), "aa bbcc");
    assert_eq!(print(&doc, &options(6)), "aa\nbb\ncc");
}

#[test]
fn a_group_past_the_line_width_breaks_at_once() {
    let doc = vec![text("aaaa"), group(vec![soft_line_break(), text("b")])];
    assert_eq!(print(&doc, &options(3)), "aaaa\nb");
}

#[test]
fn soft_lines_outside_a_group_break() {
    let doc = vec![text("a"), soft_line_break_or_space(), text("b")];
    assert_eq!(print(&doc, &options(80)), "a\nb");
}

#[test]
fn a_group_inside_a_flat_group_stays_flat() {
    let doc = vec![group(vec![
        text("a"),
        soft_line_break_or_space(),
        group(vec![text("b"), soft_line_break(), text("c")]),
    ])];
    assert_eq!(print(&doc, &options(80)), "a bc");
}

#[test]
fn a_forced_break_breaks_the_groups_around() {
    let hard = vec![group(vec![
        text("a"),
        soft_line_break_or_space(),
        group(vec![text("b"), soft_line_break_or_space(), text("c")]),
        soft_line_break_or_space(),
        group(vec![text("d"), hard_line_break(), text("e")]),
    ])];
    assert_eq!(print(&hard, &options(80)), "a\nb c\nd\ne");
    let empty = vec![group(vec![text("a"), empty_line(), text("b")])];
    assert_eq!(print(&empty, &options(80)), "a\n\nb");
    let expand = vec![group(vec![
        text("a"),
        soft_line_break_or_space(),
        group(vec![text("b"), expand_parent(), text("c")]),
    ])];
    assert_eq!(print(&expand, &options(80)), "a\nbc");
    let multiline = vec![group(vec![
        text("a\nb"),
        soft_line_break_or_space(),
        text("c"),
    ])];
    assert_eq!(print(&multiline, &options(80)), "a\nb\nc");
    let in_indent = vec![group(vec![
        text("a"),
        indent(vec![
            soft_line_break_or_space(),
            text("b"),
            hard_line_break(),
            text("c"),
        ]),
    ])];
    assert_eq!(print(&in_indent, &options(80)), "a\n  b\n  c");
}

#[test]
fn a_forced_break_inside_a_line_suffix_breaks_the_group_around() {
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
        assert_eq!(print(&doc, &options(80)), "a\nb", "{forced_break:?}");
    }
}

#[test]
fn a_boundary_inside_a_line_suffix_does_not_end_the_measurement() {
    let doc = vec![group(vec![
        text("a"),
        line_suffix(vec![line_suffix_boundary(), hard_line_break()]),
        soft_line_break_or_space(),
        text("b"),
    ])];
    assert_eq!(print(&doc, &options(80)), "a\nb");
}

#[test]
fn a_forced_break_inside_a_line_suffix_after_the_group_leaves_it_flat() {
    let doc = vec![
        group(vec![text("a"), soft_line_break_or_space(), text("b")]),
        line_suffix(vec![hard_line_break(), text("# c")]),
    ];
    assert_eq!(print(&doc, &options(80)), "a b\n# c");
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
    assert_eq!(print(&doc, &options(6)), "call\na b\nc d");
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
    assert_eq!(print(&group_then(vec![text(";")]), &options(4)), "a b;");
    assert_eq!(print(&group_then(vec![text(";;")]), &options(4)), "a\nb;;");
    let then_line = vec![hard_line_break(), text("long line")];
    assert_eq!(print(&group_then(then_line), &options(4)), "a b\nlong line");
    let then_text = vec![text("x\r\ny")];
    assert_eq!(print(&group_then(then_text), &options(4)), "a bx\ny");
    let then_suffix = vec![line_suffix(vec![text("# a comment")])];
    assert_eq!(
        print(&group_then(then_suffix), &options(4)),
        "a b# a comment"
    );
}

#[test]
fn the_indent_takes_columns() {
    let doc = vec![
        text("a"),
        indent(vec![
            hard_line_break(),
            group(vec![text("aa"), soft_line_break_or_space(), text("bb")]),
        ]),
    ];
    assert_eq!(print(&doc, &options(7)), "a\n  aa bb");
    assert_eq!(print(&doc, &options(6)), "a\n  aa\n  bb");
    let tabs = PrinterOptions {
        indent_style: IndentStyle::Tab,
        ..options(6)
    };
    assert_eq!(print(&doc, &tabs), "a\n\taa\n\tbb");
}

#[test]
fn line_breaks_inside_a_text_come_out_as_the_line_ending() {
    let doc = vec![text("a\r\nb\nc\rd"), hard_line_break(), text("e")];
    assert_eq!(print(&doc, &options(80)), "a\nb\nc\rd\ne");
    let crlf = PrinterOptions {
        line_ending: LineEnding::CrLf,
        ..options(80)
    };
    assert_eq!(print(&doc, &crlf), "a\r\nb\r\nc\rd\r\ne");
    let doc = vec![text("a\n"), hard_line_break(), text("b")];
    assert_eq!(print(&doc, &options(80)), "a\nb");
    assert_eq!(print(&doc, &crlf), "a\r\nb");
}

#[test]
fn only_the_first_line_of_a_text_is_indented() {
    let doc = vec![text("a"), indent(vec![hard_line_break(), text("b\nc")])];
    assert_eq!(print(&doc, &options(80)), "a\n  b\nc");
}

#[test]
fn a_multiline_text_resets_the_column() {
    let doc = vec![
        text("a\nbb"),
        group(vec![text("c"), soft_line_break_or_space(), text("d")]),
    ];
    assert_eq!(print(&doc, &options(5)), "a\nbbc d");
}

#[test]
fn a_line_suffix_keeps_the_indent_and_mode_of_its_place() {
    let doc = vec![
        text("a"),
        indent(vec![line_suffix(vec![hard_line_break(), text("# c")])]),
        hard_line_break(),
        text("b"),
    ];
    assert_eq!(print(&doc, &options(80)), "a\n  # c\nb");
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
    assert_eq!(print(&doc, &options(80)), "a b # c!\nd");
}

#[test]
fn a_boundary_in_a_flat_group_keeps_a_suffix_group_flat() {
    let doc = vec![
        text("a"),
        line_suffix(vec![group(vec![
            text(" #"),
            soft_line_break_or_space(),
            text("c"),
        ])]),
        group(vec![line_suffix_boundary(), text("b")]),
    ];
    assert_eq!(print(&doc, &options(80)), "a # c\nb");
}

#[test]
fn a_line_suffix_waits_for_the_next_line_break() {
    let doc = vec![
        text("a"),
        comment(),
        space(),
        text("b"),
        hard_line_break(),
        text("c"),
    ];
    assert_eq!(print(&doc, &options(80)), "a b # c\nc");
    let doc = vec![text("a"), comment(), empty_line(), text("b")];
    assert_eq!(print(&doc, &options(80)), "a # c\n\nb");
    let doc = vec![group(vec![
        text("a"),
        comment(),
        soft_line_break_or_space(),
        text("b"),
    ])];
    assert_eq!(print(&doc, &options(80)), "a b # c");
    assert_eq!(print(&doc, &options(2)), "a # c\nb");
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
    assert_eq!(print(&doc, &options(80)), "a x y\nb");
}

#[test]
fn a_line_suffix_at_the_end_still_prints() {
    let doc = vec![text("a"), comment()];
    assert_eq!(print(&doc, &options(80)), "a # c");
}

#[test]
fn a_boundary_breaks_the_line_only_after_a_suffix() {
    let doc = vec![text("a"), comment(), line_suffix_boundary(), text("b")];
    assert_eq!(print(&doc, &options(80)), "a # c\nb");
    let doc = vec![text("a"), line_suffix_boundary(), text("b")];
    assert_eq!(print(&doc, &options(80)), "ab");
}

#[test]
fn a_line_suffix_takes_no_width() {
    let doc = vec![group(vec![
        text("aa"),
        line_suffix(vec![text(" # a long comment")]),
        soft_line_break_or_space(),
        text("bb"),
    ])];
    assert_eq!(print(&doc, &options(5)), "aa bb # a long comment");
}

#[test]
fn a_boundary_after_a_suffix_ends_the_line_when_measuring() {
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
    assert_eq!(print(&doc, &options(7)), "aaa # c\nbbb ccc");
}

#[test]
fn an_alignment_adds_its_columns_to_the_indent() {
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
    assert_eq!(print(&doc, &options(80)), "a\n  b\n    c\n   d");
}

#[test]
fn with_tabs_an_alignment_before_a_level_takes_a_tab() {
    let tabs = PrinterOptions {
        indent_style: IndentStyle::Tab,
        ..options(80)
    };
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
    assert_eq!(print(&doc, &tabs), "a\n  b\n\t\tc\n\t\t d");
}

#[test]
fn an_alignment_on_a_group_follows_how_the_group_printed() {
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
    assert_eq!(print(&doc, &options(80)), "aaa | b\nc");
    assert_eq!(print(&doc, &options(4)), "aaa\n| b\n  c");
}

#[test]
fn an_alignment_takes_columns() {
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
    assert_eq!(print(&doc, &options(8)), "a\n   aa bb");
    assert_eq!(print(&doc, &options(7)), "a\n   aa\n   bb");
}

#[test]
fn an_alignment_takes_columns_after_tabs_too() {
    let doc = |line_width| {
        let tabs = PrinterOptions {
            line_width,
            indent_style: IndentStyle::Tab,
            indent_width: 4,
            line_ending: LineEnding::Lf,
        };
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
        print(&doc, &tabs)
    };
    assert_eq!(doc(11), "a\n\t  bb cc");
    assert_eq!(doc(10), "a\n\t  bb\n\t  cc");
}

#[test]
fn zero_columns_are_no_alignment() {
    let tabs = PrinterOptions {
        indent_style: IndentStyle::Tab,
        ..options(80)
    };
    let doc = vec![
        text("a"),
        align(0, vec![indent(vec![hard_line_break(), text("b")])]),
    ];
    assert_eq!(print(&doc, &tabs), "a\n\tb");
}

#[test]
fn an_alignment_on_a_flat_or_unprinted_group_does_nothing() {
    let flat = vec![group(vec![
        group_with_id(GroupId(0), vec![text("a")]),
        align_if_group_breaks(GroupId(0), 2, vec![hard_line_break(), text("b")]),
    ])];
    assert_eq!(print(&flat, &options(80)), "a\nb");
    let in_flat_parent = vec![
        group(vec![group_with_id(
            GroupId(3),
            vec![text("x"), soft_line_break(), text("y")],
        )]),
        align_if_group_breaks(GroupId(3), 2, vec![hard_line_break(), text("b")]),
    ];
    assert_eq!(print(&in_flat_parent, &options(80)), "xy\nb");
    let unprinted = vec![
        align_if_group_breaks(GroupId(1), 2, vec![text("a"), hard_line_break(), text("b")]),
        group_with_id(GroupId(1), vec![text("c"), hard_line_break(), text("d")]),
    ];
    assert_eq!(print(&unprinted, &options(80)), "a\nbc\nd");
    let inside_own = vec![group_with_id(
        GroupId(2),
        vec![
            text("xx"),
            soft_line_break_or_space(),
            text("yy"),
            align_if_group_breaks(GroupId(2), 2, vec![hard_line_break(), text("z")]),
        ],
    )];
    assert_eq!(print(&inside_own, &options(80)), "xx\nyy\n  z");
}

#[test]
fn an_alignment_leaves_empty_lines_empty_and_suffixes_in_place() {
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
    assert_eq!(print(&doc, &options(80)), "a\n\n  b\n  # c\nd");
}
