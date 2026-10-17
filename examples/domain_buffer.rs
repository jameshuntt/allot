use allot::{MemoryOrchestrator, RawBuf};
use std::sync::Arc;

fn main() -> Result<(), allot::MemoryError> {
    let memory = Arc::new(MemoryOrchestrator::new());
    let domain = memory.create_domain("request", None, None);

    {
        let mut values = RawBuf::new_in(memory.clone(), domain, "values");
        values.push(10u32)?;
        values.push(20u32)?;
        assert_eq!(values.as_slice(), &[10, 20]);
        assert_eq!(memory.domain_stats(domain.id)?.live_allocations, 1);
    }

    assert_eq!(memory.stats().live_allocations, 0);
    Ok(())
}
