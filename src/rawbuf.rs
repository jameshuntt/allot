use crate::{
    AllocationId, MemoryDomain, MemoryError, MemoryKind, MemoryOrchestrator, TraceEventKind,
};
use std::mem::MaybeUninit;
use std::ptr::{self, NonNull};
use std::slice;
use std::sync::Arc;

pub struct RawBuf<T> {
    orchestrator: Arc<MemoryOrchestrator>,
    domain: MemoryDomain,
    ptr: NonNull<MaybeUninit<T>>,
    cap: usize,
    len: usize,
    alloc_id: Option<AllocationId>,
    label: &'static str,
}
impl<T> RawBuf<T> {
    pub fn new_in(
        orchestrator: Arc<MemoryOrchestrator>,
        domain: MemoryDomain,
        label: &'static str,
    ) -> Self {
        Self {
            orchestrator,
            domain,
            ptr: NonNull::dangling(),
            cap: 0,
            len: 0,
            alloc_id: None,
            label,
        }
    }
    pub fn with_capacity_in(
        orchestrator: Arc<MemoryOrchestrator>,
        domain: MemoryDomain,
        capacity: usize,
        label: &'static str,
    ) -> Result<Self, MemoryError> {
        if std::mem::size_of::<T>() == 0 {
            return Err(MemoryError::ZeroSizedAllocation);
        }
        if capacity == 0 {
            return Ok(Self::new_in(orchestrator, domain, label));
        }
        let handle = orchestrator.alloc_array_uninit_in::<MaybeUninit<T>>(
            domain,
            capacity,
            MemoryKind::Heap,
            label,
        )?;
        Ok(Self {
            orchestrator,
            domain,
            ptr: handle.ptr,
            cap: capacity,
            len: 0,
            alloc_id: Some(handle.id),
            label,
        })
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn capacity(&self) -> usize {
        self.cap
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn push(&mut self, value: T) -> Result<(), MemoryError> {
        if std::mem::size_of::<T>() == 0 {
            return Err(MemoryError::ZeroSizedAllocation);
        }
        if self.len == self.cap {
            self.grow()?;
        }
        // SAFETY: capacity is ensured and slot len is currently uninitialized.
        unsafe {
            self.ptr
                .as_ptr()
                .add(self.len)
                .write(MaybeUninit::new(value));
        }
        self.len += 1;
        Ok(())
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        // SAFETY: indices below len-before-decrement are initialized exactly once.
        unsafe { Some(self.ptr.as_ptr().add(self.len).read().assume_init()) }
    }
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: first len entries are initialized and live for self.
        unsafe { slice::from_raw_parts(self.ptr.cast::<T>().as_ptr(), self.len) }
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: first len entries are initialized and self is exclusively borrowed.
        unsafe { slice::from_raw_parts_mut(self.ptr.cast::<T>().as_ptr(), self.len) }
    }
    fn grow(&mut self) -> Result<(), MemoryError> {
        let new_cap = if self.cap == 0 {
            4
        } else {
            self.cap.checked_mul(2).ok_or(MemoryError::Layout)?
        };
        let new_handle = self.orchestrator.alloc_array_uninit_in::<MaybeUninit<T>>(
            self.domain,
            new_cap,
            MemoryKind::Heap,
            self.label,
        )?;
        // SAFETY: both allocations are valid for len elements and non-overlapping.
        unsafe {
            if self.len != 0 {
                ptr::copy_nonoverlapping(self.ptr.as_ptr(), new_handle.ptr.as_ptr(), self.len);
            }
        }
        if let Some(id) = self.alloc_id.take() {
            self.orchestrator.free_block(id)?;
        }
        self.ptr = new_handle.ptr;
        self.cap = new_cap;
        self.alloc_id = Some(new_handle.id);
        self.orchestrator.emit_event(
            self.domain,
            TraceEventKind::RawBufGrow,
            self.label,
            Some("rawbuf_grow"),
        );
        Ok(())
    }
}
impl<T> Drop for RawBuf<T> {
    fn drop(&mut self) {
        if self.len != 0 {
            // SAFETY: first len entries are initialized; dropping slice drops each once.
            unsafe {
                ptr::drop_in_place(slice::from_raw_parts_mut(
                    self.ptr.cast::<T>().as_ptr(),
                    self.len,
                ));
            }
        }
        if let Some(id) = self.alloc_id.take() {
            let _ = self.orchestrator.free_block(id);
        }
    }
}
