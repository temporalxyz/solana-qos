use std::alloc::Layout;
use std::collections::HashMap;

use nohash_hasher::{BuildNoHashHasher, IsEnabled};

#[derive(Copy, Clone)]
struct Node<K, V> {
    key: K,
    value: V,
    prev: Option<usize>,
    next: Option<usize>,
}

#[repr(C, align(128))]
pub struct LRUCache<
    K: Copy + Eq + std::hash::Hash + IsEnabled,
    V,
    const N: usize,
> {
    nodes: [Option<Node<K, V>>; N],
    map: HashMap<K, usize, BuildNoHashHasher<K>>,
    free_list: Vec<usize>,
    head: Option<usize>,
    tail: Option<usize>,
}

impl<K: Copy + Eq + std::hash::Hash + IsEnabled, V, const N: usize>
    LRUCache<K, V, N>
{
    const _ASSERT_NONZERO: () =
        assert!(N > 0, "capacity of LRUCache must be nonzero");

    pub fn new() -> Self {
        let mut free_list = Vec::with_capacity(N);
        for i in (0..N).rev() {
            free_list.push(i);
        }

        LRUCache {
            nodes: core::array::from_fn(|_| None),
            map: HashMap::with_capacity_and_hasher(
                N,
                BuildNoHashHasher::<K>::default(),
            ),
            head: None,
            tail: None,
            free_list,
        }
    }

    pub fn new_boxed() -> Box<Self> {
        let layout = Layout::new::<Self>();
        // This needs to be alloc_zeroed otherwise the Vec and
        // HashMap may have nonzero data/capacity and attempt to
        // deallocate a nonexisting allocation
        let ptr = unsafe { std::alloc::alloc_zeroed(layout) };
        if ptr.is_null() {
            // This is a fairly rare case and program should probably
            // not continue if this fails so a panic is okay here.
            panic!("failed allocation");
        }

        // Initialize
        let lru_cache_ptr: *mut Self = ptr.cast();
        unsafe {
            // Note:
            // We iterate and set each element because initializing the
            // full array sometimes leads to a stack
            // overflow for large N...
            for i in 0..N {
                (*lru_cache_ptr).nodes[i] = None;
            }

            (*lru_cache_ptr).free_list = Vec::with_capacity(N);
            for i in (0..N).rev() {
                (*lru_cache_ptr).free_list.push(i);
            }

            (*lru_cache_ptr).map = HashMap::with_capacity_and_hasher(
                N,
                BuildNoHashHasher::<K>::default(),
            );

            (*lru_cache_ptr).head = None;
            (*lru_cache_ptr).tail = None;

            Box::from_raw(lru_cache_ptr)
        }
    }

    pub fn get(&mut self, key: K) -> Option<&V> {
        if let Some(&index) = self.map.get(&key) {
            self.move_to_front(index);
            return unsafe {
                self.nodes
                    .get_unchecked(index)
                    .as_ref()
                    .map(|node| &node.value)
            };
        }
        None
    }

    /// Similar to get but does NOT move to front
    pub fn contains(&self, key: K) -> bool {
        self.map.get(&key).is_some()
    }

    pub fn pop(&mut self, key: &K) -> Option<(K, V)> {
        if let Some(index) = self.map.remove(key) {
            let node = unsafe {
                self.nodes
                    .get_unchecked_mut(index)
                    .take()
                    .unwrap_unchecked()
            };

            if let Some(prev_index) = node.prev {
                unsafe {
                    self.nodes
                        .get_unchecked_mut(prev_index)
                        .as_mut()
                        .unwrap_unchecked()
                        .next = node.next;
                }
            } else {
                self.head = node.next;
            }

            if let Some(next_index) = node.next {
                unsafe {
                    self.nodes
                        .get_unchecked_mut(next_index)
                        .as_mut()
                        .unwrap_unchecked()
                        .prev = node.prev;
                }
            } else {
                self.tail = node.prev;
            }

            self.free_list.push(index);
            Some((node.key, node.value))
        } else {
            None
        }
    }

    /// Returns lru value if full, and returns whether this was a duplicate
    /// 1) not full, not duplicate = None, false
    /// 2) full, not duplicate = Some(...), false
    /// 3) not full, duplicate = None, true
    /// 4) full, duplicate, None, true
    pub fn put(&mut self, key: K, value: V) -> (Option<(K, V)>, bool) {
        if let Some(&index) = self.map.get(&key) {
            // NOTE: If K -> V map is unique, this write can be avoided entirely
            unsafe {
                self.nodes
                    .get_unchecked_mut(index)
                    .as_mut()
                    .unwrap_unchecked()
            }
            .value = value;
            self.move_to_front(index);
            (None, true)
        } else {
            if self.map.len() == N {
                let evicted = self.evict();
                let index =
                    unsafe { self.free_list.pop().unwrap_unchecked() };
                unsafe {
                    *self.nodes.get_unchecked_mut(index) = Some(Node {
                        key,
                        value,
                        prev: None,
                        next: self.head,
                    });
                }

                if let Some(head_index) = self.head {
                    unsafe {
                        self.nodes
                            .get_unchecked_mut(head_index)
                            .as_mut()
                            .unwrap_unchecked()
                    }
                    .prev = Some(index);
                }
                self.head = Some(index);

                if self.tail.is_none() {
                    self.tail = Some(index);
                }

                self.map.insert(key, index);
                return (evicted, false);
            } else {
                let index =
                    unsafe { self.free_list.pop().unwrap_unchecked() };
                unsafe {
                    *self.nodes.get_unchecked_mut(index) = Some(Node {
                        key,
                        value,
                        prev: None,
                        next: self.head,
                    });
                }

                if let Some(head_index) = self.head {
                    unsafe {
                        self.nodes
                            .get_unchecked_mut(head_index)
                            .as_mut()
                            .unwrap_unchecked()
                    }
                    .prev = Some(index);
                }
                self.head = Some(index);

                if self.tail.is_none() {
                    self.tail = Some(index);
                }

                self.map.insert(key, index);
                (None, false)
            }
        }
    }

    fn move_to_front(&mut self, index: usize) {
        if Some(index) == self.head {
            return;
        }

        let prev_index = unsafe {
            self.nodes
                .get_unchecked(index)
                .as_ref()
                .unwrap_unchecked()
                .prev
        };
        let next_index = unsafe {
            self.nodes
                .get_unchecked(index)
                .as_ref()
                .unwrap_unchecked()
                .next
        };

        if let Some(prev) = prev_index {
            unsafe {
                self.nodes
                    .get_unchecked_mut(prev)
                    .as_mut()
                    .unwrap_unchecked()
            }
            .next = next_index;
        }

        if let Some(next) = next_index {
            unsafe {
                self.nodes
                    .get_unchecked_mut(next)
                    .as_mut()
                    .unwrap_unchecked()
            }
            .prev = prev_index;
        } else {
            self.tail = prev_index;
        }

        unsafe {
            self.nodes
                .get_unchecked_mut(index)
                .as_mut()
                .unwrap_unchecked()
        }
        .prev = None;
        unsafe {
            self.nodes
                .get_unchecked_mut(index)
                .as_mut()
                .unwrap_unchecked()
        }
        .next = self.head;

        if let Some(head_index) = self.head {
            unsafe {
                self.nodes
                    .get_unchecked_mut(head_index)
                    .as_mut()
                    .unwrap_unchecked()
            }
            .prev = Some(index);
        }

        self.head = Some(index);
    }

    fn evict(&mut self) -> Option<(K, V)> {
        if let Some(tail_index) = self.tail {
            let tail_node = unsafe {
                self.nodes
                    .get_unchecked_mut(tail_index)
                    .take()
                    .unwrap_unchecked()
            };
            self.map.remove(&tail_node.key);
            self.tail = tail_node.prev;

            if let Some(prev_index) = self.tail {
                unsafe {
                    self.nodes
                        .get_unchecked_mut(prev_index)
                        .as_mut()
                        .unwrap_unchecked()
                }
                .next = None;
            } else {
                self.head = None;
            }

            self.free_list.push(tail_index);
            Some((tail_node.key, tail_node.value))
        } else {
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_retrieve() {
        let mut cache = LRUCache::<i32, &str, 2>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(2, "two"), (None, false));

        assert_eq!(cache.get(1), Some(&"one"));
        assert_eq!(cache.get(2), Some(&"two"));
    }

    #[test]
    fn test_eviction() {
        let mut cache = LRUCache::<i32, &str, 2>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(2, "two"), (None, false));

        // Access the first key to make it most recently used
        assert_eq!(cache.get(1), Some(&"one"));

        // Insert a third key, should evict the least recently used (key
        // 2)
        assert_eq!(cache.put(3, "three"), (Some((2, "two")), false));

        assert_eq!(cache.get(1), Some(&"one")); // Still in cache
        assert_eq!(cache.get(2), None); // Evicted
        assert_eq!(cache.get(3), Some(&"three"));
    }

    #[test]
    fn test_update_existing_key() {
        let mut cache = LRUCache::<i32, &str, 2>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(2, "two"), (None, false));

        // Update the value for key 1
        assert_eq!(cache.put(1, "uno"), (None, true));

        assert_eq!(cache.get(1), Some(&"uno"));
        assert_eq!(cache.get(2), Some(&"two"));
    }

    #[test]
    fn test_lru_property() {
        let mut cache = LRUCache::<i32, &str, 3>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(2, "two"), (None, false));
        assert_eq!(cache.put(3, "three"), (None, false));

        // Access keys in this order: 2, 1, 3
        cache.get(2);
        cache.get(1);
        cache.get(3);

        // Insert a new key, should evict the least recently used (key
        // 2)
        assert_eq!(cache.put(4, "four"), (Some((2, "two")), false));

        assert_eq!(cache.get(1), Some(&"one"));
        assert_eq!(cache.get(2), None); // Evicted
        assert_eq!(cache.get(3), Some(&"three"));
        assert_eq!(cache.get(4), Some(&"four"));
    }

    #[test]
    fn test_eviction_edge_case() {
        let mut cache = LRUCache::<i32, &str, 1>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));

        // Insert a new key, should evict the only existing key
        assert_eq!(cache.put(2, "two"), (Some((1, "one")), false));

        assert_eq!(cache.get(1), None); // Evicted
        assert_eq!(cache.get(2), Some(&"two"));
    }

    #[test]
    fn test_empty_cache() {
        let mut cache = LRUCache::<i32, &str, 1>::new_boxed();
        assert_eq!(cache.get(1), None); // Nothing in cache
    }

    #[test]
    fn test_insert_same_key_multiple_times() {
        let mut cache = LRUCache::<i32, &str, 2>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(1, "uno"), (None, true));

        assert_eq!(cache.get(1), Some(&"uno"));
    }

    #[test]
    fn test_multiple_evictions() {
        let mut cache = LRUCache::<i32, &str, 2>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(2, "two"), (None, false));
        assert_eq!(cache.put(3, "three"), (Some((1, "one")), false));

        assert_eq!(cache.get(1), None); // Evicted
        assert_eq!(cache.get(2), Some(&"two"));
        assert_eq!(cache.get(3), Some(&"three"));

        assert_eq!(cache.put(4, "four"), (Some((2, "two")), false));

        assert_eq!(cache.get(2), None); // Evicted
        assert_eq!(cache.get(3), Some(&"three"));
        assert_eq!(cache.get(4), Some(&"four"));
    }

    #[test]
    fn test_large_capacity() {
        let mut cache = LRUCache::<i32, i32, 1000>::new_boxed();
        for i in 0..1000 {
            assert_eq!(cache.put(i, i), (None, false));
        }

        for i in 0..1000 {
            assert_eq!(cache.get(i), Some(&i));
        }

        assert_eq!(cache.put(1000, 1000), (Some((0, 0)), false)); // First key should be evicted
        assert_eq!(cache.get(0), None); // Evicted
        assert_eq!(cache.get(1000), Some(&1000));
    }

    #[test]
    fn test_get_updates_lru_order() {
        let mut cache = LRUCache::<i32, &str, 2>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(2, "two"), (None, false));

        // Access key 1, making it most recently used
        assert_eq!(cache.get(1), Some(&"one"));

        // Insert a new key, should evict the least recently used (key
        // 2)
        assert_eq!(cache.put(3, "three"), (Some((2, "two")), false));

        assert_eq!(cache.get(1), Some(&"one"));
        assert_eq!(cache.get(2), None); // Evicted
        assert_eq!(cache.get(3), Some(&"three"));
    }

    #[test]
    fn test_no_eviction_on_existing_key_update() {
        let mut cache = LRUCache::<i32, &str, 2>::new_boxed();
        assert_eq!(cache.put(1, "one"), (None, false));
        assert_eq!(cache.put(2, "two"), (None, false));

        // Update existing key, should not cause eviction
        assert_eq!(cache.put(1, "uno"), (None, true));

        assert_eq!(cache.get(1), Some(&"uno"));
        assert_eq!(cache.get(2), Some(&"two"));
    }

    #[test]
    fn test_pop() {
        let mut cache = LRUCache::<i32, &str, 3>::new_boxed();
        cache.put(1, "one");
        cache.put(2, "two");
        cache.put(3, "three");

        // Pop an existing key
        assert_eq!(cache.pop(&2), Some((2, "two")));
        assert_eq!(cache.get(2), None); // Key 2 should be removed
        assert_eq!(cache.get(1), Some(&"one")); // Key 1 should still be there
        assert_eq!(cache.get(3), Some(&"three")); // Key 3 should still be there

        // Pop a non-existing key
        assert_eq!(cache.pop(&4), None);
    }
}
