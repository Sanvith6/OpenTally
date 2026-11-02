import re
import os

DOC_PATH = r"02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"

def verify():
    assert os.path.exists(DOC_PATH), f"File {DOC_PATH} does not exist"
    with open(DOC_PATH, "r", encoding="utf-8") as f:
        content = f.read()

    # 1. Header version
    assert "Document Version: 2.7.0" in content, "Document version 2.7.0 missing in header"
    print("Header version 2.7.0 confirmed.")

    # 2. Revision History
    assert "| `2.7.0` | 2026-11-02 | Architecture Subagent |" in content, "Revision history entry for 2.7.0 missing"
    print("Revision history 2.7.0 confirmed.")

    # 3. Schema tables
    assert "CREATE TABLE voucher_number_series" in content, "voucher_number_series table missing"
    assert "CONSTRAINT uq_series_name UNIQUE (company_id, voucher_type_id, series_name)" in content, "uq_series_name missing"
    assert "CONSTRAINT uq_series_code UNIQUE (company_id, voucher_type_id, series_code)" in content, "uq_series_code missing"
    assert "CONSTRAINT uq_config_series_date UNIQUE (company_id, series_id, applicable_from)" in content, "uq_config_series_date missing"
    assert "CONSTRAINT uq_seq_period UNIQUE (company_id, series_id, period_start)" in content, "uq_seq_period missing"
    assert "CONSTRAINT chk_voucher_sequence_metadata CHECK (" in content, "chk_voucher_sequence_metadata missing"
    assert "CREATE UNIQUE INDEX uq_vouchers_series_sequence" in content, "uq_vouchers_series_sequence index missing"
    print("Schema tables and constraints confirmed.")

    # 4. Triggers 8a and 8b
    assert "CREATE TRIGGER trg_prevent_series_code_mutation" in content, "Trigger 8a missing"
    assert "CREATE TRIGGER trg_prevent_series_deletion_with_vouchers" in content, "Trigger 8b missing"
    print("Triggers 8a and 8b confirmed.")

    # 5. Formatter & Parser
    assert "pub struct CanonicalVoucherFormatter;" in content, "CanonicalVoucherFormatter struct missing"
    assert "pub fn format_automatic(" in content, "format_automatic missing"
    assert "pub fn parse_and_validate_manual_voucher_number(" in content, "parse_and_validate_manual_voucher_number missing"
    assert "pub struct ParsedManualVoucherNumber" in content, "ParsedManualVoucherNumber struct missing"
    print("CanonicalVoucherFormatter confirmed.")

    # 6. Recovery algorithm
    assert "OpenTally 10-Step Safe Sequence Recovery Algorithm" in content, "10-Step Safe Sequence Recovery title missing"
    assert "resync_voucher_sequence(company_id, voucher_type_id, series_id, period_start, operator_id, resolution_mode, resolution_reason)" in content, "resync_voucher_sequence signature missing"
    print("10-Step Safe Sequence Recovery confirmed.")

    # 7. Test matrix 112 scenarios
    for i in range(1, 113):
        tc_id = f"TC-VCH-{i:02d}"
        assert tc_id in content, f"Missing test scenario {tc_id}"
    print("All 112 test scenarios TC-VCH-01 through TC-VCH-112 confirmed.")

    # 8. Scratch tests in 10.2
    assert "scratch/test_v270_series_master_schema.py" in content, "test_v270_series_master_schema.py missing in 10.2"
    assert "scratch/test_v270_sequence_metadata_constraints.py" in content, "test_v270_sequence_metadata_constraints.py missing in 10.2"
    assert "scratch/test_v270_formatter_parser.py" in content, "test_v270_formatter_parser.py missing in 10.2"
    assert "scratch/test_v270_safe_recovery.py" in content, "test_v270_safe_recovery.py missing in 10.2"
    assert "The **112 scenarios** above" in content, "112 scenarios summary missing in 10.3"
    print("Scratch test references and 112 summary confirmed.")

    # 9. DEC-S2-001 status
    assert "**DEC-S2-001**" in content, "DEC-S2-001 missing"
    assert "PENDING USER APPROVAL" in content, "PENDING USER APPROVAL missing"
    print("DEC-S2-001 status confirmed.")

    print("\nALL V2.7.0 TECHNICAL DESIGN ASSERTIONS PASSED SUCCESSFULLY!")

if __name__ == "__main__":
    verify()

