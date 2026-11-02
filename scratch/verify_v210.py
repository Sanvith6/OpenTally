import re
import sqlite3

doc_path = "02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"
with open(doc_path, "r", encoding="utf-8") as f:
    content = f.read()

print("Verifying document:", doc_path)

# Check Version
assert "Version: 2.1.0" in content, "Missing Version: 2.1.0"
assert "| `2.1.0` | 2026-11-01 |" in content, "Missing 2.1.0 revision history row"

# Check DEC-S2-001 is PENDING USER APPROVAL
assert "**PENDING USER APPROVAL**" in content, "DEC-S2-001 is not marked PENDING USER APPROVAL"

# Check test count
tc_matches = re.findall(r"\| `TC-VCH-(\d+)` \|", content)
tc_numbers = [int(n) for n in tc_matches]
assert len(tc_numbers) == 72, f"Expected 72 test cases, got {len(tc_numbers)}"
assert tc_numbers == list(range(1, 73)), f"Test cases are not continuous 1..72: {tc_numbers}"
assert "Note: The 72 scenarios above (`TC-VCH-01` to `TC-VCH-72`) are **PROPOSED TEST SCENARIOS**" in content

# Check pure counter schema: voucher_number_sequences must NOT have prefix, suffix, width, config_id
seq_ddl_match = re.search(r"CREATE TABLE\s+(?:IF NOT EXISTS\s+)?voucher_number_sequences\s*\((.*?)\);", content, re.DOTALL)
assert seq_ddl_match, "voucher_number_sequences DDL not found"
seq_ddl = seq_ddl_match.group(1)
for col in ["prefix", "suffix", "width", "config_id"]:
    assert col not in seq_ddl, f"Column '{col}' should NOT be in voucher_number_sequences DDL"
print("voucher_number_sequences DDL columns verified pure:", [line.strip().split()[0] for line in seq_ddl.strip().split("\n") if line.strip() and not line.strip().startswith("FOREIGN") and not line.strip().startswith("CONSTRAINT") and not line.strip().startswith("PRIMARY")])

# Check canonical constraint substring check in voucher_lines and vouchers
assert "(substr(amount, 1, instr(amount, '.') - 1) = '0' OR substr(amount, 1, instr(amount, '.') - 1) GLOB '[1-9]*')" in content
assert "(substr(total_amount, 1, instr(total_amount, '.') - 1) = '0' OR substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*')" in content

# Check that old flawed check 'amount GLOB 0.*' is not present in constraint definitions
assert "amount GLOB '0.*'" not in content, "Old flawed GLOB '0.*' found in doc!"

print("All static document assertions passed successfully!")
