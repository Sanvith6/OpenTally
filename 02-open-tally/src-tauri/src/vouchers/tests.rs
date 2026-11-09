//! Stage 2: Voucher Type & Numbering-Series Masters Test Suite
//! Covers 12 mapped Stage 2 scenarios (TC-VCH-81, 82, 89, 90, 91, 98, 99, 103, 104, 111, 150, 154)
//! plus predefined types listing, get type, series CRUD, validation, and tenant isolation.

use crate::database::migrations::{
    run_sprint1_migration, run_sprint2_migration, run_stage3_migration,
};
use crate::database::DatabaseManager;
use crate::vouchers::formatter::CanonicalVoucherFormatter;
use crate::vouchers::model::*;
use crate::vouchers::service::{VoucherSeriesService, VoucherTypeService};
use crate::vouchers::validator::*;
use sqlx::{Pool, Sqlite};

struct TestDb {
    pub db: DatabaseManager,
    _temp_dir: tempfile::TempDir,
}

impl TestDb {
    pub async fn new_migrated_sprint2() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
        let db_path = temp_dir.path().join("opentally_stage2_test.db");
        let db = DatabaseManager::init(&db_path)
            .await
            .expect("Failed to init database");
        let pool = db.pool();
        run_sprint1_migration(pool)
            .await
            .expect("Sprint 1 migration failed");
        run_stage3_migration(pool)
            .await
            .expect("Stage 3 migration failed");
        run_sprint2_migration(pool)
            .await
            .expect("Sprint 2 migration failed");
        Self {
            db,
            _temp_dir: temp_dir,
        }
    }
}

async fn insert_test_company(pool: &Pool<Sqlite>, company_id: &str, name: &str) {
    sqlx::query(
        "INSERT OR IGNORE INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ($1, $2, $2, 'India', 'Maharashtra', '2026-04-01', '2026-04-01')"
    )
    .bind(company_id)
    .bind(name)
    .execute(pool)
    .await
    .unwrap();
}

