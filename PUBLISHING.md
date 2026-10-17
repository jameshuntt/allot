# Publishing allot

`allot` is released independently from PETR. It has no PETR runtime dependency.

Before publication, the ecosystem release gate runs its all-feature/all-target
tests and constructs its package:

```bash
cargo test --all-features --all-targets
cargo package --allow-dirty --no-verify
```

For an actual release, choose the public version explicitly and then perform the
normal crates.io dry-run/publish from the `allot` repository. The integrated
bundle never publishes automatically.
