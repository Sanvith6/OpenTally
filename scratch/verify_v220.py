import re

doc_path = "02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md"
with open(doc_path, "r", encoding="utf-8") as f:
    content = f.read()

print("Verifying v2.2.0 document:", doc_path)

# Check Version
assert "Version: 2.2.0" in content, "Missing Version: 2.2.0"
assert "| `2.2.0` | 2026-11-01 |" in content, "Missing 2.2.0 revision history row"

# Check DEC-S2-001 is PENDING USER APPROVAL
assert "**PENDING USER APPROVAL**" in content, "DEC-S2-001 is not marked PENDING USER APPROVAL"

# Check test count
tc_matches = re.findall(r"\| `TC-VCH-(\d+)` \|", content)
tc_numbers = [int(n) for n in tc_matches]
assert len(tc_numbers) == 75, f"Expected 75 test cases, got {len(tc_numbers)}"
assert tc_numbers == list(range(1, 76)), f"Test cases are not continuous 1..75: {tc_numbers}"
assert "Note: The 75 scenarios above (`TC-VCH-01` to `TC-VCH-75`) are **PROPOSED TEST SCENARIOS**" in content

# Check pure counter schema: voucher_number_sequences must NOT have prefix, suffix, width, config_id
seq_ddl_match = re.search(r"CREATE TABLE\s+(?:IF NOT EXISTS\s+)?voucher_number_sequences\s*\((.*?)\);", content, re.DOTALL)
assert seq_ddl_match, "voucher_number_sequences DDL not found"
seq_ddl = seq_ddl_match.group(1)
for col in ["prefix", "suffix", "width", "config_id"]:
    assert col not in seq_ddl, f"Column '{col}' should NOT be in voucher_number_sequences DDL"

# Check canonical constraint logic
assert "substr(amount, 1, instr(amount, '.') - 1) = '0'" in content
assert "substr(amount, 1, instr(amount, '.') - 1) GLOB '[1-9]*'" in content
assert "substr(total_amount, 1, instr(total_amount, '.') - 1) = '0'" in content
assert "substr(total_amount, 1, instr(total_amount, '.') - 1) GLOB '[1-9]*'" in content

# Check that flawed unconditional fraction non-zero check is NOT present
# (it should only be conditioned on integer part = '0')
assert "AND substr(amount, instr(amount, '.') + 1) GLOB '*[1-9]*'\n" not in content

# Check Period-Anchor Initialization Invariant
assert "Deterministic Period-Anchor Initialization Invariant" in content

print("All v2.2.0 static document assertions passed successfully!")

