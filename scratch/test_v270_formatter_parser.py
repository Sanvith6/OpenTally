import re

class CanonicalVoucherFormatter:
    @staticmethod
    def resolve_period_tokens(template: str, period_start: str) -> str:
        if not period_start:
            return template
        parts = [int(p) for p in period_start.split('-')]
        ps_year, ps_month, ps_day = parts[0], parts[1], parts[2]
        fy_start_year = ps_year
        fy_end_year = fy_start_year + 1
        fy_token_yy = f"{fy_start_year % 100:02d}-{(fy_end_year % 100):02d}"
        fy_token_yyyy = f"{fy_start_year:04d}-{fy_end_year:04d}"

        result = template
        result = result.replace("{YY-YY}", fy_token_yy)
        result = result.replace("{YYYY-YYYY}", fy_token_yyyy)
        result = result.replace("{YYYY}", f"{ps_year:04d}")
        result = result.replace("{YYYY-MM}", f"{ps_year:04d}-{ps_month:02d}")
        result = result.replace("{YYYYMMDD}", f"{ps_year:04d}{ps_month:02d}{ps_day:02d}")
        result = result.replace("{YYYY-MM-DD}", f"{ps_year:04d}-{ps_month:02d}-{ps_day:02d}")
        return result

    @classmethod
    def format_automatic(cls, series_code: str, prefix_template: str, suffix_template: str,
                         sequence_number: int, width: int, period_start: str) -> str:
        # Assert prefix begins with or incorporates series_code
        resolved_prefix = cls.resolve_period_tokens(prefix_template, period_start)
        resolved_suffix = cls.resolve_period_tokens(suffix_template or "", period_start)
        num_str = f"{sequence_number:0{width}d}"
        return f"{resolved_prefix}{num_str}{resolved_suffix}"

    @classmethod
    def parse_and_validate(cls, voucher_number: str, series_code: str, prefix_template: str,
                           suffix_template: str, width: int, restart_period: str, period_start: str,
                           all_series_codes: list[str]) -> dict:
        # 1. Resolve expected prefix and suffix
        expected_prefix = cls.resolve_period_tokens(prefix_template, period_start)
        expected_suffix = cls.resolve_period_tokens(suffix_template or "", period_start)

        # 2. Check for usurpation of another series
        for other_code in all_series_codes:
            if other_code != series_code:
                # If the voucher number starts with or explicitly references another series code
                if f"/{other_code}/" in voucher_number or voucher_number.startswith(f"{other_code}/") or f"-{other_code}-" in voucher_number:
                    raise ValueError(f"Voucher number '{voucher_number}' usurps another series namespace ('{other_code}')")

        # 3. Check expected prefix match
        if not voucher_number.startswith(expected_prefix):
            raise ValueError(f"Voucher number '{voucher_number}' does not match expected series prefix '{expected_prefix}'")

        # 4. Check expected suffix match
        if expected_suffix and not voucher_number.endswith(expected_suffix):
            raise ValueError(f"Voucher number '{voucher_number}' does not match expected series suffix '{expected_suffix}'")

        # 5. Extract core component
        core = voucher_number[len(expected_prefix):]
        if expected_suffix:
            core = core[:-len(expected_suffix)]

        # 6. Check numeric / identifier validity
        if not core:
            raise ValueError("Voucher number has empty numeric/identifier component")

        # If pure digits expected (e.g. for automatic or numeric manual)
        if core.isdigit():
            # If width > 0, check zero-padding length
            if width > 0 and len(core) < width:
                raise ValueError(f"Numeric component '{core}' violates minimum zero-padding width of {width}")
            numeric_val = int(core)
            if numeric_val <= 0:
                raise ValueError(f"Numeric component '{core}' must be strictly positive")
            return {
                "series_code": series_code,
                "prefix": expected_prefix,
                "suffix": expected_suffix,
                "sequence_number": numeric_val,
                "is_numeric": True,
                "core": core
            }
        else:
            # Alphanumeric manual core: must be valid identifier [A-Za-z0-9_-]+
            if not re.match(r"^[A-Za-z0-9_\-]+$", core):
                raise ValueError(f"Identifier component '{core}' contains invalid characters")
            return {
                "series_code": series_code,
                "prefix": expected_prefix,
                "suffix": expected_suffix,
                "sequence_number": None,
                "is_numeric": False,
                "core": core
            }

