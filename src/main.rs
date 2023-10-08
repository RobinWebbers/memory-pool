use typed_pool::TypedPool;

fn main() {
    let capacity = 2_usize.pow(8);
    let allocator = TypedPool::new(capacity);

    let vec: Vec<_> = (0..capacity).map(|i| allocator.alloc(i)).collect();

    for i in 0..capacity {
        assert!(i == *vec[i] )
    }
}
