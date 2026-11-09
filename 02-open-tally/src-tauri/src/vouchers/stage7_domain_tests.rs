//! Sprint 2 Stage 7 Phase 1: Search Domain Foundation Unit Tests
//!
//! Pure domain unit tests for Sprint 2 Stage 7 search filter models,
//! exact monetary canonical normalization, domain validation rules,
//! and serialization / deserialization round-tripping.
//!
//! Phase 1 Scope: Pure domain logic only. Zero database or SQL dependencies.

use crate::errors::OpenTallyError;
use crate::vouchers::model::{
    DateBasis, PaginatedResponse, SortDirection, VoucherLineageResponse, VoucherRegisterItem,
    VoucherSearchFilter, VoucherSortField,
};
use crate::vouchers::validator::{normalize_canonical_amount_for_query, validate_search_filter};

#[test]
fn test_01_valid_search_request_deserialization() {
    let json = r#"{
        "voucher_type_id": "vt_sales_001",
        "series_id": "s_main_001",
        "status": "posted",
        "ledger_id": "led_debtor_001",
        "date_basis": "transaction_date",
        "date_from": "2025-04-01",
        "date_to": "2025-04-30",
        "min_amount": "100.50",
        "max_amount": "50000.00",
        "search_query": "INV-2025-001",
        "reversal_of_voucher_id": "v_orig_123",
        "page": 2,
        "page_size": 50,
        "sort_by": "total_amount",
        "sort_direction": "desc"
    }"#;

    let filter: VoucherSearchFilter =
        serde_json::from_str(json).expect("Should deserialize valid search filter");

    assert_eq!(filter.voucher_type_id.as_deref(), Some("vt_sales_001"));
    assert_eq!(filter.series_id.as_deref(), Some("s_main_001"));
    assert_eq!(filter.status.as_deref(), Some("posted"));
    assert_eq!(filter.ledger_id.as_deref(), Some("led_debtor_001"));
    assert_eq!(filter.date_basis, Some(DateBasis::TransactionDate));
    assert_eq!(filter.date_from.as_deref(), Some("2025-04-01"));
    assert_eq!(filter.date_to.as_deref(), Some("2025-04-30"));
    assert_eq!(filter.min_amount.as_deref(), Some("100.50"));
    assert_eq!(filter.max_amount.as_deref(), Some("50000.00"));
    assert_eq!(filter.search_query.as_deref(), Some("INV-2025-001"));
    assert_eq!(filter.reversal_of_voucher_id.as_deref(), Some("v_orig_123"));
    assert_eq!(filter.page, Some(2));
    assert_eq!(filter.page_size, Some(50));
    assert_eq!(filter.sort_by, Some(VoucherSortField::TotalAmount));
    assert_eq!(filter.sort_direction, Some(SortDirection::Desc));

    // Validation must also succeed
    validate_search_filter(&filter).expect("Fully populated filter should pass validation");
}

#[test]
fn test_02_invalid_date_range_fails() {
    let filter = VoucherSearchFilter {
        date_from: Some("2025-04-30".to_string()),
        date_to: Some("2025-04-01".to_string()),
        ..Default::default()
    };

    let err = validate_search_filter(&filter).unwrap_err();
    match err {
        OpenTallyError::Validation(msg) => {
            assert!(
                msg.contains("cannot be later than date_to"),
                "Expected date range error, got: {}",
                msg
            );
        }
        other => panic!("Expected Validation error, got: {:?}", other),
    }
}

