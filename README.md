# `memory-pool`

**A fixed-size block allocator for constant time (de)allocations.**

The memory pool reserves a fixed size of (virtual) memory and does not grow
with new allocations. We store a pointer in free entries, so types smaller
than a pointer have additional space overhead. The flipside is that we can
rapidly allocate and free entries, no matter the access pattern.

The primary use case of this crate is as a performance optimisation for
(de)allocation heavy code.

# Example

```rust
#![feature(allocator_api)]
use memory_pool::MemoryPool;
use std::alloc::Layout;

struct Data {
    inner: usize,
}

let capacity = 2_usize.pow(20);
let pool = MemoryPool::new(capacity, Layout::new::<Data>());

let elem = Box::new_in(Data { inner: 0 }, &pool);

// We can deallocate during the lifetime of pool
drop(elem);

// This new element can reuse the memory we freed
let elem = Box::new_in(Data { inner: 5 }, &pool);
```