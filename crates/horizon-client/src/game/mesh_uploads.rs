//! Pending meshes belong to the latest loaded section, nearest first.
use std::collections::HashSet;
type Key = (i32, i32, i32);

pub(super) fn merge<T>(
    pending: &mut Vec<(Key, T)>,
    uploads: Vec<(Key, T)>,
    loaded: impl Fn(&Key) -> bool,
    center: (i32, i32),
) -> Vec<(Key, T)> {
    let fresh: HashSet<Key> = uploads.iter().map(|(key, _)| *key).collect();
    let mut spent: Vec<_> = pending
        .extract_if(.., |(key, _)| fresh.contains(key) || !loaded(key))
        .collect();
    for upload in uploads {
        if loaded(&upload.0) {
            pending.push(upload);
        } else {
            spent.push(upload);
        }
    }
    pending.sort_unstable_by_key(|(key, _)| {
        let x = i64::from(key.0) - i64::from(center.0);
        let z = i64::from(key.2) - i64::from(center.1);
        std::cmp::Reverse(x * x + z * z)
    });
    spent
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newer_mesh_replaces_waiting_data_unloaded_sections_drop_nearest_progresses_first() {
        let mut pending = vec![
            ((1, 0, 0), "old"),
            ((2, 0, 0), "unloaded"),
            ((5, 0, 0), "far"),
        ];
        let spent = merge(
            &mut pending,
            vec![
                ((1, 0, 0), "new"),
                ((0, 0, 0), "nearest"),
                ((2, 1, 0), "gone"),
            ],
            |key| key.0 != 2,
            (0, 0),
        );
        assert_eq!(spent.len(), 3);
        assert_eq!(pending.pop(), Some(((0, 0, 0), "nearest")));
        assert_eq!(pending.pop(), Some(((1, 0, 0), "new")));
        assert_eq!(pending.pop(), Some(((5, 0, 0), "far")));
        assert!(pending.is_empty());
    }
}
