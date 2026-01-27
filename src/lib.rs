//! # ALLOT
//! High-assurance resource portioning and scoped memory allotment.
//! 
//! This crate provides primitives for the formal allotment of storage and 
//! system resources within a defined `scope`. It ensures that every byte 
//! utilized by the "Honest" suite is explicitly assigned, tracked, and 
//! reclaimed according to deterministic lifetime policies.
//!
//! ## Core Security Principles
//! * **Proportionality:** Resources are allotted only in the exact quantity required.
//! * **Temporal Bounds:** Allotments are strictly tied to `scope` lifetimes.
//! * **Exclusivity:** Prevents overlapping allotments to maintain memory integrity.
//! * **Sanitization:** Integrated with `classified` to ensure allotted space 
//!   is scrubbed upon reclamation.