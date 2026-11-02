def validate_prefix_template(restart_period, prefix_template, suffix_template):
    template = f"{prefix_template or ''}{suffix_template or ''}"
    
    PERIODIC_TOKENS = {
        "yearly": ["{YY-YY}", "{YYYY-YYYY}", "{YYYY}"],
        "monthly": ["{YYYY-MM}"],
        "daily": ["{YYYYMMDD}", "{YYYY-MM-DD}"]
    }
    
    if restart_period in PERIODIC_TOKENS:
        required_tokens = PERIODIC_TOKENS[restart_period]
        has_period_token = any(tok in template for tok in required_tokens)
        if not has_period_token:
            raise ValueError(
                f"Configuration rejected: restart_period='{restart_period}' requires at least one periodic token "
                f"from {required_tokens} in prefix or suffix. Static-only templates are prohibited for periodic resets "
                f"because they guarantee inter-period sequence collisions."
            )
    elif restart_period == "never":
        # Static-only templates permitted
        pass
    else:
        raise ValueError(f"Invalid restart_period: {restart_period}")
    return True

def test_static_prefix_rejection():
    # Test 1: Static template rejected for yearly reset
    caught = False
    try:
        validate_prefix_template("yearly", "INV/", "")
    except ValueError as e:
        caught = True
        assert "Static-only templates are prohibited for periodic resets" in str(e)
    assert caught, "Static prefix under yearly reset must be rejected"

    # Test 2: Static template rejected for monthly reset
    caught = False
    try:
        validate_prefix_template("monthly", "BILL/", "/HQ")
    except ValueError as e:
        caught = True
        assert "requires at least one periodic token" in str(e)
    assert caught, "Static prefix under monthly reset must be rejected"

    # Test 3: Static template rejected for daily reset
    caught = False
    try:
        validate_prefix_template("daily", "DAY-", "")
    except ValueError as e:
        caught = True
        assert "requires at least one periodic token" in str(e)
    assert caught, "Static prefix under daily reset must be rejected"

    # Test 4: Static template permitted for 'never' reset
    assert validate_prefix_template("never", "INV/", "") is True
    assert validate_prefix_template("never", "STATIC-", "/POST") is True

    # Test 5: Dynamic template permitted for yearly reset
    assert validate_prefix_template("yearly", "INV/{YY-YY}/", "") is True

    # Test 6: Dynamic template permitted for monthly reset
    assert validate_prefix_template("monthly", "INV/{YYYY-MM}/", "") is True

    # Test 7: Dynamic template permitted for daily reset
    assert validate_prefix_template("daily", "INV/{YYYYMMDD}/", "") is True

    print("Finding 3: Static-Prefix Rejection for Periodic Resets Tests Passed 100%!")

if __name__ == "__main__":
    test_static_prefix_rejection()

