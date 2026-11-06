//! Loading many small files at once: startup reads tens of thousands of
//! textures and models with a content pack, and one at a time it waits on
//! each open and read in turn.

/// `f` over `items` on the machine's threads, results in `items`' order.
/// Threads take the next item as they finish one, so a run of slow items
/// doesn't hold back the others.
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    horizon_threads::loading::map(items, f)
}

#[cfg(test)]
mod tests {
    use super::par_map;

    #[test]
    fn results_keep_the_items_order() {
        let items: Vec<u32> = (0..1000).collect();
        assert_eq!(
            par_map(&items, |x| x * 2),
            items.iter().map(|x| x * 2).collect::<Vec<_>>()
        );
        assert!(par_map(&[] as &[u32], |x| *x).is_empty());
    }
}
