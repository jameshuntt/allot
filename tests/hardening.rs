use allot::{MemoryKind, MemoryOrchestrator, PAGE_SIZE, PagePool, RawBuf};
use std::alloc::Layout;
use std::sync::Arc;

#[test]
fn rawbuf_survives_many_growth_and_pop_cycles_without_leaking() {
    let memory = Arc::new(MemoryOrchestrator::new());
    let domain = memory.create_domain("rawbuf-stress", None, None);
    {
        let mut buf = RawBuf::new_in(memory.clone(), domain, "values");
        for i in 0u32..20_000 {
            buf.push(i).unwrap();
        }
        assert_eq!(buf.len(), 20_000);
        assert_eq!(buf.as_slice()[0], 0);
        assert_eq!(buf.as_slice()[19_999], 19_999);
        for expected in (10_000u32..20_000).rev() {
            assert_eq!(buf.pop(), Some(expected));
        }
        assert_eq!(buf.len(), 10_000);
    }
    assert_eq!(memory.stats().live_allocations, 0);
    assert_eq!(memory.stats().live_bytes, 0);
}

#[test]
fn page_pool_reuses_a_bounded_working_set_under_long_churn() {
    let memory = Arc::new(MemoryOrchestrator::new());
    let domain = memory.create_domain("pages-stress", None, None);
    {
        let mut pool = PagePool::new_in(memory.clone(), domain, "pages");
        pool.warm(8).unwrap();
        for round in 0..2_000usize {
            let mut leases = (0..8).map(|_| pool.acquire().unwrap()).collect::<Vec<_>>();
            for (i, lease) in leases.iter_mut().enumerate() {
                lease.as_bytes_mut()[0] = (round.wrapping_add(i) & 0xff) as u8;
                lease.as_bytes_mut()[PAGE_SIZE - 1] = i as u8;
            }
            while let Some(lease) = leases.pop() {
                pool.release(lease);
            }
            assert_eq!(pool.in_use_pages(), 0);
            assert_eq!(pool.free_pages(), 8);
        }
    }
    assert_eq!(memory.stats().live_allocations, 0);
}

#[test]
fn orchestrator_bookkeeping_remains_balanced_under_parallel_alloc_free_churn() {
    let memory = Arc::new(MemoryOrchestrator::new());
    let domain = memory.create_domain("parallel", None, None);
    let workers = (0..8)
        .map(|worker| {
            let memory = memory.clone();
            std::thread::spawn(move || {
                for i in 0..2_000usize {
                    let size = 32 + ((i + worker) % 8) * 16;
                    let block = memory
                        .alloc_raw_in(
                            domain,
                            Layout::from_size_align(size, 16).unwrap(),
                            MemoryKind::Scratch,
                            "parallel",
                        )
                        .unwrap();
                    memory.free_block(block.id).unwrap();
                }
            })
        })
        .collect::<Vec<_>>();

    for worker in workers {
        worker.join().unwrap();
    }

    let stats = memory.stats();
    assert_eq!(stats.live_allocations, 0);
    assert_eq!(stats.live_bytes, 0);
    assert!(stats.peak_bytes >= 32);
    let domain_stats = memory.domain_stats(domain.id).unwrap();
    assert_eq!(domain_stats.live_allocations, 0);
    assert_eq!(domain_stats.live_bytes, 0);
}
