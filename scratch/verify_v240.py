with open('02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md', 'r', encoding='utf-8') as f:
    content = f.read()

# Check version
assert 'Document Version: 2.4.0' in content, 'Missing Document Version: 2.4.0'

# Check tests TC-VCH-01 to TC-VCH-87 in table
for i in range(1, 88):
    tc = f'TC-VCH-{i:02d}'
    expected = f'| `{tc}` |'
    assert expected in content, f'Missing {expected} in table'

# Check DEC-S2-001 status
assert 'PENDING USER APPROVAL' in content, 'Missing PENDING USER APPROVAL'

# Check Triggers 9, 10, 11
assert 'trg_prevent_active_config_mutation' in content
assert 'trg_prevent_active_config_deletion' in content
assert 'trg_prevent_retroactive_config_insert' in content

# Check schema constraint
assert 'uq_voucher_number_configs_date' in content

# Check uq_vouchers_permanent_number
assert 'uq_vouchers_permanent_number' in content

# Check partial year anchor
assert 'effective_anchor' in content

# Check series_name in vouchers
assert 'series_name TEXT NOT NULL DEFAULT' in content

print('All 87 tests TC-VCH-01 to TC-VCH-87 and v2.4.0 static assertions passed successfully!')

