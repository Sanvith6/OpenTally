# Contributing to OpenTally

Thank you for contributing to OpenTally. OpenTally is developed under strict clean-room engineering standards to maintain independent authorship.

## Clean-Room Architectural Boundaries

1. **Domain Isolation**:
   - `01-clean-room-specs/`: Protocol and behavioral specifications.
   - `02-open-tally/`: Clean-room implementation in Rust and TypeScript/React.
2. **Implementation Hygiene**:
   - Never copy, decompile, or incorporate proprietary code, database binaries, or proprietary assets into `02-open-tally/`.
   - All accounting and business logic must be derived from open specifications and tested using independent test vectors.

## Core Engineering Standards

- **Zero Floating-Point Arithmetic**: All monetary quantities must use `rust_decimal` in Rust and integer minor units or canonical decimal strings in TypeScript.
- **MCA Audit Compliance**: All modifications to financial transactions must preserve the immutable `audit_log` trail.
- **Tenant Isolation**: All database operations must be strictly partitioned by `company_id`.

## Building and Testing

### Prerequisites
- Rust 1.78+ (2021 edition)
- Node.js 18+ and npm
- SQLite 3

### Backend Verification
```bash
cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml
cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml -- -D warnings
cargo fmt --manifest-path 02-open-tally/src-tauri/Cargo.toml --check
```

### Frontend Verification
```bash
npm --prefix 02-open-tally/frontend test -- --run
npm --prefix 02-open-tally/frontend run build
```

### Packaging Desktop Binary
```bash
cargo build --release --manifest-path 02-open-tally/src-tauri/Cargo.toml
```

## Pull Request Guidelines
- All PRs must pass automated unit and integration tests with zero clippy warnings.
- Keep commits focused and descriptive, following conventional commit style.
