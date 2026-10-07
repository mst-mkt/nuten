use crate::Element;

/// Holds the children of the elements built with it.
/// So dropping it frees a few chunks, not one list per container.
/// One growing list instead would copy itself as it grows, and leave up to half of itself unused.
#[derive(Debug, Default)]
pub struct Document<'a> {
    chunks: Vec<Vec<Element<'a>>>,
}

/// Only the document that made it can resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Children {
    chunk: u32,
    start: u32,
    len: u32,
}

// Starting small keeps a small document from reserving more than it uses.
// The cap keeps a large one from reserving a chunk it fills only in part.
const FIRST_CHUNK_CAPACITY: usize = 64;
const LARGEST_CHUNK_CAPACITY: usize = 4096;

impl<'a> Document<'a> {
    pub(crate) fn push_children(
        &mut self,
        elements: impl IntoIterator<Item = Element<'a>>,
    ) -> Children {
        let elements = elements.into_iter();
        // Children past the length reserved from the hint grow the chunk, which costs a copy of it.
        // `Children` holds indices, so earlier ones stay valid.
        let (fewest, most) = elements.size_hint();
        let chunk = self.chunk_with_room_for(most.unwrap_or(fewest));
        let chunk_elements = &mut self.chunks[chunk];
        let start = chunk_elements.len();
        chunk_elements.extend(elements);
        Children {
            chunk: to_u32(chunk),
            start: to_u32(start),
            len: to_u32(chunk_elements.len() - start),
        }
    }

    fn chunk_with_room_for(&mut self, len: usize) -> usize {
        let last = self.chunks.last();
        let has_room = last.is_some_and(|chunk| chunk.capacity() - chunk.len() >= len);
        if !has_room {
            let capacity = last.map_or(FIRST_CHUNK_CAPACITY, |chunk| {
                chunk
                    .capacity()
                    .saturating_mul(2)
                    .min(LARGEST_CHUNK_CAPACITY)
            });
            self.chunks.push(Vec::with_capacity(capacity.max(len)));
        }
        self.chunks.len() - 1
    }

    /// Earlier chunks were left only for children too long for their room.
    /// So the room they keep is rarely worth a copy, unlike that of the last one.
    #[must_use]
    pub fn finish(mut self) -> Self {
        if let Some(last) = self.chunks.last_mut() {
            last.shrink_to_fit();
        }
        self
    }

    pub(crate) fn children(&self, children: Children) -> &[Element<'a>] {
        let start = children.start as usize;
        &self.chunks[children.chunk as usize][start..start + children.len as usize]
    }
}

fn to_u32(value: usize) -> u32 {
    u32::try_from(value).expect("a document holds fewer than 2^32 elements")
}

#[cfg(test)]
mod tests {
    use std::iter;

    use super::{Document, FIRST_CHUNK_CAPACITY};
    use crate::{expand_parent, line_suffix_boundary, space};

    #[test]
    fn children_past_room_of_chunk_go_to_next_chunk() {
        let mut doc = Document::default();
        let first = doc.push_children([space()]);
        let filler = doc.push_children(iter::repeat_n(expand_parent(), FIRST_CHUNK_CAPACITY));
        let last = doc.push_children([line_suffix_boundary()]);
        assert_eq!(doc.chunks.len(), 2);
        assert_eq!(doc.children(first), [space()]);
        assert_eq!(
            doc.children(filler),
            [expand_parent(); FIRST_CHUNK_CAPACITY]
        );
        assert_eq!(doc.children(last), [line_suffix_boundary()]);
    }

    #[test]
    fn children_filling_room_of_chunk_stay_in_chunk() {
        let mut doc = Document::default();
        let first = doc.push_children([space()]);
        let rest = doc.push_children(iter::repeat_n(expand_parent(), FIRST_CHUNK_CAPACITY - 1));
        assert_eq!(doc.chunks.len(), 1);
        assert_eq!(doc.children(first), [space()]);
        assert_eq!(
            doc.children(rest),
            [expand_parent(); FIRST_CHUNK_CAPACITY - 1]
        );
    }

    #[test]
    fn children_past_size_hint_grow_chunk() {
        let mut doc = Document::default();
        let first = doc.push_children([space()]);
        let mut left = FIRST_CHUNK_CAPACITY;
        let unhinted = doc.push_children(iter::from_fn(|| {
            left = left.checked_sub(1)?;
            Some(expand_parent())
        }));
        let last = doc.push_children([line_suffix_boundary()]);
        assert_eq!(doc.chunks.len(), 1);
        assert_eq!(doc.children(first), [space()]);
        assert_eq!(
            doc.children(unhinted),
            [expand_parent(); FIRST_CHUNK_CAPACITY]
        );
        assert_eq!(doc.children(last), [line_suffix_boundary()]);
    }
}