// -------------------------------------------------------------------------
// TC-VCH-81: Manual and automatic numbering collision isolation
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_81_manual_and_auto_numbering_collision_isolation() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_81", "Company 81").await;

    // Create automatic series: series_code = "AUTO", prefix = "INV/"
    let auto_series = VoucherSeriesService::create_series(
        pool,
        "comp_81",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Automatic Sales".to_string(),
            series_code: "AUTO".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("INV/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .expect("Failed to create automatic series");

    // Create manual series: series_code = "MAN", prefix = "MAN/"
    let manual_series = VoucherSeriesService::create_series(
        pool,
        "comp_81",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Manual Sales".to_string(),
            series_code: "MAN".to_string(),
            numbering_method: "manual".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("MAN/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .expect("Failed to create manual series");

    assert_eq!(auto_series.numbering_method, "automatic");
    assert_eq!(manual_series.numbering_method, "manual");

    // Format automatic numbers: INV/26-27/0001, INV/26-27/0002
    let num1 =
        CanonicalVoucherFormatter::format_automatic("AUTO", "INV/{YY-YY}/", "", 1, 4, "2026-04-01");
    assert_eq!(num1, "INV/26-27/0001");

    // Parse valid manual voucher number: MAN/26-27/0001
    let parsed_manual = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN/26-27/0001",
        "MAN",
        "MAN/{YY-YY}/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &["AUTO".to_string(), "MAN".to_string()],
    )
    .expect("Manual voucher number should be valid");

    assert_eq!(parsed_manual.series_code, "MAN");
    assert_eq!(parsed_manual.sequence_number, Some(1));
    // Numbering namespaces are strictly disjoint
    assert_ne!(num1, "MAN/26-27/0001");
}

// -------------------------------------------------------------------------
// TC-VCH-82: Rejection of shared prefix/suffix between manual and automatic series
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_82_rejection_of_shared_prefix_suffix_between_manual_and_auto() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_82", "Company 82").await;

    // Create automatic series with prefix "INV/{YY-YY}/"
    VoucherSeriesService::create_series(
        pool,
        "comp_82",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Auto Sales".to_string(),
            series_code: "AUTO".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("INV/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .expect("Auto series creation should succeed");

    // Attempting to create manual series with the exact same prefix/suffix must be rejected
    let res = VoucherSeriesService::create_series(
        pool,
        "comp_82",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Manual Sales".to_string(),
            series_code: "MAN".to_string(),
            numbering_method: "manual".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("INV/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await;

    assert!(res.is_err(), "Shared prefix/suffix between manual and automatic series under same voucher type must be rejected");
    let err = res.err().unwrap().to_string();
    assert!(err.contains("identical prefix") || err.contains("Namespaces must be distinct"));
}

// -------------------------------------------------------------------------
// TC-VCH-89: Multi-series namespace overlap rejection (validate_series_namespace_isolation)
// -------------------------------------------------------------------------
#[test]
fn test_tc_vch_89_multi_series_namespace_overlap_rejection() {
    let s_auto1 = SeriesNamespaceInfo {
        series_name: "Auto-Retail".to_string(),
        series_code: "RET".to_string(),
        numbering_method: "automatic".to_string(),
        prefix: "RET/".to_string(),
        suffix: "".to_string(),
    };
    let s_auto2 = SeriesNamespaceInfo {
        series_name: "Auto-Wholesale".to_string(),
        series_code: "WHO".to_string(),
        numbering_method: "automatic".to_string(),
        prefix: "WHO/".to_string(),
        suffix: "".to_string(),
    };
    let s_man1 = SeriesNamespaceInfo {
        series_name: "Manual-Direct".to_string(),
        series_code: "MAN_DIR".to_string(),
        numbering_method: "manual".to_string(),
        prefix: "MAN-DIR/".to_string(),
        suffix: "".to_string(),
    };

    let existing = vec![s_auto1, s_auto2, s_man1];

    // 1. Valid distinct series succeeds
    let valid_series = SeriesNamespaceInfo {
        series_name: "Manual-Online".to_string(),
        series_code: "MAN_ONL".to_string(),
        numbering_method: "manual".to_string(),
        prefix: "MAN-ONL/".to_string(),
        suffix: "".to_string(),
    };
    assert!(validate_series_namespace_isolation(&existing, &valid_series).is_ok());

    // 2. Duplicate prefix/suffix on automatic series rejected
    let dup_auto = SeriesNamespaceInfo {
        series_name: "Auto-Conflict".to_string(),
        series_code: "CONFLICT_AUTO".to_string(),
        numbering_method: "automatic".to_string(),
        prefix: "RET/".to_string(),
        suffix: "".to_string(),
    };
    let res = validate_series_namespace_isolation(&existing, &dup_auto);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("identical prefix"));

    // 3. Duplicate prefix/suffix on manual series rejected
    let dup_man = SeriesNamespaceInfo {
        series_name: "Manual-Conflict".to_string(),
        series_code: "CONFLICT_MAN".to_string(),
        numbering_method: "manual".to_string(),
        prefix: "MAN-DIR/".to_string(),
        suffix: "".to_string(),
    };
    let res = validate_series_namespace_isolation(&existing, &dup_man);
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("identical prefix"));

    // 4. Ambiguous prefix overlap (e.g. "INV" vs "INV0") rejected
    let s_base = SeriesNamespaceInfo {
        series_name: "Base".to_string(),
        series_code: "BASE".to_string(),
        numbering_method: "automatic".to_string(),
        prefix: "INV".to_string(),
        suffix: "".to_string(),
    };
    let s_num = SeriesNamespaceInfo {
        series_name: "Num".to_string(),
        series_code: "NUM".to_string(),
        numbering_method: "automatic".to_string(),
        prefix: "INV0".to_string(),
        suffix: "".to_string(),
    };
    let res = validate_series_namespace_isolation(&[s_base], &s_num);
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("Ambiguous series prefixes"));
}

// -------------------------------------------------------------------------
// TC-VCH-90: Manual voucher format backend validation
// -------------------------------------------------------------------------
#[test]
fn test_tc_vch_90_manual_voucher_format_backend_validation() {
    // Valid manual number with prefix and suffix
    let res = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-DIR/0042/DEL",
        "MAN_DIR",
        "MAN-DIR/",
        "/DEL",
        4,
        "yearly",
        "2026-04-01",
        &["MAN_DIR".to_string()],
    );
    assert!(res.is_ok());
    let parsed = res.unwrap();
    assert_eq!(parsed.sequence_number, Some(42));
    assert_eq!(parsed.core, "0042");

    // Invalid prefix
    let res_bad_prefix = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "WRONG-PREFIX/0042/DEL",
        "MAN_DIR",
        "MAN-DIR/",
        "/DEL",
        4,
        "yearly",
        "2026-04-01",
        &["MAN_DIR".to_string()],
    );
    assert!(res_bad_prefix.is_err());
    assert!(res_bad_prefix.unwrap_err().to_string().contains("prefix"));

    // Invalid suffix
    let res_bad_suffix = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-DIR/0042/WRONG",
        "MAN_DIR",
        "MAN-DIR/",
        "/DEL",
        4,
        "yearly",
        "2026-04-01",
        &["MAN_DIR".to_string()],
    );
    assert!(res_bad_suffix.is_err());
    assert!(res_bad_suffix.unwrap_err().to_string().contains("suffix"));

    // Empty core
    let res_empty_core = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-DIR//DEL",
        "MAN_DIR",
        "MAN-DIR/",
        "/DEL",
        4,
        "yearly",
        "2026-04-01",
        &["MAN_DIR".to_string()],
    );
    assert!(res_empty_core.is_err());
    assert!(res_empty_core.unwrap_err().to_string().contains("empty"));
}

