use crate::{
    AllocationId, MemoryBlock, MemoryDomain, MemoryError, MemoryKind, MemoryOrchestrator,
    TraceEventKind,
};
use std::collections::HashMap;
use std::sync::Arc;

pub const PAGE_SIZE: usize = 4096;

pub struct PageLease {
    block: MemoryBlock,
}
impl PageLease {
    pub fn id(&self) -> AllocationId {
        self.block.id
    }
    pub fn as_ptr(&self) -> *mut u8 {
        self.block.ptr.as_ptr()
    }
    pub fn zero(&mut self) {
        unsafe {
            std::ptr::write_bytes(self.block.ptr.as_ptr(), 0, PAGE_SIZE);
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.block.ptr.as_ptr(), PAGE_SIZE) }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.block.ptr.as_ptr(), PAGE_SIZE) }
    }
}

pub struct PagePool {
    orchestrator: Arc<MemoryOrchestrator>,
    domain: MemoryDomain,
    free: Vec<MemoryBlock>,
    in_use: HashMap<AllocationId, MemoryBlock>,
    label: &'static str,
}
impl PagePool {
    pub fn new_in(
        orchestrator: Arc<MemoryOrchestrator>,
        domain: MemoryDomain,
        label: &'static str,
    ) -> Self {
        Self {
            orchestrator,
            domain,
            free: Vec::new(),
            in_use: HashMap::new(),
            label,
        }
    }
    pub fn warm(&mut self, count: usize) -> Result<(), MemoryError> {
        for _ in 0..count {
            self.free.push(self.orchestrator.alloc_aligned_bytes_in(
                self.domain,
                PAGE_SIZE,
                PAGE_SIZE,
                MemoryKind::Page,
                self.label,
            )?);
        }
        Ok(())
    }
    pub fn acquire(&mut self) -> Result<PageLease, MemoryError> {
        let block = if let Some(b) = self.free.pop() {
            b
        } else {
            self.orchestrator.alloc_aligned_bytes_in(
                self.domain,
                PAGE_SIZE,
                PAGE_SIZE,
                MemoryKind::Page,
                self.label,
            )?
        };
        self.in_use.insert(block.id, block);
        self.orchestrator.emit_event(
            self.domain,
            TraceEventKind::PageAcquire,
            self.label,
            Some("page_acquire"),
        );
        Ok(PageLease { block })
    }
    pub fn release(&mut self, lease: PageLease) {
        if let Some(block) = self.in_use.remove(&lease.block.id) {
            self.free.push(block);
            self.orchestrator.emit_event(
                self.domain,
                TraceEventKind::PageRelease,
                self.label,
                Some("page_release"),
            );
        }
    }
    pub fn free_pages(&self) -> usize {
        self.free.len()
    }
    pub fn in_use_pages(&self) -> usize {
        self.in_use.len()
    }
}
impl Drop for PagePool {
    fn drop(&mut self) {
        for (_, block) in self.in_use.drain() {
            let _ = self.orchestrator.free_block(block.id);
        }
        for block in self.free.drain(..) {
            let _ = self.orchestrator.free_block(block.id);
        }
    }
}

pub struct CircularPageBuffer {
    pool: PagePool,
    ring: Vec<PageLease>,
    write_index: usize,
    capacity: usize,
}
impl CircularPageBuffer {
    pub fn new(pool: PagePool, capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            pool,
            ring: Vec::with_capacity(capacity),
            write_index: 0,
            capacity,
        }
    }
    pub fn push_frame(&mut self, data: &[u8]) -> Result<(), MemoryError> {
        if data.len() > PAGE_SIZE {
            return Err(MemoryError::PageFrameTooLarge {
                provided: data.len(),
                capacity: PAGE_SIZE,
            });
        }
        let mut lease = self.pool.acquire()?;
        lease.as_bytes_mut()[..data.len()].copy_from_slice(data);
        if self.ring.len() >= self.capacity {
            let old = std::mem::replace(&mut self.ring[self.write_index], lease);
            self.pool.release(old);
        } else {
            self.ring.push(lease);
        }
        self.write_index = (self.write_index + 1) % self.capacity;
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.ring.len()
    }
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}
