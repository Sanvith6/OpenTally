## Summary
Describe the changes implemented in `02-open-tally/` (or CI/docs under `.github/`).

## Type of Change
- [ ] Bug fix
- [ ] New accounting / statutory / UI feature
- [ ] Refactor / performance improvement
- [ ] CI/CD, build, or documentation update

## Verification Checklist
- [ ] `cargo fmt --manifest-path 02-open-tally/src-tauri/Cargo.toml --check` passes
- [ ] `cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml -- -D warnings` passes
- [ ] `cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml` passes (all tests green, >=80% coverage on new code)
- [ ] `npm --prefix 02-open-tally/frontend test -- --run` passes
- [ ] `npm --prefix 02-open-tally/frontend run build` succeeds cleanly

## Clean-Room & Accounting Integrity Attestation
- [ ] Work stays strictly inside `02-open-tally/` (and `.github/` where applicable); `01-clean-room-specs/` is untouched and never referenced in application source or release bundles
- [ ] **Zero proprietary code or assets:** No proprietary binaries, `.tsf` files, raw TDL, decompiled snippets, or copyrighted graphics/fonts
- [ ] **Zero floating-point monetary math:** Uses `rust_decimal` in Rust and integer minor units in TypeScript
- [ ] **MCA Audit Trail & Security:** Mutating operations go through `audit_log` SHA-256 chaining with `AppState::resolve_actor`, respect `PeriodLocked` & RBAC, and use `ReadOnlyConnectionGuard` for read-only reports
