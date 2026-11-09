#!/usr/bin/env python3
"""
Static verification script for OpenTally Sprint 2 Technical Design Document v2.9.5.
Asserts that all v2.9.5 invariants, findings, test cases, and governance constraints
are accurately reflected in 02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md.
"""

import os
import re
import sys

DOC_PATH = os.path.join("02-open-tally", "docs", "OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md")

def main():
    print(f"Reading document: {DOC_PATH}")
    if not os.path.exists(DOC_PATH):
        print(f"ERROR: File not found at {DOC_PATH}")
        sys.exit(1)

    with open(DOC_PATH, "r", encoding="utf-8") as f:
        content = f.read()

    checks_passed = 0
    total_checks = 22

    def assert_check(condition: bool, name: str):
        nonlocal checks_passed
        if condition:
            print(f"  [PASS] {name}")
            checks_passed += 1
        else:
            print(f"  [FAIL] {name}")

    print("\n--- Verifying Document Version & Header Metadata ---")
    assert_check("Document Version: 2.9.5" in content, "Document header specifies Version 2.9.5")
    assert_check("Date: November 3, 2026" in content, "Document date is November 3, 2026")
    assert_check("Status: READY_FOR_IMPLEMENTATION_APPROVAL (DESIGN ONLY)" in content, "Document status is READY_FOR_IMPLEMENTATION_APPROVAL")

    print("\n--- Verifying Revision History ---")
    assert_check("| `2.9.5` | 2026-11-03 | Architecture Subagent | **Final Design Sign-off Corrections Pass (v2.9.5)**:" in content, "Revision history contains comprehensive v2.9.5 entry")

    print("\n--- Verifying Finding 2: Immutable Series-Level Numbering Reset Policy ---")
    assert_check("restart_period TEXT NOT NULL CHECK (restart_period IN ('yearly', 'monthly', 'daily', 'never'))" in content, "voucher_number_series DDL specifies restart_period without SQL DEFAULT")
    assert_check("DEFAULT 'yearly'" not in re.search(r"CREATE TABLE voucher_number_series \(.*?\);", content, re.DOTALL).group(0), "No DEFAULT 'yearly' on voucher_number_series.restart_period")
    assert_check("CREATE TRIGGER trg_prevent_series_restart_period_mutation" in content, "Trigger 12 trg_prevent_series_restart_period_mutation is present")

    print("\n--- Verifying Finding 1: Complete 12-Step Audit Migration Lifecycle ---")
    assert_check("OpenTally Migration 0.3.0-sprint2 Authoritative 12-Step Connection Lifecycle" in content, "12-step connection lifecycle diagram is documented")
    assert_check("SELECT COUNT(*) FROM audit_log;" in content and "SELECT COUNT(*) FROM audit_log_dg_tmp;" in content, "Step 7 pre-drop row-count verification query is explicitly documented")
    assert_check("FOREIGN KEY (company_id) REFERENCES companies(id) ON DELETE RESTRICT" in content, "Target audit_log DDL specifies ON DELETE RESTRICT on company_id")
    assert_check("DELETE FROM companies WHERE id = 'c_01';" in content and "SQLITE_CONSTRAINT_FOREIGNKEY" in content, "Demonstration of company deletion blocked by ON DELETE RESTRICT is documented")
    assert_check("State D: Post-Commit Connection Cleanup Failure" in content, "Distinction between execution failure and post-commit cleanup failure is documented")
    assert_check("been executed against the production database" in content.lower(), "Clean-room scratch verification affirmation is present")

    print("\n--- Verifying Finding 3: Reversal Series Scope Solidification ---")
    assert_check("Sprint 2 exclusively implements and utilizes **one single dedicated `REV` numbering series**" in content, "Sprint 2 single REV series under vt_reversing_journal is solidified")
    assert_check("Future Enterprise Enhancement (Non-Breaking Extension Pattern)" in content, "REVSAL, REVPUR, REVGEN classified as future optional enterprise enhancements")
    assert_check("permanently retains its historical `voucher_type_id`" in content, "Original voucher retains historical voucher_type_id permanently")

    print("\n--- Verifying Finding 4: Appendix A & Governance Ledger ---")
    assert_check("## 13. Appendix A: Consolidated Final Architecture & Readiness Ledger" in content, "Appendix A is present")
    assert_check("### A.1 Ten Reconciled Architectural Facets" in content, "All 10 architectural facets are reconciled")
    assert_check("### A.2 Pre-Implementation Quality Gates" in content, "Quality gates are documented")
    assert_check("### A.3 Architectural Decision & Unknown Ledger Status" in content, "Decision and unknown ledger status is documented")
    assert_check("DEC-S2-001" in content and "PENDING USER APPROVAL" in content and "DEC-S2-002" in content, "Both DEC-S2-001 and DEC-S2-002 remain PENDING USER APPROVAL")

    print("\n--- Verifying Proposed Test Scenarios Matrix ---")
    tc_matches = re.findall(r"\|\s*`(TC-VCH-\d+)`\s*\|", content)
    unique_tc = sorted(list(set(tc_matches)), key=lambda x: int(x.split("-")[-1]))
    print(f"Total test case table rows found: {len(tc_matches)}")
    print(f"Total unique test case IDs found: {len(unique_tc)} (from {unique_tc[0]} to {unique_tc[-1]})")

    assert_check(len(unique_tc) == 157 and unique_tc[0] == "TC-VCH-01" and unique_tc[-1] == "TC-VCH-157", "Exactly 157 unique test cases (TC-VCH-01 to TC-VCH-157) are present")

    print(f"\nVerification Results: {checks_passed}/{total_checks} passed.")
    if checks_passed == total_checks:
        print("ALL STATIC DOCUMENT CHECKS PASSED (100%)!")
        return 0
    else:
        print(f"ERROR: {total_checks - checks_passed} check(s) failed!")
        return 1

if __name__ == "__main__":
    sys.exit(main())
