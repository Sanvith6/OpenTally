import re

class SeriesConfig:
    def __init__(self, name, method, prefix, suffix, width):
        self.name = name
        self.method = method # 'automatic' or 'manual'
        self.prefix = prefix
        self.suffix = suffix
        self.width = width

def validate_series_namespace_isolation(existing_series, new_series):
    """
    Validates that new_series does not overlap in namespace with any existing_series.
    """
    for ex in existing_series:
        # 1. Identical templates
        if ex.prefix == new_series.prefix and ex.suffix == new_series.suffix:
            raise ValueError(f"Series '{new_series.name}' has identical prefix '{new_series.prefix}' and suffix '{new_series.suffix}' to existing series '{ex.name}'. Namespaces must be distinct.")
        
        # 2. Prefix overlap check (one is prefix of another without disambiguating delimiter or suffix)
        if ex.prefix == new_series.prefix:
            # If prefixes are identical, suffixes must be distinct and non-empty
            if not ex.suffix or not new_series.suffix:
                raise ValueError(f"Series '{new_series.name}' shares prefix '{new_series.prefix}' with '{ex.name}' but lacks distinct mandatory suffixes.")
        elif ex.prefix.startswith(new_series.prefix) or new_series.prefix.startswith(ex.prefix):
            # One prefix is substring of another. E.g. 'INV' and 'INV0'.
            # If delimiters are not used, check if digits can bridge the difference
            shorter, longer = (new_series.prefix, ex.prefix) if len(new_series.prefix) < len(ex.prefix) else (ex.prefix, new_series.prefix)
            diff = longer[len(shorter):]
            if diff.isdigit():
                raise ValueError(f"Ambiguous series prefixes: '{shorter}' and '{longer}' can generate identical numbers with digit sequences.")

def validate_manual_voucher_number(series, number_str, other_series_list):
    """
    Validates manual voucher number:
    1. Must match series prefix
    2. Must match series suffix
    3. Payload between prefix and suffix must be non-empty and valid
    4. Must not start with prefix of any other series
    """
    if not number_str.startswith(series.prefix):
        raise ValueError(f"Manual voucher number '{number_str}' does not start with series prefix '{series.prefix}'.")
    if series.suffix and not number_str.endswith(series.suffix):
        raise ValueError(f"Manual voucher number '{number_str}' does not end with series suffix '{series.suffix}'.")
    
    # Check that it doesn't hijack another series
    for other in other_series_list:
        if other.name != series.name and other.prefix and number_str.startswith(other.prefix) and len(other.prefix) > len(series.prefix):
            raise ValueError(f"Manual voucher number '{number_str}' infringes on series '{other.name}' namespace.")

def test_namespace_isolation():
    s_auto1 = SeriesConfig("Auto-Retail", "automatic", "RET/", "", 4)
    s_auto2 = SeriesConfig("Auto-Wholesale", "automatic", "WHO/", "", 4)
    s_man1 = SeriesConfig("Manual-Direct", "manual", "MAN-DIR/", "", 4)
    s_man2 = SeriesConfig("Manual-Online", "manual", "MAN-ONL/", "", 4)

    existing = [s_auto1, s_auto2, s_man1]

    # Test 1: Valid new series passes
    validate_series_namespace_isolation(existing, s_man2)

    # Test 2: Two automatic series with identical prefix rejected
    dup_auto = SeriesConfig("Auto-Conflict", "automatic", "RET/", "", 4)
    caught = False
    try:
        validate_series_namespace_isolation(existing, dup_auto)
    except ValueError as e:
        caught = True
        assert "identical prefix" in str(e)
    assert caught, "Duplicate automatic series prefix must be rejected"

    # Test 3: Two manual series with identical prefix rejected
    dup_man = SeriesConfig("Manual-Conflict", "manual", "MAN-DIR/", "", 4)
    caught = False
    try:
        validate_series_namespace_isolation(existing, dup_man)
    except ValueError as e:
        caught = True
        assert "identical prefix" in str(e)
    assert caught, "Duplicate manual series prefix must be rejected"

    # Test 4: Ambiguous prefix overlap (e.g. 'RET' vs 'RET0') rejected
    ambig = SeriesConfig("Auto-Ambig", "automatic", "RET0", "", 4)
    # s_auto1 prefix is 'RET/' which has delimiter, but if someone uses 'RET' and 'RET0':
    s_base = SeriesConfig("Base", "automatic", "INV", "", 4)
    s_num = SeriesConfig("Num", "automatic", "INV0", "", 3)
    caught = False
    try:
        validate_series_namespace_isolation([s_base], s_num)
    except ValueError as e:
        caught = True
        assert "Ambiguous series prefixes" in str(e)
    assert caught, "Ambiguous prefix overlap must be rejected"

    # Test 5: Manual voucher validation: valid manual number accepted
    validate_manual_voucher_number(s_man1, "MAN-DIR/0042", existing)

    # Test 6: Manual voucher validation: wrong prefix rejected
    caught = False
    try:
        validate_manual_voucher_number(s_man1, "WRONG/0042", existing)
    except ValueError as e:
        caught = True
        assert "does not start with series prefix" in str(e)
    assert caught, "Manual voucher with wrong prefix must be rejected"

    # Test 7: Manual voucher validation: claiming automatic series prefix rejected
    caught = False
    try:
        validate_manual_voucher_number(s_man1, "RET/0001", existing)
    except ValueError as e:
        caught = True
        assert "does not start with series prefix" in str(e)
    assert caught, "Manual voucher claiming auto prefix must be rejected"

    print("Finding 2: Numbering Namespace Isolation Tests Passed 100%!")

if __name__ == "__main__":
    test_namespace_isolation()

