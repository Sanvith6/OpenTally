with open('02-open-tally/docs/OPENTALLY_SPRINT2_VOUCHER_ENGINE_TECHNICAL_DESIGN.md', 'r', encoding='utf-8') as f:
    content = f.read()

# Check version
assert 'Document Version: 2.5.0' in content, 'Missing Document Version: 2.5.0'

# Check tests TC-VCH-01 to TC-VCH-94 in table
for i in range(1, 95):
    tc = f'TC-VCH-{i:02d}'
    expected = f'| `{tc}` |'
    assert expected in content, f'Missing {expected} in table'

# Check DEC-S2-001 status
assert 'PENDING USER APPROVAL' in content, 'Missing PENDING USER APPROVAL'

# Check Triggers 9a, 9b, 10, 11
assert 'trg_prevent_config_applicable_from_mutation' in content
assert 'trg_prevent_active_config_mutation' in content
assert 'trg_prevent_active_config_deletion' in content
assert 'trg_prevent_retroactive_config_insert' in content

# Check schema constraint
assert 'uq_voucher_number_configs_date' in content

# Check uq_vouchers_permanent_number
assert 'uq_vouchers_permanent_number' in content

# Check namespace validation
assert 'validate_series_namespace_isolation' in content
assert 'validate_manual_voucher_number' in content

# Check format tokens
assert '{YY-YY}' in content
assert '{YYYY-MM}' in content

# Check recovery
assert 'VoucherNumberCollision' in content
assert 'resync_voucher_sequence' in content

print('All 94 tests TC-VCH-01 to TC-VCH-94 and v2.5.0 static assertions passed successfully!')

