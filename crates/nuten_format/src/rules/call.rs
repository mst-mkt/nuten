use nuten_doc::{Element, group, indent, soft_line_break_or_space, space};
use nuten_parser::is_command;
use nuten_syntax::{SyntaxKind, SyntaxNode};

use super::{Child, Formatter, breaks_by_width, children};

impl<'a> Formatter<'a> {
    pub(super) fn call(&mut self, call: &SyntaxNode) -> Vec<Element<'a>> {
        let items: Vec<_> = children(call.children_with_tokens(), &self.kept).collect();
        if !breaks_at_arguments(call, &items) {
            return self.spaced(call);
        }
        let name_len = name_len(call, &items);
        let arguments = self.spaced_children(&items, |index, _| {
            Some(if index < name_len || stays_after_flag(&items, index) {
                space()
            } else {
                soft_line_break_or_space()
            })
        });
        vec![group([indent(arguments)])]
    }

    pub(super) fn holds_call_breaking_at_arguments(&self, element: &SyntaxNode) -> bool {
        element
            .first_child()
            .filter(|call| {
                matches!(call.kind(), SyntaxKind::Call | SyntaxKind::ExternalCall)
                    && !self.kept_up_to(call, element)
            })
            .is_some_and(|call| {
                let items: Vec<_> = children(call.children_with_tokens(), &self.kept).collect();
                breaks_at_arguments(&call, &items)
            })
    }
}

/// One with fewer than two places to break is left as it is, since breaking would not shorten it.
/// One that ends with brackets breaks inside them instead, as `each {|x|` does.
/// A block before the last argument is what a keyword such as `if` takes, which is left as it is.
/// So is a call headed by a value, as `1kb * 2`, which nu reads as an operation, or by `FOO=1`.
fn breaks_at_arguments(call: &SyntaxNode, items: &[Child]) -> bool {
    if !breaks_by_width(call) {
        return false;
    }
    let head_names_no_command = call.first_token().is_some_and(|head| {
        let head = head.text();
        head.starts_with(|c: char| c.is_ascii_digit()) || head.contains('=')
    });
    let breaks = (name_len(call, items)..items.len())
        .filter(|&index| !items[index].is_comment() && !stays_after_flag(items, index))
        .count();
    let significant: Vec<_> = items.iter().filter(|item| !item.is_comment()).collect();
    let bracketed = significant.last().is_some_and(|last| {
        matches!(
            last.kind(),
            SyntaxKind::List
                | SyntaxKind::Table
                | SyntaxKind::Record
                | SyntaxKind::Block
                | SyntaxKind::Subexpression
        )
    });
    let keyword_like = significant
        .iter()
        .rev()
        .skip(1)
        .any(|item| item.kind() == SyntaxKind::Block);
    !head_names_no_command && breaks >= 2 && !bracketed && !keyword_like
}

fn name_len(call: &SyntaxNode, items: &[Child]) -> usize {
    let foreign = call.kind() == SyntaxKind::ExternalCall
        || (items[0].kind() == SyntaxKind::Word && !is_command(word_text(&items[0])));
    let subcommands = if foreign {
        items[1..]
            .iter()
            .take_while(|item| is_subcommand(word_text(item)))
            .count()
    } else {
        0
    };
    1 + subcommands
}

/// nuten cannot know the subcommands of a command nu does not have, so it takes lowercase words.
fn is_subcommand(word: &str) -> bool {
    word.starts_with(|c: char| c.is_ascii_lowercase())
        && word
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn stays_after_flag(items: &[Child], index: usize) -> bool {
    let takes_value = |item: &Child| {
        let text = word_text(item);
        is_flag(text) && !text.contains('=')
    };
    let before_options_end = !items[..index].iter().any(|item| word_text(item) == "--");
    before_options_end
        && items[..index].last().is_some_and(takes_value)
        && !is_flag(word_text(&items[index]))
}

fn is_flag(word: &str) -> bool {
    word.len() > 1
        && word.starts_with('-')
        && !word[1..].starts_with(|c: char| c.is_ascii_digit() || c == '.')
}

fn word_text(item: &Child) -> &str {
    item.element
        .as_token()
        .filter(|token| token.kind() == SyntaxKind::Word)
        .map_or("", |token| token.text())
}
