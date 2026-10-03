use std::{
    iter,
    num::NonZeroUsize,
    panic,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
};

pub(crate) fn map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(items.len());
    // Not a fixed share for each thread, which one slow file would hold up.
    let next = AtomicUsize::new(0);
    let work = || {
        iter::from_fn(|| {
            let index = next.fetch_add(1, Ordering::Relaxed);
            items.get(index).map(|item| (index, f(item)))
        })
        .collect::<Vec<_>>()
    };
    let mut results: Vec<_> = thread::scope(|scope| {
        let handles: Vec<_> = (0..threads).map(|_| scope.spawn(work)).collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| panic::resume_unwind(panic))
            })
            .collect()
    });
    results.sort_unstable_by_key(|(index, _)| *index);
    results.into_iter().map(|(_, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::map;

    #[test]
    fn map_keeps_order_of_items() {
        let items: Vec<usize> = (0..1000).collect();
        assert_eq!(
            map(&items, |item| item * 2),
            (0..2000).step_by(2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn map_of_no_items_is_empty() {
        let items: [usize; 0] = [];
        assert!(map(&items, |item| *item).is_empty());
    }

    #[test]
    #[should_panic(expected = "item 500")]
    fn map_passes_on_panic_of_f() {
        let items: Vec<usize> = (0..1000).collect();
        map(&items, |item| assert_ne!(*item, 500, "item 500"));
    }
}