// -------------------------------------------------------------------------
// TC-VCH-91: Manual voucher usurping another series namespace rejection
// -------------------------------------------------------------------------
#[test]
fn test_tc_vch_91_manual_voucher_usurping_another_series_namespace_rejection() {
    let all_series = vec!["AUTO_RET".to_string(), "MAN_DIR".to_string()];

    // Manual voucher attempting to usurp the automatic series code
    let res = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN/AUTO_RET/0001",
        "MAN_DIR",
        "MAN/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &all_series,
    );
    assert!(res.is_err());
    assert!(res
        .unwrap_err()
        .to_string()
        .contains("usurps series namespace"));

    // Manual voucher starting with another series code
    let res2 = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "AUTO_RET/0001",
        "MAN_DIR",
        "MAN/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &all_series,
    );
    assert!(res2.is_err());
}

// -------------------------------------------------------------------------
// TC-VCH-98: Database-level series code uniqueness constraint uq_series_code
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_98_database_level_series_code_uniqueness() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_98", "Company 98").await;

    // Insert first series with code 'PRI'
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_pri1', 'comp_98', 'vt_sales', 'Primary Series', 'PRI', 'automatic', 'yearly')"
    )
    .execute(pool)
    .await
    .expect("First series insertion should succeed");

    // Second series with identical series_code 'PRI' must fail uq_series_code
    let res = sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_pri2', 'comp_98', 'vt_sales', 'Duplicate Primary', 'PRI', 'automatic', 'yearly')"
    )
    .execute(pool)
    .await;

    assert!(
        res.is_err(),
        "Duplicate series_code must violate uq_series_code"
    );
    let err_str = res.err().unwrap().to_string();
    assert!(err_str.contains("UNIQUE") || err_str.contains("uq_series_code"));
}

// -------------------------------------------------------------------------
// TC-VCH-99: Manual voucher prefix validation and auto-series hijack rejection
// -------------------------------------------------------------------------
#[test]
fn test_tc_vch_99_manual_voucher_prefix_validation_and_auto_hijack_rejection() {
    let all_codes = vec!["AUTO".to_string(), "MAN".to_string()];

    // Hijack attempt using delimiter pattern
    let res = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-AUTO-0001",
        "MAN",
        "MAN-",
        "",
        4,
        "yearly",
        "2026-04-01",
        &all_codes,
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("usurps"));
}