#[test]
fn test_03_valid_date_range_boundaries() {
    // Exact same date (single day boundary)
    let filter_same = VoucherSearchFilter {
        date_from: Some("2025-04-15".to_string()),
        date_to: Some("2025-04-15".to_string()),
        ..Default::default()
    };
    validate_search_filter(&filter_same).expect("Same from/to date must be valid");

    // Standard range
    let filter_range = VoucherSearchFilter {
        date_from: Some("2025-04-01".to_string()),
        date_to: Some("2025-04-30".to_string()),
        ..Default::default()
    };
    validate_search_filter(&filter_range).expect("Valid date range must pass");

    // Only date_from
    let filter_from_only = VoucherSearchFilter {
        date_from: Some("2025-04-01".to_string()),
        ..Default::default()
    };
    validate_search_filter(&filter_from_only).expect("Only date_from must pass");

    // Only date_to
    let filter_to_only = VoucherSearchFilter {
        date_to: Some("2025-04-30".to_string()),
        ..Default::default()
    };
    validate_search_filter(&filter_to_only).expect("Only date_to must pass");

    // Invalid date formats
    for invalid_date in &["2025-13-01", "2025-02-30", "not-a-date", "01-04-2025", ""] {
        let filter_bad = VoucherSearchFilter {
            date_from: Some(invalid_date.to_string()),
            ..Default::default()
        };
        assert!(
            validate_search_filter(&filter_bad).is_err(),
            "Expected '{}' to fail date validation",
            invalid_date
        );
    }
}

#[test]
fn test_04_valid_pagination() {
    let filter = VoucherSearchFilter {
        page: Some(1),
        page_size: Some(20),
        ..Default::default()
    };
    validate_search_filter(&filter).expect("Default pagination should pass");

    let filter_max = VoucherSearchFilter {
        page: Some(500),
        page_size: Some(100),
        ..Default::default()
    };
    validate_search_filter(&filter_max).expect("Page size 100 should pass");

    let filter_min_page_size = VoucherSearchFilter {
        page: Some(1),
        page_size: Some(1),
        ..Default::default()
    };
    validate_search_filter(&filter_min_page_size).expect("Page size 1 should pass");
}

#[test]
fn test_05_invalid_page_size() {
    // Page size 0 fails
    let filter_zero = VoucherSearchFilter {
        page_size: Some(0),
        ..Default::default()
    };
    let err = validate_search_filter(&filter_zero).unwrap_err();
    assert!(err.to_string().contains("greater than or equal to 1"));

    // Page size 101 exceeds max 100
    let filter_101 = VoucherSearchFilter {
        page_size: Some(101),
        ..Default::default()
    };
    let err = validate_search_filter(&filter_101).unwrap_err();
    assert!(err
        .to_string()
        .contains("exceeds maximum permitted limit of 100"));

    // Page size 10_000 fails
    let filter_huge = VoucherSearchFilter {
        page_size: Some(10_000),
        ..Default::default()
    };
    assert!(validate_search_filter(&filter_huge).is_err());
}

#[test]
fn test_06_invalid_page_number_and_overflow() {
    // Page 0 fails
    let filter_zero = VoucherSearchFilter {
        page: Some(0),
        ..Default::default()
    };
    let err = validate_search_filter(&filter_zero).unwrap_err();
    assert!(err.to_string().contains("greater than or equal to 1"));

    // Offset multiplication overflow
    let filter_overflow = VoucherSearchFilter {
        page: Some(u32::MAX),
        page_size: Some(100),
        ..Default::default()
    };
    let err = validate_search_filter(&filter_overflow).unwrap_err();
    assert!(err.to_string().contains("overflows u32 range"));
}

#[test]
fn test_07_valid_sort_configuration_and_defaults() {
    // Test defaults
    assert_eq!(DateBasis::default(), DateBasis::TransactionDate);
    assert_eq!(VoucherSortField::default(), VoucherSortField::Date);
    assert_eq!(SortDirection::default(), SortDirection::Desc);

    // Test explicit configuration
    let filter = VoucherSearchFilter {
        sort_by: Some(VoucherSortField::TotalAmount),
        sort_direction: Some(SortDirection::Asc),
        date_basis: Some(DateBasis::EffectiveDate),
        ..Default::default()
    };
    validate_search_filter(&filter).expect("Valid sort configuration should pass");
}

