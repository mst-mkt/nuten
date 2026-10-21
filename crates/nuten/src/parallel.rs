use std::{
    collections::BTreeMap,
    num::NonZeroUsize,
    panic,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
};

/// Hands `each` the result of each item in the order of `items`, once those before it are done.
pub(crate) fn map<T: Sync, R: Send>(
    items: &[T],
    f: impl Fn(&T) -> R + Sync,
    mut each: impl FnMut(R),
) {
    let threads = thread::available_parallelism()
        .map_or(1, NonZeroUsize::get)
        .min(items.len());
    // Not a fixed share for each thread, which one slow file would hold up.
    let next = AtomicUsize::new(0);
    let next_item = || {
        let index = next.fetch_add(1, Ordering::Relaxed);
        items.get(index).map(|item| (index, item))
    };
    let f = &f;
    let (sender, receiver) = mpsc::channel();
    thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let sender = sender.clone();
                scope.spawn(move || {
                    while let Some((index, item)) = next_item() {
                        // Fails once `each` panics, which drops the receiver.
                        if sender.send((index, f(item))).is_err() {
                            break;
                        }
                    }
                })
            })
            .collect();
        drop(sender);
        let mut early = BTreeMap::new();
        let mut due = 0;
        for (index, result) in receiver {
            early.insert(index, result);
            while let Some(result) = early.remove(&due) {
                each(result);
                due += 1;
            }
        }
        for handle in handles {
            handle
                .join()
                .unwrap_or_else(|panic| panic::resume_unwind(panic));
        }
    });
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        thread,
        time::{Duration, Instant},
    };

    use super::map;

    fn collected<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
        let mut results = Vec::new();
        map(items, f, |result| results.push(result));
        results
    }

    #[test]
    fn map_keeps_order_of_items() {
        let items: Vec<usize> = (0..1000).collect();
        assert_eq!(
            collected(&items, |item| item * 2),
            (0..2000).step_by(2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn map_hands_on_result_before_later_items_are_done() {
        let handed = AtomicBool::new(false);
        let wait_for_first = |item: &usize| {
            let start = Instant::now();
            while *item == 1 && !handed.load(Ordering::SeqCst) {
                assert!(start.elapsed() < Duration::from_secs(10), "item 0 held");
                thread::yield_now();
            }
            *item
        };
        let mut results = Vec::new();
        map(&[0, 1], wait_for_first, |result| {
            handed.store(true, Ordering::SeqCst);
            results.push(result);
        });
        assert_eq!(results, [0, 1]);
    }

    #[test]
    fn map_of_no_items_is_empty() {
        let items: [usize; 0] = [];
        assert_eq!(collected(&items, |item| *item), Vec::<usize>::new());
    }

    #[test]
    #[should_panic(expected = "item 500")]
    fn map_passes_on_panic_of_f() {
        let items: Vec<usize> = (0..1000).collect();
        collected(&items, |item| assert_ne!(*item, 500, "item 500"));
    }

    #[test]
    #[should_panic(expected = "result 500")]
    fn map_passes_on_panic_of_each() {
        let items: Vec<usize> = (0..1000).collect();
        map(
            &items,
            |item| *item,
            |result| assert_ne!(result, 500, "result 500"),
        );
    }
}
