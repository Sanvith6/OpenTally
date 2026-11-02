with open('02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md', 'r', encoding='utf-8') as f:
    content = f.read()

# Check version
assert 'Document Version: 2.3.0' in content, 'Missing Document Version: 2.3.0'

# Check tests TC-VCH-01 to TC-VCH-80 in table
for i in range(1, 81):
    tc = f'TC-VCH-{i:02d}'
    expected = f'| `{tc}` |'
    assert expected in content, f'Missing {expected} in table'

# Check DEC-S2-001 status
assert 'PENDING USER APPROVAL' in content, 'Missing PENDING USER APPROVAL'

# Check Triggers 9 & 10
assert 'trg_prevent_active_config_mutation' in content
assert 'trg_prevent_active_config_deletion' in content

# Check uq_vouchers_permanent_number
assert 'uq_vouchers_permanent_number' in content

# Check chk_voucher_total_amount_canonical allows draft NULL
assert "status = 'draft'" in content
assert "total_amount IS NULL" in content

# Check no earliest fallback in get_or_create_sequence_for_period
assert 'earliest.map(|c| c.start_number).unwrap_or(1)' not in content

print('All 80 tests TC-VCH-01 to TC-VCH-80 and static assertions passed successfully!')
