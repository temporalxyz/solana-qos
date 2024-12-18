pub struct MinMaxHeap<T, const N: usize> {
    inner: min_max_heap::MinMaxHeap<T>,
}

impl<T: Ord, const N: usize> MinMaxHeap<T, N> {
    const _ASSERT: () = assert!(N > 0);

    #[inline(always)]
    pub fn new() -> MinMaxHeap<T, N> {
        MinMaxHeap {
            inner: min_max_heap::MinMaxHeap::with_capacity(N),
        }
    }

    // If the inner heap is at capacity, this returns the minimum value
    #[inline(always)]
    pub fn push(&mut self, value: T) -> Option<T> {
        let mut maybe_min = None;

        // Push while under capacity
        self.inner.push(value);

        // Get minimium value if full
        //
        // This means heap holds up to N - 1 items outside of this scope
        if self.inner.len() == N {
            maybe_min.replace(self.inner.pop_min().unwrap());
        }

        maybe_min
    }

    // Get an iterator over
    #[inline(always)]
    pub fn get_max_values<'a>(&'a mut self) -> PopDesc<'a, T, N> {
        PopDesc { inner: self }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

pub struct PopDesc<'a, T, const N: usize> {
    inner: &'a mut MinMaxHeap<T, N>,
}

impl<'a, T: Ord, const N: usize> Iterator for PopDesc<'a, T, N> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.inner.pop_max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_capacity() {
        // Initialize heap
        let mut heap = MinMaxHeap::<u32, 4>::new();

        // Fill up heap
        assert!(heap.push(3).is_none());
        assert!(heap.push(1).is_none());
        assert!(heap.push(2).is_none());

        // When filled, should return smallest value
        assert_eq!(heap.push(4), Some(1));
    }

    #[test]
    fn get_max_values() {
        // Initialize heap
        let mut heap = MinMaxHeap::<u32, 4>::new();

        // Fill up heap
        assert!(heap.push(3).is_none());
        assert!(heap.push(1).is_none());
        assert!(heap.push(2).is_none());

        // Ensure values are returned in descending order
        let mut iterator = heap.get_max_values();
        assert_eq!(iterator.next(), Some(3));
        assert_eq!(iterator.next(), Some(2));
        assert_eq!(iterator.next(), Some(1));
    }

    #[test]
    fn drops_after() {
        // Initialize heap
        let mut heap = MinMaxHeap::<u32, 4>::new();

        // Fill up heap
        assert!(heap.push(3).is_none());
        assert!(heap.push(1).is_none());
        assert!(heap.push(2).is_none());

        let mut iter = heap.get_max_values();
        assert_eq!(iter.next(), Some(3));
        drop(iter);

        let mut iter = heap.get_max_values();
        assert_eq!(iter.next(), Some(2));
        drop(iter);
    }
}
