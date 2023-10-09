# `typed-pool`

**A fixed size allocator for single typed, constant time (de)allocations.**

The typed pool reserves a fixed size of (virtual) memory and does not grow with
new allocations. We store a pointer in free entries, so types smaller than a
pointer have additional space overhead. The flipside is that we can rapidly
allocate and free entries, no matter the access pattern.

The primary use case of this crate is as a performance optimisation for
(de)allocation heavy code.

# Example

```rust
use typed_pool::TypedPool;

struct Data {
    inner: usize,
}

let capacity = 2_usize.pow(20);
let pool = TypedPool::new(capacity);

let elem = pool.alloc(Data { inner: 0 });

// We can deallocate during the lifetime of pool
drop(elem);

// This new element can reuse the memory we freed
let elem = pool.alloc(Data { inner: 5 });
```