#[test]
fn test_08_unsupported_sort_field_and_status_rejection() {
    // Unsupported sort_by deserialization fails
    let bad_sort = r#"{"sort_by": "unsupported_field"}"#;
    assert!(serde_json::from_str::<VoucherSearchFilter>(bad_sort).is_err());

    // Unsupported sort_direction deserialization fails
    let bad_direction = r#"{"sort_direction": "sideways"}"#;
    assert!(serde_json::from_str::<VoucherSearchFilter>(bad_direction).is_err());

    // Unsupported date_basis deserialization fails
    let bad_basis = r#"{"date_basis": "lunar_month"}"#;
    assert!(serde_json::from_str::<VoucherSearchFilter>(bad_basis).is_err());

    // Unsupported status validation fails
    for bad_status in &["pending", "deleted", "active", "submitted", "unknown"] {
        let filter = VoucherSearchFilter {
            status: Some(bad_status.to_string()),
            ..Default::default()
        };
        let err = validate_search_filter(&filter).unwrap_err();
        assert!(
            err.to_string().contains("Invalid voucher search status"),
            "Expected status rejection for '{}', got: {}",
            bad_status,
            err
        );
    }

    // Supported statuses succeed
    for valid_status in &[
        "posted",
        "reversed",
        "cancelled",
        "draft",
        "all",
        "POSTED",
        " Draft ",
    ] {
        let filter = VoucherSearchFilter {
            status: Some(valid_status.to_string()),
            ..Default::default()
        };
        validate_search_filter(&filter).unwrap_or_else(|e| {
            panic!(
                "Expected status '{}' to succeed validation, got: {}",
                valid_status, e
            )
        });
    }
}

#[test]
fn test_09_invalid_amount_range() {
    // min > max
    let filter = VoucherSearchFilter {
        min_amount: Some("500.00".to_string()),
        max_amount: Some("100.00".to_string()),
        ..Default::default()
    };
    let err = validate_search_filter(&filter).unwrap_err();
    assert!(err
        .to_string()
        .contains("cannot be greater than max_amount"));

    // Subtle lexicographical trap: raw "1000.00" < "200.00" in naive string comparison!
    // But canonical normalized 1000 > 200, so min > max check MUST catch this:
    let filter_lex = VoucherSearchFilter {
        min_amount: Some("1000.00".to_string()),
        max_amount: Some("200.00".to_string()),
        ..Default::default()
    };
    let err_lex = validate_search_filter(&filter_lex).unwrap_err();
    assert!(
        err_lex
            .to_string()
            .contains("cannot be greater than max_amount"),
        "Canonical normalizer must prevent lexicographical ordering inversion: got {}",
        err_lex
    );

    // min == max succeeds (exact amount filter)
    let filter_equal = VoucherSearchFilter {
        min_amount: Some("250.75".to_string()),
        max_amount: Some("250.75".to_string()),
        ..Default::default()
    };
    validate_search_filter(&filter_equal).expect("Equal min and max amount should pass");
}

#[test]
fn test_10_exact_monetary_input_parsing() {
    // 100 -> 25 digits + .0000
    let n1 = normalize_canonical_amount_for_query("100", "min_amount").unwrap();
    assert_eq!(n1, "0000000000000000000000100.0000");
    assert_eq!(n1.len(), 30);

    // 100.00 -> identical to 100
    let n2 = normalize_canonical_amount_for_query("100.00", "min_amount").unwrap();
    assert_eq!(n2, "0000000000000000000000100.0000");
    assert_eq!(n1, n2);

    // 0.05 -> fraction preserved and zero-padded to 4 decimal places
    let n3 = normalize_canonical_amount_for_query("0.05", "test").unwrap();
    assert_eq!(n3, "0000000000000000000000000.0500");
    assert_eq!(n3.len(), 30);

    // 2.50 -> 2.5000
    let n4 = normalize_canonical_amount_for_query("2.50", "test").unwrap();
    assert_eq!(n4, "0000000000000000000000002.5000");
    assert_eq!(n4.len(), 30);

    // Trimming whitespace
    let n5 = normalize_canonical_amount_for_query("   42.1234   ", "test").unwrap();
    assert_eq!(n5, "0000000000000000000000042.1234");
    assert_eq!(n5.len(), 30);
}

