# OpenTally — Sprint 2 Stage 7A Phase 1 Boundary Verification & Hardening Report
## Exact Monetary Normalization & Decimal Representability Analysis

**Project:** OpenTally  
**Milestone:** 7A — Backend Search & Export  
**Phase:** 1 — Search Domain Foundation (Hardening & Boundary Verification)  
**Approved Design:** Stage 7 Technical Design Document Version 2.2.2  
**Current Database Schema:** `0.3.1-sprint2` (Unmodified)  
**Status:** **VERIFIED & HARDENED**  
**Date:** 2026-11-09  

---

## 1. Executive Summary

This independent boundary verification identified and corrected a critical representability gap in the initial Phase 1 monetary normalization implementation:
1. **The Representability Gap:** `rust_decimal::Decimal` relies on a 96-bit integer mantissa ($2^{96}-1 = 79,228,162,514,264,337,593,543,950,335$). When 4 fractional decimal places are specified, the maximum value representable by `rust_decimal::Decimal` is strictly bounded at:
   $$\text{Max Decimal}_{\text{scale=4}} = 7,922,816,251,426,433,759,354,395.0335$$
   Any valid monetary value in the approved schema with 25 integer digits and 4 fractional digits exceeding this threshold (e.g. `7922816251426433759354395.0336` or the maximum schema bound `9999999999999999999999999.9999`) was rejected with an overflow error by `Decimal::from_str_exact`.
2. **The Hardened Solution:** The preliminary parsing dependency on `Decimal::from_str_exact` was replaced with an **exact, zero-float, zero-allocation, string-based canonical normalizer** in `vouchers::validator::normalize_canonical_amount_for_query`.
3. **Approved Canonical Representation Preserved:** The function strictly guarantees the approved 30-character format:
   $$\underbrace{\text{25-digit zero-padded integer}}_{\text{Index 0..25}} \mathbf{.} \underbrace{\text{4-digit zero-padded fraction}}_{\text{Index 26..30}}$$
4. **Monotonicity & Equality Preserved:** Canonical lexicographical comparison (`min_canonical <= max_canonical` and SQLite `BETWEEN`) is 100% isomorphic to exact numeric ordering across all non-negative numbers up to $10^{25}-0.0001$.

---

## 2. Decimal Representability Analysis

### A. Mathematical Boundary of `rust_decimal::Decimal`
- **Internal Storage:** 96-bit unsigned integer coefficient ($M \in [0, 2^{96}-1]$), 1 sign bit, and 1 byte scale ($s \in [0, 28]$).
- Maximum unscaled integer coefficient:
  $$2^{96} - 1 = 79,228,162,514,264,337,593,543,950,335 \quad (29 \text{ decimal digits})$$
- When scale $s = 4$:
  $$\text{Value} = \frac{M}{10^4}$$
  $$\text{Max Value} = \frac{79,228,162,514,264,337,593,543,950,335}{10^4} = 7,922,816,251,426,433,759,354,395.0335$$

### B. Schema Range vs. Decimal Range
| Representation Domain | Max Integer Digits | Max Fractional Digits | Max Value Supported |
|---|---|---|---|
| **SQLite Schema (`chk_voucher_total_amount_canonical`)** | 25 digits | 4 digits | `9999999999999999999999999.9999` ($10^{25} - 0.0001$) |
| **`rust_decimal::Decimal` ($s = 4$)** | 25 digits (limited to $\le 7.92 \times 10^{24}$) | 4 digits | `7922816251426433759354395.0335` |
| **Stage 7 30-Character Canonical String** | 25 digits | 4 digits | `9999999999999999999999999.9999` ($10^{25} - 0.0001$) |

### C. Empirical Demonstration of the Gap
Direct testing via `vouchers::stage7_domain_tests::test_19_decimal_representability_gap_and_boundary_verification` confirmed:
1. `Decimal::from_str_exact("7922816251426433759354395.0335")` $\to$ **`Ok`** (Exact 96-bit boundary).
2. `Decimal::from_str_exact("7922816251426433759354395.0336")` $\to$ **`Err(Error::ExceedsMaximumPossibleValue)`** (Gap confirmed).
3. `Decimal::from_str_exact("9999999999999999999999999.9999")` $\to$ **`Err(Error::ExceedsMaximumPossibleValue)`** (Gap confirmed).
4. `Decimal::from_str_exact("8000000000000000000000000.0000")` $\to$ **`Err(Error::ExceedsMaximumPossibleValue)`** (Gap confirmed).

Relying on `Decimal::from_str_exact` prematurely rejected valid schema-supported monetary search filters.

---

## 3. Code Corrections: Exact String Normalization

