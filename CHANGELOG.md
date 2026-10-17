# Changelog

## 0.2.0 (2026-10-16)

The V13.1 source (0.1.0 before), formatted with `cargo fmt`. CI: `.github/workflows/ci.yml` runs
the tests, the release-mode hardening test, rustfmt, the package build and Miri (strict provenance
included), and publishes on a `v*` tag.

- Added tracked memory domains and per-domain/global allocation accounting.
- Added `RawBuf`, `Arena`, `PagePool`, page leases, and circular page buffering.
- Added allocation lifecycle trace sinks/events.
- Added optional `paging` value types.
- Added churn, destructor, reclamation, page-reuse, and parallel bookkeeping tests.