#[test]
fn test_11_maximum_supported_canonical_amount() {
    // 25 nines integer (10^25 - 1)
    let max_int_canonical = "9999999999999999999999999";
    assert_eq!(max_int_canonical.len(), 25);
    let norm = normalize_canonical_amount_for_query(max_int_canonical, "max_amount").unwrap();
    assert_eq!(norm, "9999999999999999999999999.0000");
    assert_eq!(norm.len(), 30);

    // Maximum supported canonical amount with 4 decimal places (25 nines + '.' + 4 nines)
    let max_with_frac = "9999999999999999999999999.9999";
    assert_eq!(max_with_frac.len(), 30);
    let norm_frac = normalize_canonical_amount_for_query(max_with_frac, "max_amount").unwrap();
    assert_eq!(norm_frac, "9999999999999999999999999.9999");
    assert_eq!(norm_frac.len(), 30);

    // 26 integer digits fails (> 25)
    let too_long_int = "10000000000000000000000000"; // 26 digits
    assert!(normalize_canonical_amount_for_query(too_long_int, "max_amount").is_err());
}

#[test]
fn test_12_negative_and_signed_amount_rejection() {
    // Explicit negative sign fails closed
    let err_neg = normalize_canonical_amount_for_query("-50.00", "min_amount").unwrap_err();
    assert!(err_neg.to_string().contains("cannot contain explicit sign"));

    // Explicit positive sign fails closed
    let err_pos = normalize_canonical_amount_for_query("+50.00", "min_amount").unwrap_err();
    assert!(err_pos.to_string().contains("cannot contain explicit sign"));

    // Small negative fails
    assert!(normalize_canonical_amount_for_query("-0.01", "min_amount").is_err());
}

#[test]
fn test_13_zero_amount_normalization() {
    // "0"
    let z1 = normalize_canonical_amount_for_query("0", "min_amount").unwrap();
    assert_eq!(z1, "0000000000000000000000000.0000");
    assert_eq!(z1.len(), 30);

    // "0.00"
    let z2 = normalize_canonical_amount_for_query("0.00", "min_amount").unwrap();
    assert_eq!(z2, "0000000000000000000000000.0000");
    assert_eq!(z1, z2);

    // "0.0000"
    let z3 = normalize_canonical_amount_for_query("0.0000", "min_amount").unwrap();
    assert_eq!(z3, "0000000000000000000000000.0000");
    assert_eq!(z1, z3);
}

#[test]
fn test_14_invalid_monetary_precision_and_format() {
    // Scale > 4 places rejected
    let err_prec = normalize_canonical_amount_for_query("10.12345", "min_amount").unwrap_err();
    assert!(err_prec
        .to_string()
        .contains("exceeds maximum 4 decimal places"));

    // Trailing decimal without digits
    let err_trail = normalize_canonical_amount_for_query("100.", "min_amount").unwrap_err();
    assert!(err_trail.to_string().contains("trailing decimal point"));

    // Multiple decimal points
    let err_multi = normalize_canonical_amount_for_query("10.20.30", "min_amount").unwrap_err();
    assert!(
        err_multi.to_string().contains("multiple decimal points")
            || err_multi.to_string().contains("Invalid decimal format")
    );

    // Non-numeric strings
    assert!(normalize_canonical_amount_for_query("abc", "test").is_err());
    assert!(normalize_canonical_amount_for_query("12a.34", "test").is_err());

    // Empty and whitespace-only
    assert!(normalize_canonical_amount_for_query("", "test").is_err());
    assert!(normalize_canonical_amount_for_query("   ", "test").is_err());
}

#[test]
fn test_15_invalid_identifiers() {
    // Empty voucher_type_id
    let f1 = VoucherSearchFilter {
        voucher_type_id: Some("   ".to_string()),
        ..Default::default()
    };
    assert!(validate_search_filter(&f1).is_err());

    // Empty series_id
    let f2 = VoucherSearchFilter {
        series_id: Some("".to_string()),
        ..Default::default()
    };
    assert!(validate_search_filter(&f2).is_err());

    // Empty ledger_id
    let f3 = VoucherSearchFilter {
        ledger_id: Some("  ".to_string()),
        ..Default::default()
    };
    assert!(validate_search_filter(&f3).is_err());

    // Empty reversal_of_voucher_id
    let f4 = VoucherSearchFilter {
        reversal_of_voucher_id: Some(" ".to_string()),
        ..Default::default()
    };
    assert!(validate_search_filter(&f4).is_err());
}

