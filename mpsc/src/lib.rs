//! A fast mpsc. Not fair, not fifo. It's simply a wrapper
//! around multiple spsc with all the consumer ends held by a
//! single consumer.
use bytemuck::Pod;
use que::{
    error::QueError,
    headless_spmc::{
        consumer::Consumer as QueConsumer,
        producer::Producer as QueProducer,
    },
    page_size::PageSize,
};

pub struct Consumer<T, const CAP_PER_CHANNEL: usize> {
    pub consumers: Vec<QueConsumer<T, CAP_PER_CHANNEL>>,
    pub last: usize,
}

impl<T: Pod, const CAP_PER_CHANNEL: usize>
    Consumer<T, CAP_PER_CHANNEL>
{
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
                consumer.pop()
            });

        self.last = (self.last + visited) % len;

        opt
    }
}

pub fn bounded<T: Pod, const CAP_PER_CHANNEL: usize>(
    senders: usize,
    base_name: &str,
    #[cfg(target_os = "linux")] page_size: PageSize,
) -> Result<
    (
        Vec<QueProducer<T, CAP_PER_CHANNEL>>,
        Consumer<T, CAP_PER_CHANNEL>,
    ),
    QueError,
> {
    let mut consumers = vec![];
    let mut producers = vec![];

    for i in 0..senders {
        let shmem_id = format!("{base_name}_{i:03}");
        let p = unsafe {
            QueProducer::join_or_create_shmem(
                &shmem_id,
                #[cfg(target_os = "linux")]
                page_size,
            )?
        };
        let c = unsafe {
            QueConsumer::join_shmem(
                &shmem_id,
                #[cfg(target_os = "linux")]
                page_size,
            )?
        };
        producers.push(p);
        consumers.push(c);
    }

    Ok((producers, Consumer { consumers, last: 0 }))
}
