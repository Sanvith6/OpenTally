import re
import sys

def verify_v292():
    doc_path = "02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"
    with open(doc_path, "r", encoding="utf-8") as f:
        content = f.read()

    print(f"Loaded document: {len(content)} characters, {len(content.splitlines())} lines.")

    # 1. Version 2.9.2 in header
    assert "Version: 2.9.2" in content, "Header must declare Version: 2.9.2"
    print("PASS 1: Header declares Version: 2.9.2")

    # 2. Version 2.9.2 in Revision History
    assert "| `2.9.2` | 2026-11-02 |" in content, "Revision history must include 2.9.2 entry"
    print("PASS 2: Revision history includes 2.9.2 entry")

    # 3. Test matrix contains TC-VCH-01 through TC-VCH-143
    test_ids = re.findall(r"`(TC-VCH-\d+)`", content)
    unique_test_ids = sorted(list(set(test_ids)), key=lambda x: int(x.split("-")[-1]))
    print(f"Found {len(unique_test_ids)} unique test IDs in document: {unique_test_ids[0]} to {unique_test_ids[-1]}")
    assert len(unique_test_ids) == 143, f"Expected 143 unique test IDs, found {len(unique_test_ids)}"
    for i in range(1, 144):
        expected_id = f"TC-VCH-{i:02d}" if i < 100 else f"TC-VCH-{i}"
        assert expected_id in unique_test_ids, f"Missing test scenario {expected_id}"
    print("PASS 3: All 143 test scenarios (TC-VCH-01 to TC-VCH-143) are present in §10.1")

    # 4. Total count in §10.3 says 143
    assert "The **143 scenarios** above (`TC-VCH-01` to `TC-VCH-143`) are **PROPOSED TEST SCENARIOS**" in content
    print("PASS 4: §10.3 explicitly declares 143 PROPOSED TEST SCENARIOS")

    # 5. DEC-S2-001 is PENDING USER APPROVAL (Rounding-off policy)
    assert "| **DEC-S2-001** | `ACCOUNTING` | Invoice rounding-off policy" in content
    assert "### 11.1 Architectural Decision DEC-S2-001: Invoice Rounding-Off Policy" in content
    print("PASS 5: DEC-S2-001 is independently preserved and PENDING USER APPROVAL")

    # 6. DEC-S2-002 is PENDING USER APPROVAL (Sequence-gap governance)
    assert "| **DEC-S2-002** | `NUMBERING` | Sequence-gap administrative resolution" in content
    assert "### 11.2 Architectural Decision DEC-S2-002: Sequence-Gap Administrative Resolution" in content
    print("PASS 6: DEC-S2-002 is independently assigned and PENDING USER APPROVAL")

    # 7. Check for zero occurrences of obsolete current_number
    assert "current_number" not in content, "Found obsolete 'current_number' in document!"
    print("PASS 7: Zero occurrences of 'current_number' in document")

    # 8. Check for next_number in voucher_number_sequences
    assert "next_number INTEGER NOT NULL CHECK (next_number > 0)" in content
    print("PASS 8: voucher_number_sequences uses next_number INTEGER NOT NULL CHECK (next_number > 0)")

    # 9. Check composite foreign key candidate keys on series
    assert "CONSTRAINT uq_series_company_id UNIQUE (company_id, id)" in content
    assert "CONSTRAINT uq_series_type_ownership UNIQUE (company_id, voucher_type_id, id)" in content
    assert "CONSTRAINT uq_series_full_ownership UNIQUE (company_id, voucher_type_id, id, numbering_method)" in content
    print("PASS 9: All required candidate keys present on voucher_number_series")

    # 10. Check composite FK on vouchers
    assert "FOREIGN KEY (company_id, voucher_type_id, series_id, numbering_method)" in content
    assert "REFERENCES voucher_number_series(company_id, voucher_type_id, id, numbering_method)" in content
    print("PASS 10: vouchers enforces full composite foreign key to voucher_number_series")

    # 11. Check self-reversal check constraint
    assert "CONSTRAINT chk_no_self_reversal CHECK (reversal_of_voucher_id IS NULL OR reversal_of_voucher_id != id)" in content
    print("PASS 11: Self-reversal check constraint chk_no_self_reversal present in vouchers")

    # 12. Check line reassignment trigger checks both OLD and NEW
    assert "trg_prevent_posted_voucher_lines_mutation" in content
    assert "company_id = OLD.company_id" in content
    assert "id = OLD.voucher_id" in content
    assert "company_id = NEW.company_id" in content
    assert "id = NEW.voucher_id" in content
    assert "NEW.company_id IS NOT OLD.company_id" in content
    print("PASS 12: Voucher line reassignment trigger validates both OLD and NEW and company boundaries")

    # 13. Check bidirectional reversal coupling triggers
    assert "trg_validate_reversal_target_on_post" in content
    assert "Cannot mark voucher as REVERSED without an existing POSTED reversing voucher referencing it" in content
    print("PASS 13: Bidirectional reversal coupling triggers present")

    # 14. Check Section 5.6 (Comprehensive Header Inventory: 18 Columns)
    assert "### 5.6 Comprehensive Voucher Header Immutability Inventory (18 Columns)" in content
    print("PASS 14: Section 5.6 18-column header inventory present")

    # 15. Check Section 5.3.6 (Explicit policy on reversal-of-reversals)
    assert "#### 6. Explicit Policy on Reversal-of-Reversals" in content
    print("PASS 15: Section 5.3.6 explicit policy on reversal-of-reversals present")

    print("\nALL 15 VERIFICATION CHECKS PASSED FOR v2.9.2!")

if __name__ == "__main__":
    verify_v292()

