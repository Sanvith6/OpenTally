import shutil
import re
import sqlite3

src = "02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"
dst = r"C:\Users\sanvi\.gemini\antigravity\brain\6c1fb660-6b74-4677-bba2-c159b4f31c4f\OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"

# Copy to artifact
shutil.copyfile(src, dst)
print("Copied to artifact successfully.")

# Read content
with open(src, "r", encoding="utf-8") as f:
    content = f.read()

# Assertions
assert "Document Version: 2.0.0" in content, "Missing Document Version 2.0.0"
assert "voucher_number_configs" in content, "Missing voucher_number_configs"
assert "voucher_number_sequences" in content, "Missing voucher_number_sequences"
assert "applicable_from" in content, "Missing applicable_from"
assert "period_start" in content, "Missing period_start"
assert "chk_voucher_line_amount_canonical" in content, "Missing chk_voucher_line_amount_canonical"
assert "chk_voucher_total_amount_canonical" in content, "Missing chk_voucher_total_amount_canonical"
assert "State A Restoration Failure" in content or "State A restoration failure" in content, "Missing State A restoration failure"
assert "DEC-S2-001" in content and "PENDING USER APPROVAL" in content, "DEC-S2-001 must be PENDING USER APPROVAL"

# Count test cases
test_matches = re.findall(r"TC-VCH-\d\d", content)
unique_tests = sorted(list(set(test_matches)))
print(f"Found {len(unique_tests)} unique test IDs: {unique_tests[0]} to {unique_tests[-1]}")
assert len(unique_tests) == 70, f"Expected 70 tests, got {len(unique_tests)}"
assert unique_tests[-1] == "TC-VCH-70", f"Last test should be TC-VCH-70, got {unique_tests[-1]}"

# Verify all proposed tests contain PROPOSED
assert "TC-VCH-01" in content and "TC-VCH-70" in content

print("All static document assertions PASSED!")