// -------------------------------------------------------------------------
// TC-VCH-103: Normalized numbering series master supports multiple effective-dated configuration versions
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_103_normalized_numbering_series_supports_multiple_configs() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_103", "Company 103").await;

    // Create series master
    let series = VoucherSeriesService::create_series(
        pool,
        "comp_103",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Multi-Year Sales".to_string(),
            series_code: "MYS".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("INV/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .expect("Series creation should succeed");

    // Add second effective configuration for FY 2027-28
    let config2 = VoucherSeriesService::create_configuration(
        pool,
        "comp_103",
        &series.id,
        CreateVoucherNumberConfigRequest {
            applicable_from: "2027-04-01".to_string(),
            prefix: Some("INV/{YY-YY}-B/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .expect("Second configuration version should succeed");

    assert_eq!(config2.applicable_from, "2027-04-01");

    let configs = VoucherSeriesService::list_configurations(pool, "comp_103", &series.id)
        .await
        .expect("Listing configs should succeed");
    assert_eq!(configs.len(), 2);

    // Duplicate applicable_from for same series violates uq_config_series_date
    let dup_res = VoucherSeriesService::create_configuration(
        pool,
        "comp_103",
        &series.id,
        CreateVoucherNumberConfigRequest {
            applicable_from: "2027-04-01".to_string(),
            prefix: Some("INV/{YY-YY}-DUP/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await;
    assert!(
        dup_res.is_err(),
        "Duplicate applicable_from must fail uq_config_series_date"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-104: Database-level series name and code uniqueness constraints on series master
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_104_database_series_name_and_code_uniqueness_on_series_master() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_104", "Company 104").await;

    // Insert master series
    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_104_1', 'comp_104', 'vt_sales', 'Retail Sales', 'RET', 'automatic', 'yearly')"
    )
    .execute(pool)
    .await
    .expect("First insert should succeed");

    // Duplicate series_name under same company and voucher type fails uq_series_name
    let res_name = sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_104_2', 'comp_104', 'vt_sales', 'Retail Sales', 'DIFF_CODE', 'automatic', 'yearly')"
    )
    .execute(pool)
    .await;
    assert!(
        res_name.is_err(),
        "Duplicate series_name must fail uq_series_name"
    );

    // Duplicate series_code under same company and voucher type fails uq_series_code
    let res_code = sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_104_3', 'comp_104', 'vt_sales', 'Different Name', 'RET', 'automatic', 'yearly')"
    )
    .execute(pool)
    .await;
    assert!(
        res_code.is_err(),
        "Duplicate series_code must fail uq_series_code"
    );
}

// -------------------------------------------------------------------------
// TC-VCH-111: Authoritative inverse manual parser and usurpation rejection
// -------------------------------------------------------------------------
#[test]
fn test_tc_vch_111_authoritative_inverse_manual_parser_and_usurpation_rejection() {
    let other_codes = vec!["AUTO".to_string(), "MAN".to_string()];

    // 1. Valid numeric with width padding
    let p1 = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-26-27/0042",
        "MAN",
        "MAN-{YY-YY}/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &other_codes,
    )
    .unwrap();
    assert_eq!(p1.sequence_number, Some(42));
    assert!(p1.is_numeric);

    // 2. Under-padded numeric component (width = 4, length = 2) rejected
    let res_underpadded = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-26-27/42",
        "MAN",
        "MAN-{YY-YY}/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &other_codes,
    );
    assert!(res_underpadded.is_err());
    assert!(res_underpadded
        .unwrap_err()
        .to_string()
        .contains("violates minimum zero-padding"));

    // 3. Zero numeric component rejected
    let res_zero = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-26-27/0000",
        "MAN",
        "MAN-{YY-YY}/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &other_codes,
    );
    assert!(res_zero.is_err());
    assert!(res_zero
        .unwrap_err()
        .to_string()
        .contains("strictly positive"));

    // 4. Valid alphanumeric component
    let p_alpha = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-26-27/DIR-A01",
        "MAN",
        "MAN-{YY-YY}/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &other_codes,
    )
    .unwrap();
    assert_eq!(p_alpha.sequence_number, None);
    assert!(!p_alpha.is_numeric);
    assert_eq!(p_alpha.core, "DIR-A01");

    // 5. Invalid characters in identifier component
    let res_invalid_chars = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "MAN-26-27/DIR@#$",
        "MAN",
        "MAN-{YY-YY}/",
        "",
        4,
        "yearly",
        "2026-04-01",
        &other_codes,
    );
    assert!(res_invalid_chars.is_err());
    assert!(res_invalid_chars
        .unwrap_err()
        .to_string()
        .contains("invalid characters"));

    // 6. Overlapping prefix/suffix length bounds check (prevents slice panic)
    let res_short = CanonicalVoucherFormatter::parse_and_validate_manual_voucher_number(
        "INV/INV",
        "INV",
        "INV/",
        "/INV",
        4,
        "never",
        "2026-04-01",
        &other_codes,
    );
    assert!(res_short.is_err());
    assert!(res_short
        .unwrap_err()
        .to_string()
        .contains("too short to contain both expected prefix"));
}