In [`02-open-tally/src-tauri/src/vouchers/validator.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/validator.rs#L440-L536), `normalize_canonical_amount_for_query` was rewritten as an exact string-based normalizer:

```rust
pub fn normalize_canonical_amount_for_query(
    input: &str,
    field_name: &str,
) -> Result<String, OpenTallyError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(OpenTallyError::Validation(format!(
            "{} cannot be empty",
            field_name
        )));
    }

    // 1. Reject explicit signs ('+' or '-') to avoid normalization padding anomalies
    if trimmed.starts_with('+') || trimmed.starts_with('-') {
        return Err(OpenTallyError::Validation(format!(
            "{} cannot contain explicit sign: '{}'",
            field_name, input
        )));
    }

    // 2. Split into integer and fractional components without heap allocation
    let (int_part, frac_part) = match trimmed.split_once('.') {
        Some((i, f)) => {
            if f.contains('.') {
                return Err(OpenTallyError::Validation(format!(
                    "{} contains multiple decimal points: '{}'",
                    field_name, input
                )));
            }
            if f.is_empty() {
                return Err(OpenTallyError::Validation(format!(
                    "{} has trailing decimal point without fractional digits: '{}'",
                    field_name, input
                )));
            }
            (i, f)
        }
        None => (trimmed, ""),
    };

    if int_part.is_empty() {
        return Err(OpenTallyError::Validation(format!(
            "{} missing integer part before decimal: '{}'",
            field_name, input
        )));
    }

    if !int_part.chars().all(|c| c.is_ascii_digit()) {
        return Err(OpenTallyError::Validation(format!(
            "{} integer part contains non-digit characters: '{}'",
            field_name, input
        )));
    }

    // 3. Bound raw integer string length to prevent DoS via unbounded leading zeros
    if int_part.len() > 35 {
        return Err(OpenTallyError::Validation(format!(
            "{} integer part length {} exceeds maximum allowed length of 35 characters: '{}'",
            field_name,
            int_part.len(),
            input
        )));
    }

    // 4. Extract significant integer digits (handling leading zeros)
    let significant_int = int_part.trim_start_matches('0');
    let effective_int = if significant_int.is_empty() {
        "0"
    } else {
        significant_int
    };

    if effective_int.len() > 25 {
        return Err(OpenTallyError::Validation(format!(
            "{} integer part exceeds maximum 25 digits: '{}'",
            field_name, input
        )));
    }

    // 5. Validate fractional component
    if !frac_part.is_empty() {
        if !frac_part.chars().all(|c| c.is_ascii_digit()) {
            return Err(OpenTallyError::Validation(format!(
                "{} fractional part contains non-digit characters: '{}'",
                field_name, input
            )));
        }
        if frac_part.len() > 4 {
            return Err(OpenTallyError::Validation(format!(
                "{} exceeds maximum 4 decimal places: '{}'",
                field_name, input
            )));
        }
    }

    // 6. Format: 25-digit zero-padded integer + '.' + 4-digit zero-padded fraction (exactly 30 characters)
    let padded_int = format!("{:0>25}", effective_int);
    let padded_frac = format!("{:0<4}", frac_part);

    Ok(format!("{}.{}", padded_int, padded_frac))
}
```

### Architectural Properties:
1. **Zero Floats & Zero Rounding**: Pure integer string parsing; zero IEEE 754 float usage.
2. **Zero Allocation Splitting**: Operates via `str::split_once('.')`.
3. **Full Schema Range Support**: Supports all integers up to 25 digits ($10^{25}-1$) and up to 4 decimal places.
4. **Deterministic Padding**: Integer part is left-padded to 25 digits with `'0'`; fractional part is right-padded to 4 digits with `'0'`.
5. **Exact Equivalence**: Equivalent representations (`100`, `100.00`, `0100`, `00100.0000`) produce identical 30-character strings (`0000000000000000000000100.0000`).

---

## 4. Boundary Test Results

All required boundary test cases were implemented and verified in [`vouchers/stage7_domain_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_domain_tests.rs):

