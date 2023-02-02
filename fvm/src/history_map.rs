// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use std::borrow::Borrow;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::Hash;

/// A map with an "undo" history. All changes to this map are recorded in the history and can be "reverted" by calling `rollback`. Specifically:
///
/// 1. The user can call `history_len` to record the current history length.
/// 2. The user can _later_ call `rollback(previous_length)` to rollback to the state in step 1.
pub struct HistoryMap<K, V> {
    map: HashMap<K, V>,
    history: Vec<(K, Option<V>)>,
}

impl<K, V> Default for HistoryMap<K, V> {
    fn default() -> Self {
        Self {
            map: Default::default(),
            history: Default::default(),
        }
    }
}

impl<K, V> FromIterator<(K, V)> for HistoryMap<K, V>
where
    K: Eq + Hash,
{
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        HistoryMap {
            map: iter.into_iter().collect(),
            history: Vec::new(),
        }
    }
}

impl<K, V> HistoryMap<K, V>
where
    K: Hash + Eq + Clone,
    V: Eq,
{
    /// Insert a k/v pair into the map, recording the previous value in the history if it differs.
    pub fn insert(&mut self, k: K, v: V) {
        match self.map.entry(k) {
            // Entry doesn't exist, insert it and record that nothing was there.
            Entry::Vacant(e) => {
                self.history.push((e.key().clone(), None));
                e.insert(v);
            }
            // Entry exists and is different, insert it and record the old value.
            Entry::Occupied(mut e) if e.get() != &v => {
                self.history.push((e.key().clone(), Some(e.insert(v))));
            }
            // Entry exists and is the same as the new value, do nothing.
            _ => (),
        }
    }

    /// Lookup a value in the map given a key.
    pub fn get<Q>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.map.get(k)
    }

    /// Looks up a value in the map given a key, or initializes the entry with the provided
    /// function. Any modifications to the map are recorded in the history.
    pub fn get_or_try_insert_with<F, E>(&mut self, k: K, f: F) -> std::result::Result<&V, E>
    where
        F: FnOnce() -> std::result::Result<V, E>,
    {
        match self.map.entry(k) {
            Entry::Occupied(e) => Ok(e.into_mut()),
            Entry::Vacant(e) => {
                let v = f()?;
                self.history.push((e.key().clone(), None));
                Ok(e.insert(v))
            }
        }
    }

    /// Rollback to the specified point in history.
    pub fn rollback(&mut self, height: usize) {
        if self.history.len() <= height {
            return;
        }
        for (k, v) in self.history.drain(height..).rev() {
            match v {
                Some(v) => self.map.insert(k, v),
                None => self.map.remove(&k),
            };
        }
    }

    /// Returns the current history length.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Discards all undo history.
    pub fn discard_history(&mut self) {
        self.history.clear();
    }

    /// Iterate mutably over the current map.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)> {
        self.map.iter_mut()
    }
}

#[cfg(test)]
mod test {
    use super::HistoryMap;

    #[test]
    fn history_map() {
        let mut map = HistoryMap::<i32, &'static str>::default();

        // Basic history tests.
        assert_eq!(map.get(&1), None);
        assert_eq!(map.history_len(), 0);
        map.insert(1, "foo");
        assert_eq!(map.history_len(), 1);
        assert_eq!(map.get(&1), Some(&"foo"));
        map.insert(2, "bar");
        assert_eq!(map.history_len(), 2);
        assert_eq!(map.get(&1), Some(&"foo"));
        assert_eq!(map.get(&2), Some(&"bar"));
        map.insert(1, "baz");
        assert_eq!(map.history_len(), 3);
        assert_eq!(map.get(&1), Some(&"baz"));
        map.rollback(4); // doesn't panic.
        assert_eq!(map.history_len(), 3);
        map.rollback(3); // no-op.
        assert_eq!(map.history_len(), 3);
        assert_eq!(map.get(&1), Some(&"baz"));
        map.rollback(2); // undoes the insert of 1 -> baz
        assert_eq!(map.history_len(), 2);
        assert_eq!(map.get(&1), Some(&"foo"));
        assert_eq!(map.get(&2), Some(&"bar"));
        map.rollback(1); // undoes the insert of 2 -> bar
        assert_eq!(map.history_len(), 1);
        assert_eq!(map.get(&1), Some(&"foo"));
        assert_eq!(map.get(&2), None);
        map.rollback(0); // empties the map
        assert_eq!(map.history_len(), 0);
        assert_eq!(map.get(&1), None);

        // Inserts
        assert_eq!(
            map.get_or_try_insert_with(1, || -> Result<_, ()> { Ok("foo") })
                .unwrap(),
            &"foo",
        );
        assert_eq!(map.get(&1), Some(&"foo"));
        assert_eq!(map.history_len(), 1);

        // Doing it again changes nothing.
        assert_eq!(
            map.get_or_try_insert_with(1, || -> Result<_, ()> { panic!() })
                .unwrap(),
            &"foo",
        );
        assert_eq!(map.history_len(), 1);

        // Inserting without changing the value doesn't increase the history length.
        map.insert(1, "foo");
        assert_eq!(map.history_len(), 1);

        // Bubbles the error and changes nothing.
        assert_eq!(
            map.get_or_try_insert_with(2, || { Err("err") })
                .unwrap_err(),
            "err",
        );
        assert_eq!(map.get(&2), None);
        assert_eq!(map.history_len(), 1);

        // Undo the first insertion.
        map.rollback(0); // empties the map
        assert_eq!(map.history_len(), 0);
        assert_eq!(map.get(&1), None);
    }
}