// -------------------------------------------------------------------------
// TC-VCH-150: Immutable series-level restart_period enforcement via Trigger 12
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_150_immutable_series_restart_period_enforcement() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_150", "Company 150").await;

    sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_150', 'comp_150', 'vt_sales', 'Primary Sales', 'PRI', 'automatic', 'yearly')"
    )
    .execute(pool)
    .await
    .expect("Series insertion should succeed");

    // Updating restart_period must be aborted by Trigger 12
    let res = sqlx::query(
        "UPDATE voucher_number_series SET restart_period = 'monthly' WHERE id = 's_150'",
    )
    .execute(pool)
    .await;

    assert!(
        res.is_err(),
        "Trigger 12 must prevent mutating restart_period"
    );
    let err_str = res.err().unwrap().to_string();
    assert!(err_str.contains("Series restart_period is permanently immutable"));
}

// -------------------------------------------------------------------------
// TC-VCH-154: Explicit mandatory restart_period selection without SQL default
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_tc_vch_154_explicit_mandatory_restart_period_without_sql_default() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_154", "Company 154").await;

    // 1. Omitting restart_period should fail NOT NULL constraint
    let res_null = sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method)
         VALUES ('s_154_null', 'comp_154', 'vt_sales', 'Null Reset', 'NUL', 'automatic')"
    )
    .execute(pool)
    .await;
    assert!(res_null.is_err());
    let err_null = res_null.err().unwrap().to_string();
    assert!(err_null.contains("NOT NULL") || err_null.contains("restart_period"));

    // 2. Invalid restart_period value fails CHECK constraint
    let res_invalid = sqlx::query(
        "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
         VALUES ('s_154_inv', 'comp_154', 'vt_sales', 'Invalid Reset', 'INV_PER', 'automatic', 'quarterly')"
    )
    .execute(pool)
    .await;
    assert!(res_invalid.is_err());
    let err_inv = res_invalid.err().unwrap().to_string();
    assert!(err_inv.contains("CHECK"));
}

// -------------------------------------------------------------------------
// Listing 24 predefined voucher types per company
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_list_24_predefined_voucher_types_per_company() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_24", "Company 24").await;

    let types = VoucherTypeService::list_voucher_types(pool, "comp_24")
        .await
        .expect("Listing voucher types should succeed");

    assert_eq!(
        types.len(),
        24,
        "Expected exactly 24 predefined voucher types"
    );

    let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"Contra"));
    assert!(names.contains(&"Payment"));
    assert!(names.contains(&"Receipt"));
    assert!(names.contains(&"Journal"));
    assert!(names.contains(&"Sales"));
    assert!(names.contains(&"Purchase"));
    assert!(names.contains(&"Credit Note"));
    assert!(names.contains(&"Debit Note"));
    assert!(names.contains(&"Reversing Journal"));
    assert!(names.contains(&"Memorandum"));
    assert!(names.contains(&"Sales Order"));
    assert!(names.contains(&"Purchase Order"));
    assert!(names.contains(&"Delivery Note"));
    assert!(names.contains(&"Receipt Note"));
    assert!(names.contains(&"Rejections In"));
    assert!(names.contains(&"Rejections Out"));
    assert!(names.contains(&"Stock Journal"));
    assert!(names.contains(&"Physical Stock"));
    assert!(names.contains(&"Material In"));
    assert!(names.contains(&"Material Out"));
    assert!(names.contains(&"Job Work In Order"));
    assert!(names.contains(&"Job Work Out Order"));
    assert!(names.contains(&"Attendance"));
    assert!(names.contains(&"Payroll"));

    // Verify properties of Sales voucher type
    let sales = types.iter().find(|t| t.name == "Sales").unwrap();
    assert!(sales.affects_ledger_balance);
    assert!(!sales.affects_inventory);
    assert!(sales.is_predefined);

    // Verify properties of Delivery Note
    let del_note = types.iter().find(|t| t.name == "Delivery Note").unwrap();
    assert!(!del_note.affects_ledger_balance);
    assert!(del_note.affects_inventory);

    // Verify category of Attendance
    let attendance = types.iter().find(|t| t.name == "Attendance").unwrap();
    assert_eq!(attendance.category, "payroll");
}

// -------------------------------------------------------------------------
// Getting single voucher type details
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_get_single_voucher_type_details() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_details", "Company Details").await;

    let vtype = VoucherTypeService::get_voucher_type(pool, "comp_details", "vt_sales")
        .await
        .expect("Should retrieve sales voucher type");
    assert_eq!(vtype.id, "vt_sales");
    assert_eq!(vtype.name, "Sales");
    assert_eq!(vtype.category, "accounting");

    // Nonexistent type returns NotFound
    let res = VoucherTypeService::get_voucher_type(pool, "comp_details", "nonexistent").await;
    assert!(res.is_err());
    assert!(matches!(
        res.err().unwrap(),
        crate::errors::OpenTallyError::NotFound(_)
    ));
}