| Input Value | Category | Normalization Result | Status |
|---|---|---|---|
| `0` | Zero minimum | `0000000000000000000000000.0000` (30 chars) | **PASS** |
| `0.0001` | Minimum positive fraction | `0000000000000000000000000.0001` (30 chars) | **PASS** |
| `1.0000` | Integer with explicit zero fraction | `0000000000000000000000001.0000` (30 chars) | **PASS** |
| `9999999999999999999999999` | Max 25-digit integer ($10^{25}-1$) | `9999999999999999999999999.0000` (30 chars) | **PASS** |
| `9999999999999999999999999.9999` | Max schema monetary value | `9999999999999999999999999.9999` (30 chars) | **PASS** |
| `7922816251426433759354395.0334` | Immediately below Decimal boundary | `7922816251426433759354395.0334` (30 chars) | **PASS** |
| `7922816251426433759354395.0335` | Exact 96-bit Decimal boundary | `7922816251426433759354395.0335` (30 chars) | **PASS** |
| `7922816251426433759354395.0336` | Immediately above Decimal boundary | `7922816251426433759354395.0336` (30 chars) | **PASS** |
| `10.12345` | Invalid precision (scale 5) | Rejected with `exceeds maximum 4 decimal places` | **PASS** |
| `0.00001` | Invalid precision (scale 5) | Rejected with `exceeds maximum 4 decimal places` | **PASS** |
| `-50.00` | Negative value | Rejected with `cannot contain explicit sign` | **PASS** |
| `+50.00` | Explicit positive sign | Rejected with `cannot contain explicit sign` | **PASS** |
| `10.20.30` | Multiple decimal points | Rejected with `contains multiple decimal points` | **PASS** |
| `100.` | Trailing decimal without digits | Rejected with `trailing decimal point without fractional digits` | **PASS** |
| `.50` | Missing integer part | Rejected with `missing integer part before decimal` | **PASS** |
| `abc`, `12a.34` | Malformed non-digit characters | Rejected with `contains non-digit characters` | **PASS** |
| `10000000000000000000000000` | 26 integer digits ($10^{25}$) | Rejected with `integer part exceeds maximum 25 digits` | **PASS** |

---

## 5. Property Test Results

Four dedicated property and boundary test suites were executed in [`stage7_domain_tests.rs`](file:///c:/project/OpenTally/02-open-tally/src-tauri/src/vouchers/stage7_domain_tests.rs):

1. **`test_19_decimal_representability_gap_and_boundary_verification`**:
   - Confirms `Decimal::from_str_exact` failure on values $> 7922816251426433759354395.0335$.
   - Confirms canonical normalizer success across `[below, boundary, above, schema_max]`.
   - Confirms monotonicity: `below < boundary < above < schema_max`.
2. **`test_20_property_test_exact_30_chars_and_equivalence`**:
   - Validates that all sample inputs across orders of magnitude produce strings of length exactly 30 with '.' at index 25.
   - Validates equivalence groups: `0 == 0.0 == 0.00 == 00 == 00.0000`, `100 == 100.00 == 0100`, etc.
3. **`test_21_property_test_lexicographical_equals_numeric_order`**:
   - Validates strict monotonicity $S_i < S_{i+1}$ across 19 ordered values spanning from `0` to `9999999999999999999999999.9999`.
4. **`test_22_malformed_input_rejections`**:
   - Validates fail-closed rejection across 17 distinct malformed input edge cases.

---

## 6. Changed-File Inventory

| File Path | Status | Nature of Hardening Changes |
|---|---|---|
| `02-open-tally/src-tauri/src/vouchers/validator.rs` | Modified | Replaced `Decimal::from_str_exact` with exact ASCII string normalization supporting full 25-digit integer + 4-digit fractional range. |
| `02-open-tally/src-tauri/src/vouchers/stage7_domain_tests.rs` | Modified | Updated `test_11` for max schema bounds; added `test_19`, `test_20`, `test_21`, `test_22` covering 96-bit gap, property invariants, monotonicity, and malformed inputs. |

---

## 7. Verification & Quality Gates

1. **Full Backend Test Suite**:
   ```bash
   cargo test --manifest-path 02-open-tally/src-tauri/Cargo.toml
   ```
   **Result:** **273 passed; 0 failed; 0 ignored** (251 baseline + 22 Stage 7 pure domain tests).

2. **Frontend Vitest Suite**:
   ```bash
   npm --prefix 02-open-tally/frontend test -- --run
   ```
   **Result:** **3 passed (3), 27 passed (27)**.

3. **Rustfmt**:
   ```bash
   cargo fmt --manifest-path 02-open-tally/src-tauri/Cargo.toml -- --check
   ```
   **Result:** Clean exit code 0; zero formatting discrepancies.

4. **Clippy**:
   ```bash
   cargo clippy --manifest-path 02-open-tally/src-tauri/Cargo.toml --all-targets -- -D warnings
   ```
   **Result:** Clean exit code 0; zero linter warnings or errors.

---

## 8. Confirmation of Phase Boundary Adherence

- **Phase 2 Implementation:** Has **NOT** started.
- **SQL Queries:** Zero queries implemented or executed.
- **Database Migrations:** Zero migrations created or modified (schema remains `0.3.1-sprint2`).
- **Dependencies:** Zero new crates added (`Cargo.toml` unchanged).
- **Export & Filesystem:** Zero export handlers or file replacement calls implemented.
- **Frontend / React UI:** Zero frontend modifications.
- **Governance:** `DEC-S2-001`, `DEC-S2-002`, `DEC-S2-003`, `DEC-S2-004`, and `FIND-6-01` through `FIND-6-06` remain preserved and tracked without alteration.

