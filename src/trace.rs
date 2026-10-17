use crate::{AllocationId, JobId, MemoryDomainId, MemoryKind, TraceId};
use std::sync::{Arc, Mutex, mpsc::Sender};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceEventKind {
    Alloc,
    Free,
    RawBufGrow,
    ArenaReset,
    PageAcquire,
    PageRelease,
    Custom,
}

#[derive(Debug, Clone)]
pub struct TraceEvent {
    pub at: Instant,
    pub trace_id: Option<TraceId>,
    pub job_id: Option<JobId>,
    pub domain_id: MemoryDomainId,
    pub domain_label: &'static str,
    pub alloc_id: Option<AllocationId>,
    pub kind: TraceEventKind,
    pub memory_kind: Option<MemoryKind>,
    pub size: usize,
    pub align: usize,
    pub label: &'static str,
    pub detail: Option<&'static str>,
}

pub trait TraceSink: Send + Sync + 'static {
    fn record(&self, event: TraceEvent);
}
#[derive(Default)]
pub struct NoopTraceSink;
impl TraceSink for NoopTraceSink {
    fn record(&self, _event: TraceEvent) {}
}

pub struct ChannelTraceSink {
    tx: Mutex<Sender<TraceEvent>>,
}
impl ChannelTraceSink {
    pub fn new(tx: Sender<TraceEvent>) -> Self {
        Self { tx: Mutex::new(tx) }
    }
}
impl TraceSink for ChannelTraceSink {
    fn record(&self, event: TraceEvent) {
        let _ = self.tx.lock().expect("trace sink poisoned").send(event);
    }
}

#[derive(Default)]
pub struct VecTraceSink {
    pub events: Mutex<Vec<TraceEvent>>,
}
impl TraceSink for VecTraceSink {
    fn record(&self, event: TraceEvent) {
        self.events.lock().expect("trace sink poisoned").push(event);
    }
}
pub type SharedTraceSink = Arc<dyn TraceSink>;
