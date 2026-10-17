//! High-assurance memory allotment primitives.
//!
//! `allot` owns the concrete memory side of the ecosystem: domains, tracked
//! allocations, bump arenas, raw growable buffers, reusable page leases and
//! optional page-table value types. Scheduling and pointer dispatch stay in PETR.

#![deny(unsafe_op_in_unsafe_fn)]

mod arena;
mod orchestrator;
mod page_pool;
#[cfg(feature = "paging")]
pub mod paging;
mod rawbuf;
mod trace;
mod types;

pub use arena::Arena;
pub use orchestrator::MemoryOrchestrator;
pub use page_pool::{CircularPageBuffer, PAGE_SIZE, PageLease, PagePool};
/// Backward-compatible name from the temporal pointer prototype.
pub type CircularRealityBuffer = CircularPageBuffer;
pub use rawbuf::RawBuf;
pub use trace::{
    ChannelTraceSink, NoopTraceSink, SharedTraceSink, TraceEvent, TraceEventKind, TraceSink,
    VecTraceSink,
};
pub use types::{
    AllocationId, DomainStats, JobId, MemoryBlock, MemoryDomain, MemoryDomainId, MemoryError,
    MemoryKind, MemoryStats, TraceId, TypedHandle,
};