def test_formatter_parser():
    all_codes = ["PRI", "RET", "MAN"]

    # 1. Automatic Formatter Round-Trip
    formatted = CanonicalVoucherFormatter.format_automatic(
        series_code="PRI",
        prefix_template="SALES/PRI/{YY-YY}/",
        suffix_template="",
        sequence_number=42,
        width=4,
        period_start="2026-04-01"
    )
    assert formatted == "SALES/PRI/26-27/0042", f"Got {formatted}"

    parsed = CanonicalVoucherFormatter.parse_and_validate(
        voucher_number=formatted,
        series_code="PRI",
        prefix_template="SALES/PRI/{YY-YY}/",
        suffix_template="",
        width=4,
        restart_period="yearly",
        period_start="2026-04-01",
        all_series_codes=all_codes
    )
    assert parsed["sequence_number"] == 42
    assert parsed["series_code"] == "PRI"
    assert parsed["prefix"] == "SALES/PRI/26-27/"

    # 2. Manual Voucher Parser: Valid manual number
    man_num = "SALES/MAN/OFFLINE-101"
    parsed_man = CanonicalVoucherFormatter.parse_and_validate(
        voucher_number=man_num,
        series_code="MAN",
        prefix_template="SALES/MAN/",
        suffix_template="",
        width=0,
        restart_period="never",
        period_start="2026-04-01",
        all_series_codes=all_codes
    )
    assert parsed_man["core"] == "OFFLINE-101"
    assert parsed_man["is_numeric"] is False

    # 3. Rejection Tests:
    # A. Usurping another series code
    caught = False
    try:
        CanonicalVoucherFormatter.parse_and_validate(
            voucher_number="SALES/PRI/26-27/0042",
            series_code="MAN",
            prefix_template="SALES/MAN/",
            suffix_template="",
            width=0,
            restart_period="never",
            period_start="2026-04-01",
            all_series_codes=all_codes
        )
    except ValueError as e:
        caught = True
        assert "usurps another series namespace" in str(e)
    assert caught, "Manual voucher usurping PRI series code must be rejected"

    # B. Missing series code / prefix mismatch
    caught = False
    try:
        CanonicalVoucherFormatter.parse_and_validate(
            voucher_number="INV-999",
            series_code="MAN",
            prefix_template="SALES/MAN/",
            suffix_template="",
            width=0,
            restart_period="never",
            period_start="2026-04-01",
            all_series_codes=all_codes
        )
    except ValueError as e:
        caught = True
        assert "does not match expected series prefix" in str(e)
    assert caught, "Number without series prefix must be rejected"

    # C. Incorrect period token
    caught = False
    try:
        CanonicalVoucherFormatter.parse_and_validate(
            voucher_number="SALES/PRI/25-26/0042",
            series_code="PRI",
            prefix_template="SALES/PRI/{YY-YY}/",
            suffix_template="",
            width=4,
            restart_period="yearly",
            period_start="2026-04-01",
            all_series_codes=all_codes
        )
    except ValueError as e:
        caught = True
        assert "does not match expected series prefix" in str(e)
    assert caught, "Incorrect period token must be rejected"

    # D. Incorrect zero padding
    caught = False
    try:
        CanonicalVoucherFormatter.parse_and_validate(
            voucher_number="SALES/PRI/26-27/42",
            series_code="PRI",
            prefix_template="SALES/PRI/{YY-YY}/",
            suffix_template="",
            width=4,
            restart_period="yearly",
            period_start="2026-04-01",
            all_series_codes=all_codes
        )
    except ValueError as e:
        caught = True
        assert "violates minimum zero-padding width" in str(e)
    assert caught, "Under-padded sequence number must be rejected"

    print("Finding 3: Canonical Formatter and Parser Tests Passed 100%!")

if __name__ == "__main__":
    test_formatter_parser()

