//! Generic ordered overlay collapse helpers.
//!
//! This module intentionally knows nothing about sandbox paths, host paths, or
//! metadata. Callers project their domain-specific layers into ordered overlay
//! effects, then use the same collapse operation to derive the effective view.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayEffect<T> {
    Value(T),
    Tombstone,
}

pub fn collapse<T, I, F>(entries: I, mut merge: F) -> Option<T>
where
    I: IntoIterator<Item = (u64, OverlayEffect<T>)>,
    F: FnMut(Option<T>, OverlayEffect<T>) -> Option<T>,
{
    let mut entries: Vec<_> = entries.into_iter().collect();
    entries.sort_by_key(|(order, _)| *order);
    let mut state = None;
    for (_, effect) in entries {
        state = merge(state, effect);
    }
    state
}

pub fn collapse_latest<T, I>(entries: I) -> Option<T>
where
    I: IntoIterator<Item = (u64, OverlayEffect<T>)>,
{
    collapse(entries, |_, effect| match effect {
        OverlayEffect::Value(value) => Some(value),
        OverlayEffect::Tombstone => None,
    })
}
