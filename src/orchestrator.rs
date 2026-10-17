use crate::trace::*;
use crate::types::*;
use std::alloc::{Layout, alloc, dealloc};
use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone)]
struct AllocationRecord {
    canonical_ptr: NonNull<u8>,
    layout: Layout,
    kind: MemoryKind,
    label: &'static str,
    domain_id: MemoryDomainId,
}
#[derive(Debug, Clone)]
struct DomainRecord {
    label: &'static str,
    trace_id: Option<TraceId>,
    job_id: Option<JobId>,
    stats: DomainStats,
}
#[derive(Default)]
struct Inner {
    next_alloc_id: usize,
    next_domain_id: u64,
    live_bytes: usize,
    peak_bytes: usize,
    allocations: HashMap<AllocationId, AllocationRecord>,
    domains: HashMap<MemoryDomainId, DomainRecord>,
}

pub struct MemoryOrchestrator {
    inner: Mutex<Inner>,
    sink: SharedTraceSink,
}
// SAFETY: all allocation records are accessed behind `inner`; raw pointers are
// never dereferenced by cross-thread bookkeeping and deallocation is serialized.
unsafe impl Send for MemoryOrchestrator {}
// SAFETY: same reasoning as `Send`; shared methods synchronize mutable state.
unsafe impl Sync for MemoryOrchestrator {}

