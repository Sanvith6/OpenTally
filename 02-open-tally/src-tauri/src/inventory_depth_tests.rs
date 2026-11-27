use crate::commercial::{
    CommercialDocumentType, CommercialLinePosting, CommercialPostingInput,
    CommercialPostingService, TaxPostingLine,
};
use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::gst::GstService;
use crate::inventory::pricing::{create_price_level, resolve_item_price, upsert_price_list_entry};
use crate::inventory::units::{convert_to_base_quantity_tx, create_compound_unit};
use rust_decimal::Decimal;
use std::str::FromStr;
use tempfile::TempDir;
use uuid::Uuid;

async fn setup_m20_db() -> (TempDir, DatabaseManager, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("m20_test.db");
    let db = DatabaseManager::init(&db_path).await.expect("init db");
    crate::database::migrations::run_all_migrations(db.pool())
        .await
        .expect("migrations");
    let comp = CompanyService::create_company(
        db.pool(),
        CreateCompanyRequest {
            name: "M20 Distributor Corp".to_string(),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Maharashtra".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .expect("create company");

    GstService::save_company_gst_profile(
        db.pool(),
        &comp.id,
        Some("27AABCU9603R1ZN"),
        "27",
        "Regular",
        true,
    )
    .await
    .unwrap();

    (dir, db, comp.id)
}

async fn seed_m20_masters(
    pool: &sqlx::Pool<sqlx::Sqlite>,
    company_id: &str,
    valuation_method: &str,
) -> (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
) {
    let dr_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Debtors' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let cr_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sundry Creditors' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let sal_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Sales Accounts' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let pur_grp: String = sqlx::query_scalar(
        "SELECT id FROM account_groups WHERE company_id = ? AND name = 'Purchase Accounts' LIMIT 1",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await
    .unwrap();

    let cust_led = Uuid::new_v4().to_string();
    let supp_led = Uuid::new_v4().to_string();
    let sal_led = Uuid::new_v4().to_string();
    let pur_led = Uuid::new_v4().to_string();

    for (id, grp, name) in [
        (&cust_led, &dr_grp, "Retailer A"),
        (&supp_led, &cr_grp, "FMCG Supplier"),
        (&sal_led, &sal_grp, "Sales Account"),
        (&pur_led, &pur_grp, "Purchase Account"),
    ] {
        sqlx::query(
            "INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
             VALUES (?, ?, ?, ?, 0, 'standard')",
        )
        .bind(id)
        .bind(company_id)
        .bind(grp)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    }

    let unit_nos = Uuid::new_v4().to_string();
    let unit_box = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO units (id, company_id, formal_name, symbol, decimal_precision, unit_type)
         VALUES (?, ?, 'Numbers', 'Nos', 0, 'simple')",
    )
    .bind(&unit_nos)
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO units (id, company_id, formal_name, symbol, decimal_precision, unit_type)
         VALUES (?, ?, 'Box', 'Box', 0, 'simple')",
    )
    .bind(&unit_box)
    .bind(company_id)
    .execute(pool)
    .await
    .unwrap();

    let sgrp_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO stock_groups (id, company_id, name) VALUES (?, ?, 'Primary')")
        .bind(&sgrp_id)
        .bind(company_id)
        .execute(pool)
        .await
        .unwrap();

    let item_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stock_items (id, company_id, name, stock_group_id, base_unit_id, valuation_method)
         VALUES (?, ?, 'FMCG Soap', ?, ?, ?)",
    )
    .bind(&item_id)
    .bind(company_id)
    .bind(&sgrp_id)
    .bind(&unit_nos)
    .bind(valuation_method)
    .execute(pool)
    .await
    .unwrap();

    let godown_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO godowns (id, company_id, name) VALUES (?, ?, 'Main Godown')")
        .bind(&godown_id)
        .bind(company_id)
        .execute(pool)
        .await
        .unwrap();

    let sal_ser = Uuid::new_v4().to_string();
    let pur_ser = Uuid::new_v4().to_string();
    for (sid, vtype, scode, sname, pfx) in [
        (&sal_ser, "vt_sales", "SAL", "Sales Series", "SAL/"),
        (&pur_ser, "vt_pur", "PUR", "Purchase Series", "PUR/"),
    ] {
        sqlx::query(
            "INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_code, series_name, numbering_method, restart_period)
             VALUES (?, ?, ?, ?, ?, 'automatic', 'yearly')",
        )
        .bind(sid)
        .bind(company_id)
        .bind(vtype)
        .bind(scode)
        .bind(sname)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
             VALUES (?, ?, ?, ?, '2026-04-01', ?, '', 1, 4)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(company_id)
        .bind(vtype)
        .bind(sid)
        .bind(pfx)
        .execute(pool)
        .await
        .unwrap();
    }

    (
        cust_led, supp_led, sal_led, pur_led, unit_nos, unit_box, item_id, godown_id, sal_ser,
    )
}

