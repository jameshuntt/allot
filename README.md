# allot

Concrete, lifetime-oriented memory allotment primitives for the Honest/PETR ecosystem.

`allot` owns the concrete memory side of the system: tracked memory domains,
raw growable buffers, bump arenas, reusable 4 KiB page leases, allocation trace
events, and optional paging value types. It deliberately does **not** own task
scheduling, temporal eligibility, CPU placement, pointer dispatch, or secret
classification.

## Ownership boundary

| Concern | Owner |
|---|---|
| tracked allocation / reclamation | `allot` |
| memory domains and accounting | `allot` |
| bump arenas / raw buffers / reusable pages | `allot` |
| page-table value types | `allot` behind `paging` |
| tagged-pointer policy | PETR |
| temporal admission / CPU placement | PETR execution crates |
| scoped task lifetime | `scope` |
| classified / zeroized secret containers | `classified` |

## Example

```rust
use allot::{MemoryOrchestrator, RawBuf};
use std::sync::Arc;

let memory = Arc::new(MemoryOrchestrator::new());
let domain = memory.create_domain("request", None, None);

{
    let mut values = RawBuf::new_in(memory.clone(), domain, "values");
    values.push(10u32)?;
    values.push(20u32)?;
    assert_eq!(values.as_slice(), &[10, 20]);
}

assert_eq!(memory.stats().live_allocations, 0);
# Ok::<(), allot::MemoryError>(())
```

A runnable version is in `examples/domain_buffer.rs`.

## Features

`allot` has no default features. Enable `paging` for the optional page-table
value types:

```toml
allot = { version = "0.1", features = ["paging"] }
```

## Safety posture

The public API keeps allocation identity separate from raw pointer access.
`RawBuf`, `Arena`, and `PagePool` own the unsafe allocation mechanics and pair
allocation/reclamation through `MemoryOrchestrator`. The hardening suite covers
repeated growth and pop cycles, bounded page reuse, destructor behavior, and
parallel allocation/free bookkeeping.

`allot` is a memory primitive crate, not a general allocator replacement and
not a hard real-time allocator.