#[test]
fn test_16_text_search_query_handling() {
    // Empty / whitespace query is accepted (ignored as no-op by query builder)
    let f_empty = VoucherSearchFilter {
        search_query: Some("".to_string()),
        ..Default::default()
    };
    validate_search_filter(&f_empty).expect("Empty search query should pass validation");

    let f_ws = VoucherSearchFilter {
        search_query: Some("   ".to_string()),
        ..Default::default()
    };
    validate_search_filter(&f_ws).expect("Whitespace search query should pass validation");

    // Exact 200 characters is accepted
    let query_200 = "a".repeat(200);
    let f_200 = VoucherSearchFilter {
        search_query: Some(query_200),
        ..Default::default()
    };
    validate_search_filter(&f_200).expect("200 character search query should pass validation");

    // 201 characters fails
    let query_201 = "a".repeat(201);
    let f_201 = VoucherSearchFilter {
        search_query: Some(query_201),
        ..Default::default()
    };
    let err = validate_search_filter(&f_201).unwrap_err();
    assert!(err.to_string().contains("cannot exceed 200 characters"));
}

#[test]
fn test_17_optional_filters_omitted() {
    // Default filter has all None - must succeed validation
    let default_filter = VoucherSearchFilter::default();
    validate_search_filter(&default_filter)
        .expect("Default filter with all None must pass validation");
}

#[test]
fn test_18_serialization_deserialization_round_trip() {
    // 1. VoucherSearchFilter
    let filter = VoucherSearchFilter {
        voucher_type_id: Some("vt_1".to_string()),
        series_id: Some("s_1".to_string()),
        status: Some("posted".to_string()),
        ledger_id: Some("l_1".to_string()),
        date_basis: Some(DateBasis::EffectiveDate),
        date_from: Some("2025-01-01".to_string()),
        date_to: Some("2025-12-31".to_string()),
        min_amount: Some("10.00".to_string()),
        max_amount: Some("999.99".to_string()),
        search_query: Some("query".to_string()),
        reversal_of_voucher_id: Some("rev_orig_1".to_string()),
        page: Some(3),
        page_size: Some(25),
        sort_by: Some(VoucherSortField::VoucherNumber),
        sort_direction: Some(SortDirection::Asc),
    };
    let json = serde_json::to_string(&filter).expect("Serialize filter");
    let deserialized: VoucherSearchFilter =
        serde_json::from_str(&json).expect("Deserialize filter");
    assert_eq!(filter, deserialized);

    // 2. VoucherRegisterItem
    let item = VoucherRegisterItem {
        id: "v_1".to_string(),
        company_id: "comp_1".to_string(),
        voucher_type_id: "vt_1".to_string(),
        voucher_type_name: "Sales".to_string(),
        series_id: "s_1".to_string(),
        series_name: "Main Series".to_string(),
        voucher_number: "INV-001".to_string(),
        date: "2025-04-10".to_string(),
        effective_date: "2025-04-10".to_string(),
        status: "posted".to_string(),
        reference_no: Some("REF-99".to_string()),
        reference_date: Some("2025-04-09".to_string()),
        narration: Some("Test sales voucher".to_string()),
        total_amount: Some("1500.0000".to_string()),
        line_count: 2,
        reversal_of_voucher_id: None,
        reversal_voucher_id: None,
        cancellation_reason: None,
        created_at: "2025-04-10T10:00:00Z".to_string(),
        updated_at: "2025-04-10T10:00:00Z".to_string(),
    };
    let json_item = serde_json::to_string(&item).expect("Serialize register item");
    let deserialized_item: VoucherRegisterItem =
        serde_json::from_str(&json_item).expect("Deserialize register item");
    assert_eq!(item, deserialized_item);

    // 3. VoucherLineageResponse
    let lineage = VoucherLineageResponse {
        items: vec![item.clone()],
        has_exceeded_depth_limit: false,
        max_depth_reached: 1,
    };
    let json_lineage = serde_json::to_string(&lineage).expect("Serialize lineage");
    let deserialized_lineage: VoucherLineageResponse =
        serde_json::from_str(&json_lineage).expect("Deserialize lineage");
    assert_eq!(lineage, deserialized_lineage);

    // 4. PaginatedResponse<VoucherRegisterItem>
    let paginated = PaginatedResponse {
        items: vec![item],
        total: 1,
        page: 1,
        page_size: 20,
        total_pages: 1,
    };
    let json_paginated = serde_json::to_string(&paginated).expect("Serialize paginated");
    let deserialized_paginated: PaginatedResponse<VoucherRegisterItem> =
        serde_json::from_str(&json_paginated).expect("Deserialize paginated");
    assert_eq!(paginated, deserialized_paginated);
}

