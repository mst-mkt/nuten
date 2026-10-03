use rowan::GreenNodeBuilder;

use crate::{Parsed, parser::Event};

pub(crate) fn build(src: &str, events: Vec<Event>) -> Parsed {
    let mut builder = GreenNodeBuilder::new();
    let mut errors = Vec::new();
    for event in events {
        match event {
            Event::Start(kind) => builder.start_node(kind.to_raw()),
            Event::Token(token) => builder.token(token.kind.to_raw(), &src[token.range]),
            Event::Error(error) => errors.push(*error),
            Event::Finish => builder.finish_node(),
        }
    }
    errors.sort_by_key(|error| error.range.start());
    Parsed {
        green: builder.finish(),
        errors,
    }
}
