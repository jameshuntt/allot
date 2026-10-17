use allot::*;
use std::sync::Arc;

#[test]
fn domains_track_and_reclaim_bytes() {
    let mem = MemoryOrchestrator::new();
    let d = mem.create_domain("test", None, None);
    let block = mem
        .alloc_aligned_bytes_in(d, 128, 16, MemoryKind::Heap, "x")
        .unwrap();
    assert_eq!(mem.stats().live_bytes, 128);
    assert_eq!(mem.domain_stats(d.id).unwrap().live_allocations, 1);
    mem.free_block(block.id).unwrap();
    assert_eq!(mem.stats().live_bytes, 0);
}

#[test]
fn rawbuf_grows_and_preserves_values() {
    let mem = Arc::new(MemoryOrchestrator::new());
    let d = mem.create_domain("buf", None, None);
    let mut b = RawBuf::new_in(mem.clone(), d, "raw");
    for i in 0..100 {
        b.push(i).unwrap();
    }
    assert_eq!(b.as_slice(), (0..100).collect::<Vec<_>>().as_slice());
    drop(b);
    assert_eq!(mem.stats().live_allocations, 0);
}

#[test]
fn page_pool_reuses_pages_and_reclaims_on_drop() {
    let mem = Arc::new(MemoryOrchestrator::new());
    let d = mem.create_domain("pages", None, None);
    {
        let mut pool = PagePool::new_in(mem.clone(), d, "page");
        pool.warm(2).unwrap();
        let a = pool.acquire().unwrap();
        let id = a.id();
        pool.release(a);
        let b = pool.acquire().unwrap();
        assert_eq!(b.id(), id);
        pool.release(b);
    }
    assert_eq!(mem.stats().live_allocations, 0);
}

#[test]
fn arena_reset_runs_destructors_and_reuses_storage() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    struct D {
        _byte: u8,
    }
    impl Drop for D {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::SeqCst);
        }
    }
    DROPS.store(0, Ordering::SeqCst);
    let mem = Arc::new(MemoryOrchestrator::new());
    let d = mem.create_domain("arena", None, None);
    let mut a = Arena::new_in(mem.clone(), d, 4096, 64, "arena").unwrap();
    a.alloc(D { _byte: 1 }).unwrap();
    assert!(a.used() > 0);
    a.reset();
    assert_eq!(DROPS.load(Ordering::SeqCst), 1);
    assert_eq!(a.used(), 0);
}

#[test]
fn trace_sink_observes_allocation_lifecycle() {
    let sink = Arc::new(VecTraceSink::default());
    let mem = MemoryOrchestrator::with_sink(sink.clone());
    let d = mem.create_domain("trace", Some(TraceId(1)), Some(JobId(2)));
    let block = mem
        .alloc_aligned_bytes_in(d, 64, 16, MemoryKind::Scratch, "scratch")
        .unwrap();
    mem.free_block(block.id).unwrap();
    let events = sink.events.lock().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, TraceEventKind::Alloc);
    assert_eq!(events[1].kind, TraceEventKind::Free);
}
