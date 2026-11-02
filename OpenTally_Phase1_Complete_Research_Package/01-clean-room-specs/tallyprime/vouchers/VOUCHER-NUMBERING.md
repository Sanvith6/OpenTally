# OpenTally Voucher Numbering Specification

## 1. Voucher Numbering Methods
In TallyPrime, each Voucher Type can be independently configured with one of five numbering methods:

1. **Automatic**:
   - The engine generates sequential integers starting from 1 (or configured start number).
   - If a voucher is deleted, downstream vouchers automatically renumber themselves to maintain a gapless sequence.
   - *Impact*: In audited environments, automatic renumbering of past vouchers causes legal non-compliance because issued invoice numbers shift.
2. **Automatic (Manual Override)**:
   - System proposes the next sequential integer, but the user is permitted to edit the number.
   - Prevents downstream shifting if earlier vouchers are altered.
3. **Manual**:
   - The user must enter the voucher number explicitly on every entry.
   - Option: `Prevent duplicates = Yes/No`. If Yes, the engine rejects committing a voucher with an existing number in the same voucher type and period.
4. **Multi-User Automatic**:
   - Allocates the next available number at the instant of saving rather than upon opening the entry screen, preventing numbering collisions across simultaneous network users.
5. **None**:
   - Vouchers have no voucher number recorded.

## 2. Advanced Numbering Configuration Parameters
For each voucher type, an advanced numbering series can be defined:
- **Starting Number**: Integer (default `1`).
- **Width of Numerical Part**: Number of digits (e.g., `4` pads `1` to `0001`).
- **Prefill with Zero**: Boolean (`Yes` -> `0001`, `No` -> `1`).
- **Restart Periods**:
  - `Daily`: Number resets to 1 each day.
  - `Weekly`: Resets each week.
  - `Monthly`: Resets on 1st of each calendar month.
  - `Never`: Sequence runs indefinitely.
  - `Yearly`: Resets on the first day of each financial year (e.g., 01-Apr).
- **Prefix Details**:
  - Configurable string attached before the numeric part with effective date (e.g., `NTE/24-25/`).
- **Suffix Details**:
  - Configurable string attached after the numeric part with effective date (e.g., `/HO`).
- **Composed Number Formula**: `VoucherNumber = Prefix + ZeroPadded(SequentialNumber, Width) + Suffix`. E.g., `NTE/24-25/0042/HO`.
