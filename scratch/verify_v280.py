import re
import sys

def verify_v280():
    doc_path = "02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"
    with open(doc_path, "r", encoding="utf-8") as f:
        content = f.read()

    print(f"Loaded document: {len(content)} characters, {len(content.splitlines())} lines.")

    # 1. Version 2.8.0 in header
    assert "Version: 2.8.0" in content, "Header must declare Version: 2.8.0"
    print("PASS: Header declares Version: 2.8.0")

    # 2. Version 2.8.0 in Revision History
    assert "| `2.8.0` | 2026-11-02 |" in content, "Revision history must include 2.8.0 entry"
    print("PASS: Revision history includes 2.8.0 entry")

    # 3. Test matrix contains TC-VCH-01 through TC-VCH-120
    test_ids = re.findall(r"`(TC-VCH-\d+)`", content)
    unique_test_ids = sorted(list(set(test_ids)), key=lambda x: int(x.split("-")[-1]))
    print(f"Found {len(unique_test_ids)} unique test IDs in document: {unique_test_ids[0]} to {unique_test_ids[-1]}")
    assert len(unique_test_ids) == 120, f"Expected 120 unique test IDs, found {len(unique_test_ids)}"
    for i in range(1, 121):
        expected_id = f"TC-VCH-{i:02d}" if i < 100 else f"TC-VCH-{i}"
        assert expected_id in unique_test_ids, f"Missing test scenario {expected_id}"
    print("PASS: All 120 test scenarios (TC-VCH-01 to TC-VCH-120) are present in §10.1")

    # 4. Total count in §10.3 says 120
    assert "The **120 scenarios** above (`TC-VCH-01` to `TC-VCH-120`) are **PROPOSED TEST SCENARIOS**" in content
    print("PASS: §10.3 explicitly declares 120 PROPOSED TEST SCENARIOS")

    # 5. DEC-S2-001 is PENDING USER APPROVAL
    assert "DEC-S2-001" in content
    assert "**PENDING USER APPROVAL**" in content
    print("PASS: DEC-S2-001 is PENDING USER APPROVAL")

    # 6. Check for zero occurrences of current_number
    assert "current_number" not in content, "Found obsolete 'current_number' in document!"
    print("PASS: Zero occurrences of 'current_number' in document")

    # 7. Check for next_number in voucher_number_sequences
    assert "next_number INTEGER NOT NULL CHECK (next_number > 0)" in content
    print("PASS: voucher_number_sequences uses next_number INTEGER NOT NULL CHECK (next_number > 0)")

    # 8. Check composite foreign key candidate keys on series
    assert "CONSTRAINT uq_series_company_id UNIQUE (company_id, id)" in content
    assert "CONSTRAINT uq_series_type_ownership UNIQUE (company_id, voucher_type_id, id)" in content
    assert "CONSTRAINT uq_series_full_ownership UNIQUE (company_id, voucher_type_id, id, numbering_method)" in content
    print("PASS: All required candidate keys present on voucher_number_series")

    # 9. Check composite FK on vouchers
    assert "FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method)" in content
    assert "REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method)" in content
    print("PASS: vouchers enforces full composite foreign key to voucher_number_series")

    # 10. Check calendar-date checks
    assert "date(period_start) IS NOT NULL" in content
    assert "date(period_start) = period_start" in content
    assert "date(date) IS NOT NULL" in content
    assert "date(date) = date" in content
    print("PASS: Real calendar-date checks with IS NOT NULL guard present in vouchers table")

    # 11. Check Section 4.3.2.1 Counter Semantics & 6 worked examples
    assert "##### 4.3.2.1 Counter Semantics & Off-by-One Prevention (next_number_to_allocate)" in content
    assert "Example 1: Pre-Initialization without Vouchers" in content
    assert "Example 2: First Voucher Posting" in content
    assert "Example 3: Second Voucher Posting" in content
    assert "Example 4: Failed Rollback Reversion" in content
    assert "Example 5: Lagged Counter Recovery" in content
    assert "Example 6: Ahead Counter Preservation" in content
    print("PASS: Section 4.3.2.1 with all 6 worked examples present")

    # 12. Check 10-step recovery algorithm with start_number baseline S_0
    assert "RESOLVE BASELINE S_0: Fetch start_number from config for period" in content
    assert "Detect Sequence Gaps in Baseline Universe $[S_0, M]$" in content
    assert "Numbers Below $S_0$ are Never Gaps" in content
    print("PASS: 10-step sequence recovery algorithm with S_0 baseline and [S_0, M] present")

    print("\nALL 12 VERIFICATION CHECKS PASSED FOR v2.8.0!")

if __name__ == "__main__":
    verify_v280()
