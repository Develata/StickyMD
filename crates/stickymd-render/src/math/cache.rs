//! Small deterministic LRU stores used only by the preview worker.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

use std::collections::HashMap;
use std::hash::Hash;

struct Stamped<V> {
    value: V,
    last_used: u64,
}

pub(super) struct EntryLru<K, V> {
    entries: HashMap<K, Stamped<V>>,
    limit: usize,
    clock: u64,
}

impl<K: Clone + Eq + Hash, V: Clone> EntryLru<K, V> {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            limit,
            clock: 0,
        }
    }

    pub(super) fn get(&mut self, key: &K) -> Option<V> {
        let stamp = self.next_stamp();
        let entry = self.entries.get_mut(key)?;
        entry.last_used = stamp;
        Some(entry.value.clone())
    }

    pub(super) fn insert(&mut self, key: K, value: V) -> bool {
        if self.limit == 0 {
            return false;
        }
        let stamp = self.next_stamp();
        if let Some(entry) = self.entries.get_mut(&key) {
            *entry = Stamped {
                value,
                last_used: stamp,
            };
            return false;
        }
        let evicted = if self.entries.len() == self.limit {
            self.evict_oldest();
            true
        } else {
            false
        };
        self.entries.insert(
            key,
            Stamped {
                value,
                last_used: stamp,
            },
        );
        evicted
    }

    fn next_stamp(&mut self) -> u64 {
        if self.clock == u64::MAX {
            // Cache recency is disposable. Clearing on the practically unreachable wrap boundary
            // is simpler and safer than allowing old entries to become newest.
            self.entries.clear();
            self.clock = 0;
        }
        self.clock += 1;
        self.clock
    }

    fn evict_oldest(&mut self) {
        let oldest = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone());
        if let Some(key) = oldest {
            self.entries.remove(&key);
        }
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }
}

pub(super) struct ByteLru<K, V> {
    entries: HashMap<K, ByteStamped<V>>,
    bytes: usize,
    limit: usize,
    clock: u64,
}

struct ByteStamped<V> {
    value: V,
    bytes: usize,
    last_used: u64,
}

impl<K: Clone + Eq + Hash, V: Clone> ByteLru<K, V> {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            limit,
            clock: 0,
        }
    }

    pub(super) fn get(&mut self, key: &K) -> Option<V> {
        let stamp = self.next_stamp();
        let entry = self.entries.get_mut(key)?;
        entry.last_used = stamp;
        Some(entry.value.clone())
    }

    pub(super) fn insert(&mut self, key: K, value: V, bytes: usize) -> usize {
        if bytes > self.limit || self.limit == 0 {
            return 0;
        }
        let stamp = self.next_stamp();
        if let Some(old) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(old.bytes);
        }
        let mut evicted = 0;
        while self.bytes.saturating_add(bytes) > self.limit {
            let Some(removed_bytes) = self.evict_oldest() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(removed_bytes);
            evicted += 1;
        }
        self.entries.insert(
            key,
            ByteStamped {
                value,
                bytes,
                last_used: stamp,
            },
        );
        self.bytes = self.bytes.saturating_add(bytes);
        evicted
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    fn next_stamp(&mut self) -> u64 {
        if self.clock == u64::MAX {
            self.entries.clear();
            self.bytes = 0;
            self.clock = 0;
        }
        self.clock += 1;
        self.clock
    }

    fn evict_oldest(&mut self) -> Option<usize> {
        let oldest = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone())?;
        self.entries.remove(&oldest).map(|entry| entry.bytes)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) const fn bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_lru_moves_hits_and_evicts_the_oldest() {
        let mut cache = EntryLru::new(2);
        assert!(!cache.insert(1, "one"));
        assert!(!cache.insert(2, "two"));
        assert_eq!(cache.get(&1), Some("one"));
        assert!(cache.insert(3, "three"));
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&1), Some("one"));
    }

    #[test]
    fn byte_lru_never_exceeds_its_budget() {
        let mut cache = ByteLru::new(10);
        assert_eq!(cache.insert(1, "one", 6), 0);
        assert_eq!(cache.insert(2, "two", 6), 1);
        assert_eq!(cache.bytes(), 6);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.insert(3, "large", 11), 0);
        assert_eq!(cache.bytes(), 6);
    }

    #[test]
    fn byte_lru_hit_protects_the_entry_from_the_next_eviction() {
        let mut cache = ByteLru::new(12);
        assert_eq!(cache.insert(1, "one", 6), 0);
        assert_eq!(cache.insert(2, "two", 6), 0);
        assert_eq!(cache.get(&1), Some("one"));
        assert_eq!(cache.insert(3, "three", 6), 1);
        assert_eq!(cache.get(&2), None);
        assert_eq!(cache.get(&1), Some("one"));
        assert_eq!(cache.get(&3), Some("three"));
    }
}
