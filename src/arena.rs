use crate::{
    MemoryBlock, MemoryDomain, MemoryError, MemoryKind, MemoryOrchestrator, TraceEventKind,
};
use std::alloc::Layout;
use std::ptr::{self, NonNull};
use std::sync::Arc;

struct DropRecord {
    ptr: *mut u8,
    drop_fn: unsafe fn(*mut u8),
}
unsafe fn drop_impl<T>(ptr: *mut u8) {
    unsafe { ptr::drop_in_place(ptr.cast::<T>()) }
}
fn align_up(value: usize, align: usize) -> usize {
    (value + (align - 1)) & !(align - 1)
}

pub struct Arena {
    orchestrator: Arc<MemoryOrchestrator>,
    domain: MemoryDomain,
    block: MemoryBlock,
    cursor: usize,
    drops: Vec<DropRecord>,
}
impl Arena {
    pub fn new_in(
        orchestrator: Arc<MemoryOrchestrator>,
        domain: MemoryDomain,
        size: usize,
        align: usize,
        label: &'static str,
    ) -> Result<Self, MemoryError> {
        let block =
            orchestrator.alloc_aligned_bytes_in(domain, size, align, MemoryKind::Arena, label)?;
        Ok(Self {
            orchestrator,
            domain,
            block,
            cursor: 0,
            drops: Vec::new(),
        })
    }
    pub fn capacity(&self) -> usize {
        self.block.layout.size()
    }
    pub fn used(&self) -> usize {
        self.cursor
    }
    pub fn remaining(&self) -> usize {
        self.capacity().saturating_sub(self.cursor)
    }
    pub fn alloc<T>(&mut self, value: T) -> Result<NonNull<T>, MemoryError> {
        if std::mem::size_of::<T>() == 0 {
            return Err(MemoryError::ZeroSizedAllocation);
        }
        let ptr = self.alloc_layout(Layout::new::<T>())?.cast::<T>();
        // SAFETY: the returned region is properly aligned, large enough for T and uninitialized.
        unsafe {
            ptr.as_ptr().write(value);
        }
        self.drops.push(DropRecord {
            ptr: ptr.as_ptr().cast::<u8>(),
            drop_fn: drop_impl::<T>,
        });
        Ok(ptr)
    }
    pub fn alloc_bytes(&mut self, size: usize, align: usize) -> Result<NonNull<u8>, MemoryError> {
        if size == 0 {
            return Err(MemoryError::ZeroSizedAllocation);
        }
        self.alloc_layout(Layout::from_size_align(size, align).map_err(|_| MemoryError::Layout)?)
    }
    fn alloc_layout(&mut self, layout: Layout) -> Result<NonNull<u8>, MemoryError> {
        let base_ptr = self.block.ptr.as_ptr();
        let base_addr = base_ptr.addr();
        let current_addr = base_addr
            .checked_add(self.cursor)
            .ok_or(MemoryError::Layout)?;
        let start_addr = align_up(current_addr, layout.align());
        let start_offset = start_addr
            .checked_sub(base_addr)
            .ok_or(MemoryError::Layout)?;
        let used = start_offset
            .checked_add(layout.size())
            .ok_or(MemoryError::Layout)?;
        if used > self.capacity() {
            return Err(MemoryError::ArenaOutOfSpace {
                requested: layout.size(),
                remaining: self.remaining(),
            });
        }
        self.cursor = used;
        // SAFETY: start_offset is checked to lie inside the owned allocation and
        // the requested layout fits entirely within the arena capacity. Pointer
        // arithmetic therefore preserves the original allocation provenance.
        let ptr = unsafe { base_ptr.add(start_offset) };
        NonNull::new(ptr).ok_or(MemoryError::Layout)
    }
    pub fn reset(&mut self) {
        for rec in self.drops.drain(..).rev() {
            // SAFETY: each record corresponds to one initialized value not previously dropped.
            unsafe { (rec.drop_fn)(rec.ptr) };
        }
        self.cursor = 0;
        self.orchestrator.emit_event(
            self.domain,
            TraceEventKind::ArenaReset,
            self.block.label,
            Some("arena_reset"),
        );
    }
}
impl Drop for Arena {
    fn drop(&mut self) {
        self.reset();
        let _ = self.orchestrator.free_block(self.block.id);
    }
}