// -------------------------------------------------------------------------
// Cross-company tenant isolation for voucher types and series
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_cross_company_tenant_isolation_voucher_types_and_series() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_iso_1", "Company Isolation 1").await;
    insert_test_company(pool, "comp_iso_2", "Company Isolation 2").await;

    let s1 = VoucherSeriesService::create_series(
        pool,
        "comp_iso_1",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Comp 1 Series".to_string(),
            series_code: "C1".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("C1/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(1),
            width: Some(4),
        },
    )
    .await
    .unwrap();

    // Querying series from Company 2 returns NotFound
    let res = VoucherSeriesService::get_series(pool, "comp_iso_2", &s1.id).await;
    assert!(res.is_err());
    assert!(matches!(
        res.err().unwrap(),
        crate::errors::OpenTallyError::NotFound(_)
    ));

    // Listing series for Company 2 voucher type does not include Company 1 series
    let list_c2 = VoucherSeriesService::list_series_by_voucher_type(pool, "comp_iso_2", "vt_sales")
        .await
        .unwrap();
    assert!(!list_c2.iter().any(|s| s.id == s1.id));
}

// -------------------------------------------------------------------------
// Series CRUD lifecycle
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_series_crud_lifecycle() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    insert_test_company(pool, "comp_crud", "Company CRUD").await;

    // 1. Create
    let series = VoucherSeriesService::create_series(
        pool,
        "comp_crud",
        CreateVoucherSeriesRequest {
            voucher_type_id: "vt_sales".to_string(),
            series_name: "Export Sales".to_string(),
            series_code: "EXP".to_string(),
            numbering_method: "automatic".to_string(),
            restart_period: "yearly".to_string(),
            applicable_from: Some("2026-04-01".to_string()),
            prefix: Some("EXP/{YY-YY}/".to_string()),
            suffix: None,
            start_number: Some(100),
            width: Some(5),
        },
    )
    .await
    .expect("Create series should succeed");
    assert_eq!(series.series_name, "Export Sales");
    assert_eq!(series.series_code, "EXP");

    // 2. List
    let series_list =
        VoucherSeriesService::list_series_by_voucher_type(pool, "comp_crud", "vt_sales")
            .await
            .unwrap();
    assert!(series_list.iter().any(|s| s.id == series.id));

    // 3. Get
    let fetched = VoucherSeriesService::get_series(pool, "comp_crud", &series.id)
        .await
        .unwrap();
    assert_eq!(fetched.id, series.id);

    // 4. Update series name
    let updated = VoucherSeriesService::update_series(
        pool,
        "comp_crud",
        &series.id,
        UpdateVoucherSeriesRequest {
            series_name: Some("Overseas Export Sales".to_string()),
            is_active: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.series_name, "Overseas Export Sales");

    // 5. Deactivate
    let deactivated = VoucherSeriesService::deactivate_series(pool, "comp_crud", &series.id)
        .await
        .unwrap();
    assert!(!deactivated.is_active);

    // 6. Delete
    VoucherSeriesService::delete_series(pool, "comp_crud", &series.id)
        .await
        .expect("Delete unused series should succeed");
    assert!(
        VoucherSeriesService::get_series(pool, "comp_crud", &series.id)
            .await
            .is_err()
    );
}

// -------------------------------------------------------------------------
// Input validation on series code and series name
// -------------------------------------------------------------------------
#[test]
fn test_input_validation_on_series_code_and_name() {
    // Valid code
    assert!(validate_series_code("RET").is_ok());
    assert!(validate_series_code("MAN_01").is_ok());
    assert!(validate_series_code("SER-1").is_ok());

    // Invalid code
    assert!(validate_series_code("").is_err());
    assert!(validate_series_code("CODE WITH SPACE").is_err());
    assert!(validate_series_code("CODE@#$").is_err());

    // Valid name
    assert!(validate_series_name("Retail Sales").is_ok());

    // Invalid name
    assert!(validate_series_name("").is_err());
    assert!(validate_series_name("   ").is_err());
    let long_name = "A".repeat(101);
    assert!(validate_series_name(&long_name).is_err());
}

