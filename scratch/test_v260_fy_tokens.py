def validate_and_format_token(restart_period, fy_begins, prefix_template, suffix_template, number, width, voucher_date, period_start):
    fy_m, fy_d = [int(p) for p in fy_begins.split('-')][1:]
    is_calendar_year = (fy_m == 1 and fy_d == 1)

    template = f"{prefix_template}{suffix_template}"

    # Validation: Reject invalid token configurations at creation time
    if restart_period == "yearly":
        if is_calendar_year:
            if not any(t in template for t in ["{YYYY}", "{YY-YY}", "{YYYY-YYYY}"]):
                raise ValueError("Calendar-year reset requires {YYYY}, {YY-YY}, or {YYYY-YYYY} token")
        else:
            # Non-calendar financial year (e.g. Indian FY 04-01)
            if "{YYYY}" in template and not any(t in template for t in ["{YY-YY}", "{YYYY-YYYY}"]):
                raise ValueError("Yearly financial-year reset prohibits bare {YYYY} token because financial years cross calendar boundaries. Require {YY-YY} or {YYYY-YYYY}.")
            if not any(t in template for t in ["{YY-YY}", "{YYYY-YYYY}"]):
                raise ValueError("Yearly financial-year reset requires {YY-YY} or {YYYY-YYYY} token.")
    elif restart_period == "monthly":
        if "{YYYY-MM}" not in template:
            raise ValueError("Monthly reset requires {YYYY-MM} token.")
    elif restart_period == "daily":
        if not any(t in template for t in ["{YYYYMMDD}", "{YYYY-MM-DD}"]):
            raise ValueError("Daily reset requires {YYYYMMDD} or {YYYY-MM-DD} token.")
    elif restart_period == "never":
        pass
    else:
        raise ValueError(f"Unknown restart_period: {restart_period}")

    # Evaluation Domain: Tokens are evaluated strictly from period_start!
    ps_year, ps_month, ps_day = [int(p) for p in period_start.split('-')]
    fy_start_year = ps_year
    fy_end_year = fy_start_year + 1
    fy_token_yy = f"{fy_start_year % 100:02d}-{(fy_end_year % 100):02d}"
    fy_token_yyyy = f"{fy_start_year:04d}-{fy_end_year:04d}"

    def replace_tokens(s):
        s = s.replace("{YY-YY}", fy_token_yy)
        s = s.replace("{YYYY-YYYY}", fy_token_yyyy)
        s = s.replace("{YYYY}", f"{ps_year:04d}")
        s = s.replace("{YYYY-MM}", f"{ps_year:04d}-{ps_month:02d}")
        s = s.replace("{YYYYMMDD}", f"{ps_year:04d}{ps_month:02d}{ps_day:02d}")
        s = s.replace("{YYYY-MM-DD}", f"{ps_year:04d}-{ps_month:02d}-{ps_day:02d}")
        return s

    p = replace_tokens(prefix_template)
    sfx = replace_tokens(suffix_template)
    num_str = f"{number:0{width}d}"
    return f"{p}{num_str}{sfx}"

def test_fy_tokens():
    fy_begins_indian = "2026-04-01"

    # Test 1: Bare {YYYY} rejected for Indian FY
    caught = False
    try:
        validate_and_format_token("yearly", fy_begins_indian, "INV/{YYYY}/", "", 1, 4, "2027-01-15", "2026-04-01")
    except ValueError as e:
        caught = True
        assert "prohibits bare {YYYY} token" in str(e)
    assert caught, "Bare {YYYY} must be rejected for Indian FY"

    # Test 2: {YY-YY} accepted for Indian FY
    # Scenario: 15 January 2027 belongs to FY 2026-27 (period_start = 2026-04-01)
    v1 = validate_and_format_token("yearly", fy_begins_indian, "INV/{YY-YY}/", "", 1, 4, "2027-01-15", "2026-04-01")
    assert v1 == "INV/26-27/0001", f"Got {v1}"

    # Scenario: 15 April 2027 belongs to FY 2027-28 (period_start = 2027-04-01)
    v2 = validate_and_format_token("yearly", fy_begins_indian, "INV/{YY-YY}/", "", 1, 4, "2027-04-15", "2027-04-01")
    assert v2 == "INV/27-28/0001", f"Got {v2}"

    assert v1 != v2, "Vouchers across FY boundary must have distinct formatted numbers even if calendar year matches!"

    # Test 3: Calendar year reset (01-01) accepts {YYYY}
    fy_begins_cal = "2026-01-01"
    v_cal = validate_and_format_token("yearly", fy_begins_cal, "BILL/{YYYY}/", "", 1, 4, "2026-06-15", "2026-01-01")
    assert v_cal == "BILL/2026/0001"

    # Test 4: Historical backdated voucher (voucher_date = 2026-02-15 in FY 2025-26)
    v_back = validate_and_format_token("yearly", fy_begins_indian, "INV/{YY-YY}/", "", 5, 4, "2026-02-15", "2025-04-01")
    assert v_back == "INV/25-26/0005"

    print("Finding 1: Financial-Year Token Correctness Tests Passed 100%!")

if __name__ == "__main__":
    test_fy_tokens()

