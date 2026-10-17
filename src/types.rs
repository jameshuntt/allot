use std::alloc::Layout;
use std::marker::PhantomData;
use std::ptr::NonNull;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationId(pub usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemoryDomainId(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceId(pub u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    Heap,
    Page,
    Arena,
    Scratch,
    Slab,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryDomain {
    pub id: MemoryDomainId,
    pub trace_id: Option<TraceId>,
    pub job_id: Option<JobId>,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryBlock {
    pub id: AllocationId,
    pub domain_id: MemoryDomainId,
    pub ptr: NonNull<u8>,
    pub layout: Layout,
    pub kind: MemoryKind,
    pub label: &'static str,
}

impl MemoryBlock {
    #[inline]
    pub fn addr(&self) -> usize {
        self.ptr.as_ptr() as usize
    }
    #[inline]
    pub fn cast<T>(&self) -> NonNull<T> {
        self.ptr.cast()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypedHandle<T> {
    pub id: AllocationId,
    pub domain_id: MemoryDomainId,
    pub ptr: NonNull<T>,
    pub layout: Layout,
    pub kind: MemoryKind,
    pub label: &'static str,
    _marker: PhantomData<T>,
}

impl<T> TypedHandle<T> {
    pub(crate) fn new(block: MemoryBlock) -> Self {
        Self {
            id: block.id,
            domain_id: block.domain_id,
            ptr: block.cast(),
            layout: block.layout,
            kind: block.kind,
            label: block.label,
            _marker: PhantomData,
        }
    }
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        self.ptr.as_ptr()
    }
    /// # Safety
    /// The allocation must contain an initialized `T` and aliasing/lifetime rules must hold.
    pub unsafe fn as_ref<'a>(&self) -> &'a T {
        unsafe { self.ptr.as_ref() }
    }
    /// # Safety
    /// The allocation must contain an initialized `T` and unique access must hold.
    pub unsafe fn as_mut<'a>(&mut self) -> &'a mut T {
        unsafe { self.ptr.as_mut() }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MemoryStats {
    pub live_allocations: usize,
    pub live_bytes: usize,
    pub peak_bytes: usize,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DomainStats {
    pub live_allocations: usize,
    pub live_bytes: usize,
    pub peak_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    Layout,
    ZeroSizedAllocation,
    AllocFailed { size: usize, align: usize },
    UnknownAllocation(AllocationId),
    UnknownDomain(MemoryDomainId),
    ArenaOutOfSpace { requested: usize, remaining: usize },
    PageFrameTooLarge { provided: usize, capacity: usize },
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Layout => write!(f, "invalid allocation layout"),
            Self::ZeroSizedAllocation => write!(f, "zero-sized raw allocations are not supported"),
            Self::AllocFailed { size, align } => {
                write!(f, "allocation failed: size={size}, align={align}")
            }
            Self::UnknownAllocation(id) => write!(f, "unknown allocation id: {id:?}"),
            Self::UnknownDomain(id) => write!(f, "unknown domain id: {id:?}"),
            Self::ArenaOutOfSpace {
                requested,
                remaining,
            } => write!(
                f,
                "arena out of space: requested {requested} bytes, remaining {remaining} bytes"
            ),
            Self::PageFrameTooLarge { provided, capacity } => write!(
                f,
                "page frame too large: provided {provided} bytes, capacity {capacity} bytes"
            ),
        }
    }
}
impl std::error::Error for MemoryError {}