// -------------------------------------------------------------------------
// Validate configuration tokens across periodic resets
// -------------------------------------------------------------------------
#[test]
fn test_validate_configuration_tokens_periodic_resets() {
    // Indian FY (non-calendar 04-01): requires {YY-YY} or {YYYY-YYYY}, rejects bare {YYYY}
    assert!(validate_configuration_tokens("yearly", "2026-04-01", "INV/{YY-YY}/", "").is_ok());
    assert!(validate_configuration_tokens("yearly", "2026-04-01", "INV/{YYYY-YYYY}/", "").is_ok());
    assert!(validate_configuration_tokens("yearly", "2026-04-01", "INV/{YYYY}/", "").is_err());
    assert!(validate_configuration_tokens("yearly", "2026-04-01", "INV/", "").is_err());

    // Calendar FY (01-01): allows {YYYY}
    assert!(validate_configuration_tokens("yearly", "2026-01-01", "INV/{YYYY}/", "").is_ok());
    assert!(validate_configuration_tokens("yearly", "2026-01-01", "INV/", "").is_err());

    // Monthly reset: requires {YYYY-MM}
    assert!(validate_configuration_tokens("monthly", "2026-04-01", "INV/{YYYY-MM}/", "").is_ok());
    assert!(validate_configuration_tokens("monthly", "2026-04-01", "INV/", "").is_err());

    // Daily reset: requires {YYYYMMDD} or {YYYY-MM-DD}
    assert!(validate_configuration_tokens("daily", "2026-04-01", "INV/{YYYYMMDD}/", "").is_ok());
    assert!(validate_configuration_tokens("daily", "2026-04-01", "INV/{YYYY-MM-DD}/", "").is_ok());
    assert!(validate_configuration_tokens("daily", "2026-04-01", "INV/", "").is_err());

    // Never reset: allows static template
    assert!(validate_configuration_tokens("never", "2026-04-01", "INV/", "").is_ok());
}

// -------------------------------------------------------------------------
// Validation: Calendar date enforcement on applicable_from
// -------------------------------------------------------------------------
#[tokio::test]
async fn test_calendar_date_validation_on_applicable_from() {
    let test_db = TestDb::new_migrated_sprint2().await;
    let pool = test_db.db.pool();
    let company_id = "comp_val_date";
    insert_test_company(pool, company_id, "Company Val Date").await;

    let types = VoucherTypeService::list_voucher_types(pool, company_id)
        .await
        .unwrap();
    let sales_type = types.iter().find(|t| t.name == "Sales").unwrap();

    // 1. Invalid date in create_series
    let invalid_series_req = CreateVoucherSeriesRequest {
        voucher_type_id: sales_type.id.clone(),
        series_name: "Bad Date Series".into(),
        series_code: "BAD".into(),
        numbering_method: "automatic".into(),
        restart_period: "never".into(),
        prefix: Some("BAD/".into()),
        suffix: None,
        applicable_from: Some("2026-99-99".into()), // Invalid calendar date
        start_number: Some(1),
        width: Some(4),
    };

    let res_create =
        VoucherSeriesService::create_series(pool, company_id, invalid_series_req).await;
    assert!(res_create.is_err());
    assert!(res_create
        .unwrap_err()
        .to_string()
        .contains("Invalid calendar date"));

    // 2. Valid series creation
    let valid_series_req = CreateVoucherSeriesRequest {
        voucher_type_id: sales_type.id.clone(),
        series_name: "Valid Series".into(),
        series_code: "VAL".into(),
        numbering_method: "automatic".into(),
        restart_period: "never".into(),
        prefix: Some("VAL/".into()),
        suffix: None,
        applicable_from: Some("2026-04-01".into()),
        start_number: Some(1),
        width: Some(4),
    };
    let created_series = VoucherSeriesService::create_series(pool, company_id, valid_series_req)
        .await
        .unwrap();

    // 3. Invalid date in create_configuration
    let invalid_config_req = CreateVoucherNumberConfigRequest {
        applicable_from: "2026-02-29".into(), // 2026 is not a leap year!
        prefix: Some("VAL/".into()),
        suffix: None,
        start_number: Some(10),
        width: Some(4),
    };
    let res_config = VoucherSeriesService::create_configuration(
        pool,
        company_id,
        &created_series.id,
        invalid_config_req,
    )
    .await;
    assert!(res_config.is_err());
    assert!(res_config
        .unwrap_err()
        .to_string()
        .contains("Invalid calendar date"));
}