#[test]
fn test_19_decimal_representability_gap_and_boundary_verification() {
    use rust_decimal::Decimal;

    // 1. Empirical proof of rust_decimal's 96-bit limit:
    // (2^96 - 1) / 10^4 = 7922816251426433759354395.0335
    let max_decimal_str = "7922816251426433759354395.0335";
    let above_decimal_str = "7922816251426433759354395.0336";
    let schema_max_str = "9999999999999999999999999.9999";

    // Decimal succeeds at exact boundary
    assert!(
        Decimal::from_str_exact(max_decimal_str).is_ok(),
        "Decimal must parse max 96-bit scale-4 value"
    );

    // Decimal FAILS immediately above 96-bit boundary (gap confirmed)
    assert!(
        Decimal::from_str_exact(above_decimal_str).is_err(),
        "Decimal must fail on 7922816251426433759354395.0336 due to 96-bit mantissa overflow"
    );

    // Decimal FAILS on maximum schema-supported value (gap confirmed)
    assert!(
        Decimal::from_str_exact(schema_max_str).is_err(),
        "Decimal must fail on 9999999999999999999999999.9999 due to 96-bit mantissa overflow"
    );

    // 2. Proof that our canonical normalizer correctly handles all values across the gap:
    let below_norm =
        normalize_canonical_amount_for_query("7922816251426433759354395.0334", "test").unwrap();
    let at_norm = normalize_canonical_amount_for_query(max_decimal_str, "test").unwrap();
    let above_norm = normalize_canonical_amount_for_query(above_decimal_str, "test").unwrap();
    let max_schema_norm = normalize_canonical_amount_for_query(schema_max_str, "test").unwrap();

    assert_eq!(below_norm, "7922816251426433759354395.0334");
    assert_eq!(at_norm, "7922816251426433759354395.0335");
    assert_eq!(above_norm, "7922816251426433759354395.0336");
    assert_eq!(max_schema_norm, "9999999999999999999999999.9999");

    // All lengths exactly 30
    assert_eq!(below_norm.len(), 30);
    assert_eq!(at_norm.len(), 30);
    assert_eq!(above_norm.len(), 30);
    assert_eq!(max_schema_norm.len(), 30);

    // Lexicographical ordering matches numeric ordering across the Decimal boundary
    assert!(below_norm < at_norm);
    assert!(at_norm < above_norm);
    assert!(above_norm < max_schema_norm);
}

