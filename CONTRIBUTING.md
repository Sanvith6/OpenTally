# Contributing to OpenTally

Thank you for your interest in contributing to **OpenTally**! OpenTally is an open-source, high-performance accounting engine and desktop application built with Tauri 2, Rust, SQLite, and React.

To preserve independent authorship, copyright hygiene, and clean architectural integrity, all contributors must adhere to the engineering standards and clean-room policies outlined below.

---

## Clean-Room Software Engineering Policy

OpenTally is built using a strict **Clean-Room Engineering** methodology.

### Absolute Prohibitions
- **NO Proprietary Code or Artifacts**: You must **never** contribute proprietary executable binaries, proprietary database files (e.g. `.tsf` or company data directories), raw proprietary TDL source code, extracted graphics/fonts, or decompiled/disassembled code.
- **NO Unlicensed Third-Party Ports**: Do not copy, scrape, or port code from unlicensed third-party tools or repositories that reverse-engineer proprietary software.
- **Independent Authorship**: All clean-room implementation code in `02-open-tally/` must be authored independently based on written behavioral specifications in `01-clean-room-specs/` or standard double-entry accounting principles (ICAI / MCA / GAAP).

---

## Development Standards

### 1. Rust Engineering Standards
- **Idiomatic Rust**: Code must pass `cargo fmt --check` and `cargo clippy -- -D warnings`.
- **Exact Financial Arithmetic**: Always use `rust_decimal::Decimal` for monetary calculations. IEEE 754 floating-point types (`f32`, `f64`) are **strictly prohibited** for financial calculations and balances.
- **Explicit Error Handling**: Use typed errors via `thiserror` (e.g. `OpenTallyError`). Do not use unhandled `.unwrap()` or `.expect()` calls in production code paths.
- **Transactional Integrity**: Database operations that modify state must run within SQLite transactions (`BEGIN IMMEDIATE`) with atomic rollback on failure.

### 2. TypeScript & React Standards
- **Strict Typing**: TypeScript `strict` mode is enabled. Avoid `any`; use discriminating unions, interfaces, and branded types.
- **Component Modularity**: Keep UI components focused and cohesive (typically under 400 lines per file).
- **Keyboard-First Design**: Ensure financial forms and tables are accessible via keyboard hotkeys (`Tab`, `Enter`, `F2`–`F10`, `Escape`).

### 3. Test-Driven Development (TDD)
- **Minimum Coverage**: All new features and bug fixes must include automated tests targeting 80%+ test coverage.
- **Test Types**:
  - Unit tests for arithmetic and validation logic (`cargo test`).
  - Integration tests for database migrations, IPC commands, and multi-company isolation.
  - Component tests for UI views (`npm test` via Vitest).

---

## How to Submit a Contribution

### 1. Fork & Branch
1. Fork the repository on GitHub: `https://github.com/Sanvith6/OpenTally`
2. Clone your fork locally:
   ```bash
   git clone https://github.com/<your-username>/OpenTally.git
   cd OpenTally
   ```
3. Create a feature branch:
   ```bash
   git checkout -b feat/your-feature-name
   ```

### 2. Make Your Changes & Test
Ensure all automated tests pass before committing:
```bash
# Backend verification
cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml
cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml -- -D warnings

# Frontend verification
cd 02-open-tally/frontend
npm test
npm run build
cd ../..
```

### 3. Commit with Conventional Commits
Use standard Conventional Commits format:
```text
<type>(<scope>): <short description>

<optional body explaining the motivation and changes>
```
**Types**:
- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation changes
- `refactor`: Code refactoring without changing behavior
- `test`: Adding or correcting tests
- `chore`: Maintenance tasks, dependencies, tooling

### 4. Open a Pull Request
1. Push your branch to GitHub.
2. Open a Pull Request against the appropriate branch.
3. Include a clear description of the change, test evidence, and confirmation that no proprietary code or decompilations were used.
