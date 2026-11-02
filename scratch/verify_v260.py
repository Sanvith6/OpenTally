import re

def verify_v260():
    doc_path = "02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"
    with open(doc_path, "r", encoding="utf-8") as f:
        content = f.read()

    # 1. Version check
    assert "Document Version: 2.6.0" in content, "Document Version 2.6.0 missing"
    assert "| `2.6.0` |" in content, "Revision history entry for 2.6.0 missing"

    # 2. Schema DDL checks
    assert "series_code TEXT NOT NULL" in content, "series_code missing from voucher_number_configs"
    assert "CONSTRAINT uq_series_code UNIQUE (company_id, voucher_type_id, series_code)" in content, "uq_series_code constraint missing"
    assert "period_start TEXT" in content, "period_start missing from vouchers"
    assert "sequence_number INTEGER" in content, "sequence_number missing from vouchers"
    assert "CREATE UNIQUE INDEX uq_vouchers_series_sequence" in content, "uq_vouchers_series_sequence index missing"

    # 3. Check tests TC-VCH-01 to TC-VCH-102
    for i in range(1, 103):
        tc_id = f"TC-VCH-{i:02d}"
        assert f"`{tc_id}`" in content, f"Missing test scenario {tc_id}"

    # 4. Check Section 10.3 summary says 102 scenarios
    assert "The **102 scenarios** above (`TC-VCH-01` to `TC-VCH-102`) are **PROPOSED TEST SCENARIOS**" in content, "Section 10.3 does not state 102 scenarios"

    # 5. Check DEC-S2-001 status
    assert "DEC-S2-001" in content, "DEC-S2-001 missing"
    assert "PENDING USER APPROVAL" in content, "DEC-S2-001 not marked PENDING USER APPROVAL"

    # 6. Check Findings details
    # Finding 1: Bare {YYYY} rejection, evaluation from period_start
    assert "Yearly financial-year reset prohibits bare {YYYY} token" in content, "Bare {YYYY} prohibition missing"
    assert "strictly from `period_start`" in content, "Evaluation from period_start missing"

    # Finding 2: Formal EBNF grammar & series_code namespace isolation
    assert "VoucherNumber       ::= Prefix Number Suffix ;" in content, "EBNF grammar missing"
    assert "proof of mathematical non-overlap" in content.lower() or "\\mathcal{L}(S_1) \\cap \\mathcal{L}(S_2) = \\emptyset" in content, "Language disjointness missing"
    assert "validate_manual_voucher_number" in content, "validate_manual_voucher_number missing"

    # Finding 3: Strict rejection of static-only templates for periodic resets
    assert "Static-only templates are prohibited for periodic resets" in content, "Static template prohibition missing"
    assert "Static templates are strictly permitted for never-reset mode" in content or "permitted only for restart_period = 'never'" in content.lower(), "Static template permitted for never missing"

    # Finding 4: Sequence recovery algorithm
    assert "resync_voucher_sequence" in content, "resync_voucher_sequence missing"
    assert "OpenTally Safe Sequence Recovery Algorithm" in content, "Recovery algorithm box missing"
    assert "NEVER LOWER COUNTER" in content, "Never lower counter invariant missing"

    # 7. Check executed test count breakdown
    assert "64 passed, 0 failed" in content, "Executed test count 64 missing"

    print("All v2.6.0 Document Verification Assertions Passed 100%!")

if __name__ == "__main__":
    verify_v260()