impl Default for MemoryOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}
impl MemoryOrchestrator {
    pub fn new() -> Self {
        Self::with_sink(Arc::new(NoopTraceSink))
    }
    pub fn with_sink(sink: SharedTraceSink) -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
            sink,
        }
    }

    pub fn create_domain(
        &self,
        label: &'static str,
        trace_id: Option<TraceId>,
        job_id: Option<JobId>,
    ) -> MemoryDomain {
        let mut inner = self.inner.lock().expect("memory orchestrator poisoned");
        let id = MemoryDomainId(inner.next_domain_id);
        inner.next_domain_id = inner.next_domain_id.wrapping_add(1);
        inner.domains.insert(
            id,
            DomainRecord {
                label,
                trace_id,
                job_id,
                stats: DomainStats::default(),
            },
        );
        MemoryDomain {
            id,
            trace_id,
            job_id,
            label,
        }
    }

    pub fn contains_domain(&self, domain_id: MemoryDomainId) -> bool {
        self.inner
            .lock()
            .expect("memory orchestrator poisoned")
            .domains
            .contains_key(&domain_id)
    }
    pub fn domain_stats(&self, domain_id: MemoryDomainId) -> Result<DomainStats, MemoryError> {
        self.inner
            .lock()
            .expect("memory orchestrator poisoned")
            .domains
            .get(&domain_id)
            .map(|d| d.stats)
            .ok_or(MemoryError::UnknownDomain(domain_id))
    }
    pub fn stats(&self) -> MemoryStats {
        let inner = self.inner.lock().expect("memory orchestrator poisoned");
        MemoryStats {
            live_allocations: inner.allocations.len(),
            live_bytes: inner.live_bytes,
            peak_bytes: inner.peak_bytes,
        }
    }

    fn emit(
        &self,
        domain: MemoryDomain,
        alloc_id: Option<AllocationId>,
        kind: TraceEventKind,
        memory_kind: Option<MemoryKind>,
        size: usize,
        align: usize,
        label: &'static str,
        detail: Option<&'static str>,
    ) {
        self.sink.record(TraceEvent {
            at: Instant::now(),
            trace_id: domain.trace_id,
            job_id: domain.job_id,
            domain_id: domain.id,
            domain_label: domain.label,
            alloc_id,
            kind,
            memory_kind,
            size,
            align,
            label,
            detail,
        });
    }
    pub fn emit_custom(&self, domain: MemoryDomain, label: &'static str, detail: &'static str) {
        self.emit(
            domain,
            None,
            TraceEventKind::Custom,
            None,
            0,
            0,
            label,
            Some(detail),
        );
    }
    pub(crate) fn emit_event(
        &self,
        domain: MemoryDomain,
        kind: TraceEventKind,
        label: &'static str,
        detail: Option<&'static str>,
    ) {
        self.emit(domain, None, kind, None, 0, 0, label, detail);
    }

    pub fn alloc_raw_in(
        &self,
        domain: MemoryDomain,
        layout: Layout,
        kind: MemoryKind,
        label: &'static str,
    ) -> Result<MemoryBlock, MemoryError> {
        if layout.size() == 0 {
            return Err(MemoryError::ZeroSizedAllocation);
        }
        {
            let inner = self.inner.lock().expect("memory orchestrator poisoned");
            if !inner.domains.contains_key(&domain.id) {
                return Err(MemoryError::UnknownDomain(domain.id));
            }
        }
        // SAFETY: layout is valid and non-zero; null is checked below.
        let raw = unsafe { alloc(layout) };
        let ptr = NonNull::new(raw).ok_or(MemoryError::AllocFailed {
            size: layout.size(),
            align: layout.align(),
        })?;
        let mut inner = self.inner.lock().expect("memory orchestrator poisoned");
        let id = AllocationId(inner.next_alloc_id);
        inner.next_alloc_id = inner.next_alloc_id.wrapping_add(1);
        inner.live_bytes = inner.live_bytes.saturating_add(layout.size());
        inner.peak_bytes = inner.peak_bytes.max(inner.live_bytes);
        let domain_record = inner
            .domains
            .get_mut(&domain.id)
            .ok_or(MemoryError::UnknownDomain(domain.id))?;
        domain_record.stats.live_allocations =
            domain_record.stats.live_allocations.saturating_add(1);
        domain_record.stats.live_bytes =
            domain_record.stats.live_bytes.saturating_add(layout.size());
        domain_record.stats.peak_bytes = domain_record
            .stats
            .peak_bytes
            .max(domain_record.stats.live_bytes);
        inner.allocations.insert(
            id,
            AllocationRecord {
                canonical_ptr: ptr,
                layout,
                kind,
                label,
                domain_id: domain.id,
            },
        );
        drop(inner);
        self.emit(
            domain,
            Some(id),
            TraceEventKind::Alloc,
            Some(kind),
            layout.size(),
            layout.align(),
            label,
            None,
        );
        Ok(MemoryBlock {
            id,
            domain_id: domain.id,
            ptr,
            layout,
            kind,
            label,
        })
    }

    pub fn alloc_array_uninit_in<T>(
        &self,
        domain: MemoryDomain,
        count: usize,
        kind: MemoryKind,
        label: &'static str,
    ) -> Result<TypedHandle<T>, MemoryError> {
        let layout = Layout::array::<T>(count).map_err(|_| MemoryError::Layout)?;
        let block = self.alloc_raw_in(domain, layout, kind, label)?;
        Ok(TypedHandle::new(block))
    }
    pub fn alloc_aligned_bytes_in(
        &self,
        domain: MemoryDomain,
        size: usize,
        align: usize,
        kind: MemoryKind,
        label: &'static str,
    ) -> Result<MemoryBlock, MemoryError> {
        let layout = Layout::from_size_align(size, align).map_err(|_| MemoryError::Layout)?;
        self.alloc_raw_in(domain, layout, kind, label)
    }

    pub fn free_block(&self, id: AllocationId) -> Result<(), MemoryError> {
        let mut inner = self.inner.lock().expect("memory orchestrator poisoned");
        let record = inner
            .allocations
            .remove(&id)
            .ok_or(MemoryError::UnknownAllocation(id))?;
        inner.live_bytes = inner.live_bytes.saturating_sub(record.layout.size());
        let domain_record = inner
            .domains
            .get_mut(&record.domain_id)
            .ok_or(MemoryError::UnknownDomain(record.domain_id))?;
        domain_record.stats.live_allocations =
            domain_record.stats.live_allocations.saturating_sub(1);
        domain_record.stats.live_bytes = domain_record
            .stats
            .live_bytes
            .saturating_sub(record.layout.size());
        let domain = MemoryDomain {
            id: record.domain_id,
            trace_id: domain_record.trace_id,
            job_id: domain_record.job_id,
            label: domain_record.label,
        };
        drop(inner);
        // SAFETY: `canonical_ptr` was returned by `alloc` with this exact layout and
        // the record is removed before deallocation, preventing a second free by id.
        unsafe {
            dealloc(record.canonical_ptr.as_ptr(), record.layout);
        }
        self.emit(
            domain,
            Some(id),
            TraceEventKind::Free,
            Some(record.kind),
            record.layout.size(),
            record.layout.align(),
            record.label,
            None,
        );
        Ok(())
    }
}

impl Drop for MemoryOrchestrator {
    fn drop(&mut self) {
        let Ok(inner) = self.inner.get_mut() else {
            return;
        };
        for (_, record) in inner.allocations.drain() {
            // SAFETY: these are still-live records created by `alloc` with the same layout.
            unsafe {
                dealloc(record.canonical_ptr.as_ptr(), record.layout);
            }
        }
        inner.live_bytes = 0;
    }
}
