def format_voucher_number_with_tokens(prefix_template, number, suffix_template, width, voucher_date, fy_start_month=4):
    v_year, v_month, v_day = [int(p) for p in voucher_date.split('-')]
    
    # Compute FY
    fy_start_year = v_year if v_month >= fy_start_month else v_year - 1
    fy_end_year = fy_start_year + 1
    fy_token_yy = f"{fy_start_year % 100:02d}-{(fy_end_year % 100):02d}" # "26-27"
    fy_token_yyyy = f"{fy_start_year:04d}-{fy_end_year:04d}" # "2026-2027"

    # Replace tokens in prefix
    p = prefix_template
    p = p.replace("{YY-YY}", fy_token_yy)
    p = p.replace("{YYYY-YYYY}", fy_token_yyyy)
    p = p.replace("{YYYY}", f"{v_year:04d}")
    p = p.replace("{YYYY-MM}", f"{v_year:04d}-{v_month:02d}")
    p = p.replace("{YYYYMMDD}", f"{v_year:04d}{v_month:02d}{v_day:02d}")

    # Replace tokens in suffix
    s = suffix_template
    s = s.replace("{YY-YY}", fy_token_yy)
    s = s.replace("{YYYY-YYYY}", fy_token_yyyy)
    s = s.replace("{YYYY}", f"{v_year:04d}")
    s = s.replace("{YYYY-MM}", f"{v_year:04d}-{v_month:02d}")
    s = s.replace("{YYYYMMDD}", f"{v_year:04d}{v_month:02d}{v_day:02d}")

    num_str = f"{number:0{width}d}"
    return f"{p}{num_str}{s}"

def test_reset_period_formatting():
    # 1. Yearly reset with {YY-YY} token
    template_yearly = "INV/{YY-YY}/"
    v1 = format_voucher_number_with_tokens(template_yearly, 1, "", 4, "2026-05-10")
    assert v1 == "INV/26-27/0001", f"Got {v1}"

    # Same sequence number in next FY 2027-28
    v2 = format_voucher_number_with_tokens(template_yearly, 1, "", 4, "2027-05-10")
    assert v2 == "INV/27-28/0001", f"Got {v2}"
    assert v1 != v2, "Yearly reset with {YY-YY} token must produce distinct numbers across financial years"

    # 2. Monthly reset with {YYYY-MM} token
    template_monthly = "BILL/{YYYY-MM}/"
    m1 = format_voucher_number_with_tokens(template_monthly, 1, "", 3, "2026-04-15")
    m2 = format_voucher_number_with_tokens(template_monthly, 1, "", 3, "2026-05-02")
    assert m1 == "BILL/2026-04/001"
    assert m2 == "BILL/2026-05/001"
    assert m1 != m2, "Monthly reset with {YYYY-MM} token must produce distinct numbers across months"

    # 3. Daily reset with {YYYYMMDD} token
    template_daily = "POS/{YYYYMMDD}/"
    d1 = format_voucher_number_with_tokens(template_daily, 1, "", 4, "2026-05-01")
    d2 = format_voucher_number_with_tokens(template_daily, 1, "", 4, "2026-05-02")
    assert d1 == "POS/20260501/0001"
    assert d2 == "POS/20260502/0001"
    assert d1 != d2, "Daily reset with {YYYYMMDD} token must produce distinct numbers across days"

    # 4. Never reset
    template_never = "DOC/"
    n1 = format_voucher_number_with_tokens(template_never, 1, "", 6, "2026-05-01")
    n2 = format_voucher_number_with_tokens(template_never, 2, "", 6, "2027-05-01")
    assert n1 == "DOC/000001"
    assert n2 == "DOC/000002"
    assert n1 != n2

    print("Finding 3: Reset-Period Formatting Uniqueness Tests Passed 100%!")

if __name__ == "__main__":
    test_reset_period_formatting()

