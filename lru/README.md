# `qos-lru`

A fixed capacity Least-Recently-Used Cache. This implementation squeezes out a bit more performance than other crates by omitting bounds checks and preallocating contiguous memory for all entries.