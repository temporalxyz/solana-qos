//! A fast wait-free mpsc. Not fair, not fifo. It's simply a wrapper
//! around multiple wait-free spsc with all the consumer ends held by a
//! single consumer.

pub use rtrb::Producer;

pub struct Consumer<T> {
    pub consumers: Vec<rtrb::Consumer<T>>,
    pub last: usize,
}

impl<T> Consumer<T> {
    /// This is an unfair (non-fifo) pop
    pub fn pop(&mut self) -> Option<T> {
        let len = self.consumers.len();
        let (right, left) = self.consumers.split_at_mut(self.last);

        let mut visited = 0;

        let opt = right
            .iter_mut()
            .chain(left.iter_mut())
            .find_map(|consumer| {
                visited += 1;
                consumer.pop().ok()
            });

        self.last = (self.last + visited) % len;

        opt
    }

    /// Chains together the iterators of the underlying spsc channels.
    /// When this returns None, there may still exist elements in some
    /// of the spscs consumed earlier.
    pub fn take_all<'a>(&'a mut self) -> impl Iterator<Item = T> + 'a {
        self.consumers
            .iter_mut()
            .map(|c| {
                c.read_chunk(c.slots())
                    .unwrap()
                    .into_iter()
            })
            .flatten()
    }
}

pub fn bounded<T: Send>(
    senders: usize,
    cap_per_sender: usize,
) -> (Vec<Producer<T>>, Consumer<T>) {
    let mut consumers = vec![];
    let mut producers = vec![];

    for _ in 0..senders {
        let (p, c) = rtrb::RingBuffer::new(cap_per_sender);
        producers.push(p);
        consumers.push(c);
    }

    (producers, Consumer { consumers, last: 0 })
}