#[test]
fn test_20_property_test_exact_30_chars_and_equivalence() {
    // Property 1: All valid inputs normalize to exactly 30 characters
    let sample_inputs = [
        "0",
        "0.0000",
        "0.0001",
        "0.05",
        "0.5000",
        "1",
        "1.0000",
        "42.1234",
        "123456789.9876",
        "7922816251426433759354395.0335",
        "7922816251426433759354395.0336",
        "9999999999999999999999999",
        "9999999999999999999999999.9999",
    ];

    for input in &sample_inputs {
        let norm = normalize_canonical_amount_for_query(input, "test")
            .unwrap_or_else(|e| panic!("Failed to normalize '{}': {}", input, e));
        assert_eq!(
            norm.len(),
            30,
            "Normalized string for '{}' must be exactly 30 characters, got '{}' ({})",
            input,
            norm,
            norm.len()
        );
        assert_eq!(
            &norm[25..26],
            ".",
            "Dot must be at index 25 for '{}'",
            input
        );
    }

    // Property 2: Equivalent monetary representations normalize identically
    let equivalence_groups: Vec<Vec<&str>> = vec![
        vec!["0", "0.0", "0.00", "0.000", "0.0000", "00", "00.0000"],
        vec!["1", "1.0", "1.00", "1.000", "1.0000", "01", "0001.0000"],
        vec!["100", "100.0", "100.00", "100.0000", "0100", "00100.00"],
        vec!["2.5", "2.50", "2.500", "2.5000", "02.50", "002.5000"],
        vec![
            "9999999999999999999999999",
            "9999999999999999999999999.0",
            "9999999999999999999999999.00",
            "9999999999999999999999999.0000",
        ],
    ];

    for group in equivalence_groups {
        let first_norm = normalize_canonical_amount_for_query(group[0], "test").unwrap();
        for variant in &group[1..] {
            let variant_norm = normalize_canonical_amount_for_query(variant, "test").unwrap();
            assert_eq!(
                first_norm, variant_norm,
                "Variants '{}' and '{}' should normalize identically",
                group[0], variant
            );
        }
    }
}

#[test]
fn test_21_property_test_lexicographical_equals_numeric_order() {
    // Strictly monotonically increasing sequence across the entire schema range
    let ordered_inputs = [
        "0",
        "0.0001",
        "0.0002",
        "0.05",
        "0.5",
        "1",
        "1.0001",
        "2.5",
        "10",
        "10.0001",
        "99.99",
        "100",
        "1000",
        "999999",
        "7922816251426433759354395.0334",
        "7922816251426433759354395.0335",
        "7922816251426433759354395.0336",
        "9999999999999999999999999",
        "9999999999999999999999999.9999",
    ];

    let normalized: Vec<String> = ordered_inputs
        .iter()
        .map(|inp| normalize_canonical_amount_for_query(inp, "test").unwrap())
        .collect();

    for i in 0..normalized.len() - 1 {
        assert!(
            normalized[i] < normalized[i + 1],
            "Monotonicity violation: '{}' ({}) must be strictly less than '{}' ({})",
            ordered_inputs[i],
            normalized[i],
            ordered_inputs[i + 1],
            normalized[i + 1]
        );
    }
}

#[test]
fn test_22_malformed_input_rejections() {
    let invalid_inputs = [
        ("", "cannot be empty"),
        ("   ", "cannot be empty"),
        ("+10.00", "cannot contain explicit sign"),
        ("-10.00", "cannot contain explicit sign"),
        ("-0.0001", "cannot contain explicit sign"),
        ("1.2.3", "multiple decimal points"),
        ("10.20.30", "multiple decimal points"),
        ("100.", "trailing decimal point without fractional digits"),
        (".50", "missing integer part before decimal"),
        ("abc", "contains non-digit characters"),
        ("12a.34", "contains non-digit characters"),
        ("12.34a", "contains non-digit characters"),
        ("$100", "contains non-digit characters"),
        ("10.12345", "exceeds maximum 4 decimal places"),
        ("0.00001", "exceeds maximum 4 decimal places"),
        (
            "10000000000000000000000000",
            "integer part exceeds maximum 25 digits",
        ),
        (
            "10000000000000000000000000.0000",
            "integer part exceeds maximum 25 digits",
        ),
    ];

    for (invalid_input, expected_err) in &invalid_inputs {
        let res = normalize_canonical_amount_for_query(invalid_input, "test");
        assert!(
            res.is_err(),
            "Expected input '{}' to be rejected",
            invalid_input
        );
        let err_str = res.unwrap_err().to_string();
        assert!(
            err_str.contains(expected_err),
            "Expected error for '{}' to contain '{}', got '{}'",
            invalid_input,
            expected_err,
            err_str
        );
    }
}
