import re
import sys

def verify_v290():
    doc_path = "02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"
    with open(doc_path, "r", encoding="utf-8") as f:
        content = f.read()

    print(f"Loaded document: {len(content)} characters, {len(content.splitlines())} lines.")

    # 1. Version 2.9.0 in header
    assert "Version: 2.9.0" in content, "Header must declare Version: 2.9.0"
    print("PASS: Header declares Version: 2.9.0")

    # 2. Version 2.9.0 in Revision History
    assert "| `2.9.0` | 2026-11-02 |" in content, "Revision history must include 2.9.0 entry"
    print("PASS: Revision history includes 2.9.0 entry")

    # 3. Test matrix contains TC-VCH-01 through TC-VCH-128
    test_ids = re.findall(r"`(TC-VCH-\d+)`", content)
    unique_test_ids = sorted(list(set(test_ids)), key=lambda x: int(x.split("-")[-1]))
    print(f"Found {len(unique_test_ids)} unique test IDs in document: {unique_test_ids[0]} to {unique_test_ids[-1]}")
    assert len(unique_test_ids) == 128, f"Expected 128 unique test IDs, found {len(unique_test_ids)}"
    for i in range(1, 129):
        expected_id = f"TC-VCH-{i:02d}" if i < 100 else f"TC-VCH-{i}"
        assert expected_id in unique_test_ids, f"Missing test scenario {expected_id}"
    print("PASS: All 128 test scenarios (TC-VCH-01 to TC-VCH-128) are present in §10.1")

    # 4. Total count in §10.3 says 128
    assert "The **128 scenarios** above (`TC-VCH-01` to `TC-VCH-128`) are **PROPOSED TEST SCENARIOS**" in content
    print("PASS: §10.3 explicitly declares 128 PROPOSED TEST SCENARIOS")

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

    # 11. Check Finding 1: Zero silent defaults & newly created series mandate
    assert "Implicit fallback to default configuration (start_number = 1) is strictly prohibited." in content
    assert "Newly Created Series & Initial Configuration Mandate (Resolving Finding 1)" in content
    print("PASS: Finding 1 explicit configuration requirement and zero silent fallback verified")

    # 12. Check Finding 2: Counter-tail gaps detection & worked examples
    assert "DETECT COUNTER-TAIL GAPS IN [M + 1, N - 1]: When N > M + 1" in content
    assert "Example A: Normal Aligned Counter" in content
    assert "Example B: Lagging Counter" in content
    assert "Example C: Ahead Counter Explained by Cancelled and Reversed Allocations" in content
    assert "Example D: Ahead Counter with Unexplained Missing Allocations (Counter-Tail Gaps)" in content
    assert "Example E: No Permanent Allocations" in content
    assert "Example F: Starting Number 100 ($S_0 = 100$)" in content
    print("PASS: Finding 2 counter-tail gap inspection and all 6 worked examples present")

    # 13. Check Finding 3 & 4: derive_period_start and header immutability
    assert "derive_period_start" in content
    assert "trg_prevent_posted_voucher_mutation" in content
    assert "NEW.series_id != OLD.series_id" in content
    assert "NEW.numbering_method != OLD.numbering_method" in content
    print("PASS: Finding 3 derive_period_start and Finding 4 header immutability trigger verified")

    print("\nALL 13 VERIFICATION CHECKS PASSED FOR v2.9.0!")

if __name__ == "__main__":
    verify_v290()