#[tokio::test]
async fn test_m20_compound_unit_conversion_and_incompatible_rejection() {
    let (_dir, db, company_id) = setup_m20_db().await;
    let (_c, _s, _sal, _pur, unit_nos, unit_box, item_id, _gd, _ser) =
        seed_m20_masters(db.pool(), &company_id, "FIFO").await;

    let cmp_unit = create_compound_unit(
        db.pool(),
        &company_id,
        "Box of 12 Nos",
        "Box12",
        &unit_box,
        &unit_nos,
        "12",
    )
    .await
    .expect("create compound unit");

    let mut conn = db.pool().acquire().await.unwrap();
    let converted = convert_to_base_quantity_tx(
        &mut conn,
        &company_id,
        &item_id,
        Decimal::from(2),
        Some(&cmp_unit.id),
    )
    .await
    .unwrap();
    assert_eq!(converted, Decimal::from(24));

    let err = convert_to_base_quantity_tx(
        &mut conn,
        &company_id,
        &item_id,
        Decimal::from(2),
        Some("invalid_unit_id"),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("incompatible"));
}

#[tokio::test]
async fn test_m20_actual_vs_billed_qty_free_goods() {
    let (_dir, db, company_id) = setup_m20_db().await;
    let (cust_led, supp_led, sal_led, pur_led, unit_nos, _box, item_id, godown_id, sal_ser) =
        seed_m20_masters(db.pool(), &company_id, "FIFO").await;

    let pur_ser: String = sqlx::query_scalar(
        "SELECT id FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = 'vt_pur' LIMIT 1",
    )
    .bind(&company_id)
    .fetch_one(db.pool())
    .await
    .unwrap();

    // Purchase 20 Nos @ ₹60
    CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Purchase,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: pur_ser,
            date: "2026-05-01".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: supp_led,
            commercial_account_ledger_id: Some(pur_led),
            total_amount: Decimal::from_str("1200.00").unwrap(),
            document_reference: None,
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                description: "Stock in".to_string(),
                stock_item_id: Some(item_id.clone()),
                unit_id: Some(unit_nos.clone()),
                godown_id: Some(godown_id.clone()),
                quantity: Decimal::from(20),
                rate: Decimal::from(60),
                discount_amount: Decimal::ZERO,
                line_amount: Decimal::from_str("1200.00").unwrap(),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
                billed_quantity: None,
                entered_unit_id: None,
            }],
            tax_lines: vec![],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("27".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(false),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: None,
        },
    )
    .await
    .unwrap();

    // Sell Actual=11, Billed=10 @ ₹100 + 18% GST -> Invoice = 1180.00, Stock consumed = 11 (remaining = 9)
    let sal_res = CommercialPostingService::post_commercial_voucher(
        db.pool(),
        &company_id,
        CommercialPostingInput {
            company_id: company_id.clone(),
            document_type: CommercialDocumentType::Sales,
            source_document_id: Uuid::new_v4().to_string(),
            source_voucher_id: None,
            series_id: sal_ser,
            date: "2026-05-05".to_string(),
            due_date: None,
            party_id: Uuid::new_v4().to_string(),
            party_ledger_id: cust_led,
            commercial_account_ledger_id: Some(sal_led.clone()),
            total_amount: Decimal::from_str("1000.00").unwrap(),
            document_reference: None,
            narration: None,
            lines: vec![CommercialLinePosting {
                line_number: 1,
                description: "Buy 10 Get 1 Free".to_string(),
                stock_item_id: Some(item_id.clone()),
                unit_id: Some(unit_nos),
                godown_id: Some(godown_id),
                quantity: Decimal::from(11),
                rate: Decimal::from(100),
                discount_amount: Decimal::ZERO,
                line_amount: Decimal::from_str("1000.00").unwrap(),
                batch_number: None,
                mfg_date: None,
                expiry_date: None,
                billed_quantity: Some(Decimal::from(10)),
                entered_unit_id: None,
            }],
            tax_lines: vec![TaxPostingLine {
                tax_type: "IGST".to_string(),
                tax_ledger_id: sal_led,
                rate: Decimal::from(18),
                amount: Decimal::from_str("180.00").unwrap(),
            }],
            bill_allocations: vec![],
            place_of_supply_state_code: Some("29".to_string()),
            is_reverse_charge: Some(false),
            allow_tax_override: Some(true),
            original_voucher_id: None,
            return_reason: None,
            tds_section_code: None,
            supply_category: None,
            itc_eligibility: None,
            port_code: None,
            shipping_bill_no: None,
            shipping_bill_date: None,
            allow_post_cutoff_note: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(
        sal_res.voucher.voucher.total_amount.as_deref(),
        Some("1180")
    );
    let rem_qty: String = sqlx::query_scalar(
        "SELECT remaining_quantity FROM inventory_cost_layers WHERE company_id = ? AND stock_item_id = ?",
    )
    .bind(&company_id)
    .bind(&item_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rem_qty, "9");
}

#[tokio::test]
async fn test_m20_fifo_vs_lifo_vs_weighted_average_valuation() {
    let mut results = Vec::new();
    for method in ["FIFO", "LIFO", "WeightedAverage"] {
        let (_dir, db, company_id) = setup_m20_db().await;
        let (cust_led, supp_led, sal_led, pur_led, unit_nos, _box, item_id, godown_id, sal_ser) =
            seed_m20_masters(db.pool(), &company_id, method).await;

        let pur_ser: String = sqlx::query_scalar(
            "SELECT id FROM voucher_number_series WHERE company_id = ? AND voucher_type_id = 'vt_pur' LIMIT 1",
        )
        .bind(&company_id)
        .fetch_one(db.pool())
        .await
        .unwrap();

        // Batch 1: 10 @ 100, Batch 2: 10 @ 120
        for (dt, rate, total) in [
            ("2026-05-01", "100", "1000.00"),
            ("2026-05-02", "120", "1200.00"),
        ] {
            CommercialPostingService::post_commercial_voucher(
                db.pool(),
                &company_id,
                CommercialPostingInput {
                    company_id: company_id.clone(),
                    document_type: CommercialDocumentType::Purchase,
                    source_document_id: Uuid::new_v4().to_string(),
                    source_voucher_id: None,
                    series_id: pur_ser.clone(),
                    date: dt.to_string(),
                    due_date: None,
                    party_id: Uuid::new_v4().to_string(),
                    party_ledger_id: supp_led.clone(),
                    commercial_account_ledger_id: Some(pur_led.clone()),
                    total_amount: Decimal::from_str(total).unwrap(),
                    document_reference: None,
                    narration: None,
                    lines: vec![CommercialLinePosting {
                        line_number: 1,
                        description: "Batch".to_string(),
                        stock_item_id: Some(item_id.clone()),
                        unit_id: Some(unit_nos.clone()),
                        godown_id: Some(godown_id.clone()),
                        quantity: Decimal::from(10),
                        rate: Decimal::from_str(rate).unwrap(),
                        discount_amount: Decimal::ZERO,
                        line_amount: Decimal::from_str(total).unwrap(),
                        batch_number: None,
                        mfg_date: None,
                        expiry_date: None,
                        billed_quantity: None,
                        entered_unit_id: None,
                    }],
                    tax_lines: vec![],
                    bill_allocations: vec![],
                    place_of_supply_state_code: Some("27".to_string()),
                    is_reverse_charge: Some(false),
                    allow_tax_override: Some(false),
                    original_voucher_id: None,
                    return_reason: None,
                    tds_section_code: None,
                    supply_category: None,
                    itc_eligibility: None,
                    port_code: None,
                    shipping_bill_no: None,
                    shipping_bill_date: None,
                    allow_post_cutoff_note: None,
                },
            )
            .await
            .unwrap();
        }

        // Sell 10 units -> FIFO: 1000, LIFO: 1200, WeightedAverage: 1100
        let sal = CommercialPostingService::post_commercial_voucher(
            db.pool(),
            &company_id,
            CommercialPostingInput {
                company_id: company_id.clone(),
                document_type: CommercialDocumentType::Sales,
                source_document_id: Uuid::new_v4().to_string(),
                source_voucher_id: None,
                series_id: sal_ser,
                date: "2026-05-05".to_string(),
                due_date: None,
                party_id: Uuid::new_v4().to_string(),
                party_ledger_id: cust_led,
                commercial_account_ledger_id: Some(sal_led),
                total_amount: Decimal::from_str("1500.00").unwrap(),
                document_reference: None,
                narration: None,
                lines: vec![CommercialLinePosting {
                    line_number: 1,
                    description: "Sale".to_string(),
                    stock_item_id: Some(item_id),
                    unit_id: Some(unit_nos),
                    godown_id: Some(godown_id),
                    quantity: Decimal::from(10),
                    rate: Decimal::from(150),
                    discount_amount: Decimal::ZERO,
                    line_amount: Decimal::from_str("1500.00").unwrap(),
                    batch_number: None,
                    mfg_date: None,
                    expiry_date: None,
                    billed_quantity: None,
                    entered_unit_id: None,
                }],
                tax_lines: vec![],
                bill_allocations: vec![],
                place_of_supply_state_code: Some("27".to_string()),
                is_reverse_charge: Some(false),
                allow_tax_override: Some(false),
                original_voucher_id: None,
                return_reason: None,
                tds_section_code: None,
                supply_category: None,
                itc_eligibility: None,
                port_code: None,
                shipping_bill_no: None,
                shipping_bill_date: None,
                allow_post_cutoff_note: None,
            },
        )
        .await
        .unwrap();

        results.push((method, sal.cogs_amount.unwrap_or(Decimal::ZERO)));
    }

    assert_eq!(results[0], ("FIFO", Decimal::from(1000)));
    assert_eq!(results[1], ("LIFO", Decimal::from(1200)));
    assert_eq!(results[2], ("WeightedAverage", Decimal::from(1100)));
}

#[tokio::test]
async fn test_m20_price_level_slab_resolution() {
    let (_dir, db, company_id) = setup_m20_db().await;
    let (cust_led, _s, _sal, _pur, _nos, _box, item_id, _gd, _ser) =
        seed_m20_masters(db.pool(), &company_id, "FIFO").await;

    let wholesale = create_price_level(db.pool(), &company_id, "Wholesale")
        .await
        .unwrap();
    upsert_price_list_entry(
        db.pool(),
        &company_id,
        &wholesale.id,
        &item_id,
        "2026-04-01",
        "10",
        None,
        "85.00",
        "5.00",
    )
    .await
    .unwrap();

    sqlx::query("UPDATE ledgers SET price_level_id = ? WHERE id = ?")
        .bind(&wholesale.id)
        .bind(&cust_led)
        .execute(db.pool())
        .await
        .unwrap();

    let resolved = resolve_item_price(
        db.pool(),
        &company_id,
        &item_id,
        Some(&cust_led),
        None,
        "15",
        "2026-05-01",
    )
    .await
    .unwrap();
    assert_eq!(resolved.unit_rate, "85.00");
    assert_eq!(resolved.discount_pct, "5.00");
    assert_eq!(resolved.matched_price_level.as_deref(), Some("Wholesale"));
}
