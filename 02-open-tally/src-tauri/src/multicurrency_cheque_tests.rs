use crate::audit::AuditCertificationService;
use crate::billwise::{AllocationType, BillAllocationInput, BillWiseService};
use crate::currency::models::*;
use crate::currency::service::CurrencyService;
use crate::database::migrations::run_all_migrations;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use crate::pdf::cheque::{format_cts2010_date_chars, generate_cheque_pdf_bytes};
use crate::security::models::{ActionPermission, UserRole};
use crate::vouchers::model::{SaveVoucherDraftRequest, VoucherLineInput};
use crate::vouchers::service::draft::VoucherDraftService;
use crate::vouchers::service::posting::VoucherPostingService;
use rust_decimal::Decimal;
use std::str::FromStr;

async fn setup_test_db() -> (tempfile::TempDir, DatabaseManager, String, String) {
    let dir = tempfile::tempdir().expect("create tempdir for test db");
    let db = DatabaseManager::init(&dir.path().join("multicurrency_test.db"))
        .await
        .expect("init db");
    let pool = db.pool();

    run_all_migrations(pool)
        .await
        .expect("run all migrations up to milestone 16");

    let comp1_id = "cmp_mc_01".to_string();
    let comp2_id = "cmp_mc_02".to_string();

    sqlx::query(
        r#"
        INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from, currency_symbol, currency_formal_name, currency_decimal_places)
        VALUES (?, 'Global Exports Ltd', 'Global Exports', 'India', 'Maharashtra', '2026-04-01', '2026-04-01', '₹', 'INR', 2),
               (?, 'Local Trading Corp', 'Local Trading', 'India', 'Karnataka', '2026-04-01', '2026-04-01', '₹', 'INR', 2)
        "#,
    )
    .bind(&comp1_id)
    .bind(&comp2_id)
    .execute(pool)
    .await
    .expect("seed companies");

    // Standard groups & ledgers for company 1
    sqlx::query(
        r#"
        INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
        VALUES ('grp_ca_mc1', 'cmp_mc_01', 'Current Assets', NULL, 'Assets', 1),
               ('grp_bank_mc1', 'cmp_mc_01', 'Bank Accounts', 'grp_ca_mc1', 'Assets', 0),
               ('grp_debtors_mc1', 'cmp_mc_01', 'Sundry Debtors', 'grp_ca_mc1', 'Assets', 0),
               ('grp_cl_mc1', 'cmp_mc_01', 'Current Liabilities', NULL, 'Liabilities', 1),
               ('grp_creditors_mc1', 'cmp_mc_01', 'Sundry Creditors', 'grp_cl_mc1', 'Liabilities', 0),
               ('grp_income_mc1', 'cmp_mc_01', 'Sales Accounts', NULL, 'Income', 1),
               ('grp_ind_inc_mc1', 'cmp_mc_01', 'Indirect Incomes', NULL, 'Income', 1)
        "#,
    )
    .execute(pool)
    .await
    .expect("seed groups");

    sqlx::query(
        r#"
        INSERT INTO ledgers (id, company_id, group_id, name, is_reserved, ledger_classification)
        VALUES ('led_bank_mc1', 'cmp_mc_01', 'grp_bank_mc1', 'HDFC Bank', 0, 'standard'),
               ('led_debtor_us1', 'cmp_mc_01', 'grp_debtors_mc1', 'US Customer Inc', 0, 'standard'),
               ('led_creditor_eu1', 'cmp_mc_01', 'grp_creditors_mc1', 'Euro Supplier GmbH', 0, 'standard'),
               ('led_sales_mc1', 'cmp_mc_01', 'grp_income_mc1', 'Export Sales', 0, 'standard'),
               ('led_forex_mc1', 'cmp_mc_01', 'grp_ind_inc_mc1', 'Forex Gain / Loss', 0, 'standard')
        "#,
    )
    .execute(pool)
    .await
    .expect("seed ledgers");

    // Seed voucher series for company 1
    sqlx::query(
        r#"
        INSERT INTO voucher_number_series (id, company_id, voucher_type_id, series_name, series_code, numbering_method, restart_period)
        VALUES ('ser_sales_mc1', 'cmp_mc_01', 'vt_sales', 'Sales Series', 'SAL', 'automatic', 'yearly'),
               ('ser_receipt_mc1', 'cmp_mc_01', 'vt_receipt', 'Receipt Series', 'RCPT', 'automatic', 'yearly'),
               ('ser_payment_mc1', 'cmp_mc_01', 'vt_payment', 'Payment Series', 'PYMT', 'automatic', 'yearly'),
               ('ser_jnl_mc1', 'cmp_mc_01', 'vt_jnl', 'Journal Series', 'JNL', 'automatic', 'yearly')
        "#,
    )
    .execute(pool)
    .await
    .expect("seed series");

    sqlx::query(
        r#"
        INSERT INTO voucher_number_configs (id, company_id, voucher_type_id, series_id, applicable_from, prefix, suffix, start_number, width)
        VALUES ('cfg_sales_mc1', 'cmp_mc_01', 'vt_sales', 'ser_sales_mc1', '2026-04-01', 'SAL/', '', 1, 4),
               ('cfg_receipt_mc1', 'cmp_mc_01', 'vt_receipt', 'ser_receipt_mc1', '2026-04-01', 'RCPT/', '', 1, 4),
               ('cfg_payment_mc1', 'cmp_mc_01', 'vt_payment', 'ser_payment_mc1', '2026-04-01', 'PYMT/', '', 1, 4),
               ('cfg_jnl_mc1', 'cmp_mc_01', 'vt_jnl', 'ser_jnl_mc1', '2026-04-01', 'JNL/', '', 1, 4)
        "#,
    )
    .execute(pool)
    .await
    .expect("seed configs");

    (dir, db, comp1_id, comp2_id)
}

#[tokio::test]
async fn test_currency_and_exchange_rate_lookup_effective_date() {
    let (_dir, db, comp1, _comp2) = setup_test_db().await;
    let pool = db.pool();

    // 1. Enable Multi-Currency via F11
    let mut tx = pool.begin().await.unwrap();
    let seed_res = CurrencyService::seed_company_currency_defaults_tx(&mut tx, &comp1, "admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    assert!(seed_res.base_currency_seeded);
    assert_eq!(seed_res.currencies_seeded_count, 5); // USD, EUR, GBP, AED, SGD
    assert_eq!(seed_res.cheque_templates_seeded_count, 4); // HDFC, SBI, ICICI, Axis

    // 2. Verify Base Currency
    let currencies = CurrencyService::list_currencies(pool, &comp1)
        .await
        .unwrap();
    let inr = currencies
        .iter()
        .find(|c| c.code == "INR")
        .expect("INR must exist");
    assert!(inr.is_base);
    assert_eq!(inr.symbol, "₹");

    // 3. Set progressive daily exchange rates for USD
    // 2026-04-01 @ 83.00
    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "USD".to_string(),
            effective_date: "2026-04-01".to_string(),
            standard_rate: "83.00".to_string(),
            selling_rate: Some("83.20".to_string()),
            buying_rate: Some("82.80".to_string()),
        },
        "admin",
    )
    .await
    .unwrap();

    // 2026-04-15 @ 83.50
    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "USD".to_string(),
            effective_date: "2026-04-15".to_string(),
            standard_rate: "83.50".to_string(),
            selling_rate: Some("83.70".to_string()),
            buying_rate: Some("83.30".to_string()),
        },
        "admin",
    )
    .await
    .unwrap();

    // 2026-05-01 @ 84.00
    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "USD".to_string(),
            effective_date: "2026-05-01".to_string(),
            standard_rate: "84.00".to_string(),
            selling_rate: Some("84.25".to_string()),
            buying_rate: Some("83.75".to_string()),
        },
        "admin",
    )
    .await
    .unwrap();

    // 4. Test deterministic lookup as of dates
    // As of 2026-04-10 (before April 15) -> should resolve 83.00
    let rate_apr10 = CurrencyService::lookup_exchange_rate(pool, &comp1, "USD", "2026-04-10")
        .await
        .unwrap();
    assert_eq!(
        rate_apr10.standard_rate,
        Decimal::from_str("83.00").unwrap()
    );
    assert_eq!(rate_apr10.selling_rate, Decimal::from_str("83.20").unwrap());

    // As of 2026-04-20 (between April 15 and May 01) -> should resolve 83.50
    let rate_apr20 = CurrencyService::lookup_exchange_rate(pool, &comp1, "USD", "2026-04-20")
        .await
        .unwrap();
    assert_eq!(
        rate_apr20.standard_rate,
        Decimal::from_str("83.50").unwrap()
    );

    // As of 2026-05-15 (after May 01) -> should resolve 84.00
    let rate_may15 = CurrencyService::lookup_exchange_rate(pool, &comp1, "USD", "2026-05-15")
        .await
        .unwrap();
    assert_eq!(
        rate_may15.standard_rate,
        Decimal::from_str("84.00").unwrap()
    );

    // Base currency lookup always returns 1.00
    let rate_base = CurrencyService::lookup_exchange_rate(pool, &comp1, "INR", "2026-05-15")
        .await
        .unwrap();
    assert_eq!(rate_base.standard_rate, Decimal::ONE);
    assert!(rate_base.is_base);

    // Rate lookup before earliest configured date should return an error
    let err = CurrencyService::lookup_exchange_rate(pool, &comp1, "USD", "2026-03-31").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn test_foreign_invoice_conversion_and_voucher_currency_details() {
    let (_dir, db, comp1, _comp2) = setup_test_db().await;
    let pool = db.pool();

    let mut tx = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx, &comp1, "admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Set rate for USD on 2026-04-10 @ 83.50
    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "USD".to_string(),
            effective_date: "2026-04-10".to_string(),
            standard_rate: "83.50".to_string(),
            selling_rate: None,
            buying_rate: None,
        },
        "admin",
    )
    .await
    .unwrap();

    // Convert USD 1,000.00 @ 83.50
    let foreign_amount = Decimal::from_str("1000.00").unwrap();
    let converted_base =
        CurrencyService::convert_currency_amount(pool, &comp1, "USD", foreign_amount, "2026-04-15")
            .await
            .unwrap();

    assert_eq!(converted_base, Decimal::from_str("83500.00").unwrap());

    // Create & Post Sales Voucher (F8) in base currency (INR 83,500.00)
    let draft_res = VoucherDraftService::save_draft(
        pool,
        &comp1,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_sales".to_string(),
            series_id: "ser_sales_mc1".to_string(),
            date: "2026-04-15".to_string(),
            effective_date: None,
            reference_no: Some("INV-USD-101".to_string()),
            reference_date: None,
            narration: Some("Export of software services (USD 1,000 @ 83.50)".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_debtor_us1".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "83500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_sales_mc1".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "83500.00".to_string(),
                    narration: None,
                },
            ],
        },
    )
    .await
    .unwrap();

    let post_res =
        VoucherPostingService::post_voucher(pool, &comp1, &draft_res.voucher.id, Some("admin"))
            .await
            .unwrap();

    // Attach Bill allocation (NewRef INV-USD-101 for INR 83500.00)
    let debtor_line_id = post_res.lines[0].id.clone();
    let mut conn = pool.acquire().await.unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn,
        &comp1,
        &post_res.voucher.id,
        &debtor_line_id,
        "led_debtor_us1",
        "Dr",
        Decimal::from_str("83500.00").unwrap(),
        "2026-04-15",
        None,
        &[BillAllocationInput {
            allocation_type: AllocationType::NewRef,
            bill_name: "INV-USD-101".to_string(),
            amount: "83500.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: Some("2026-05-15".to_string()),
        }],
    )
    .await
    .unwrap();

    // Record foreign currency detail in voucher_currency_details
    sqlx::query(
        r#"
        INSERT INTO voucher_currency_details (id, company_id, voucher_id, voucher_line_id, currency_code, foreign_amount, exchange_rate, base_amount)
        VALUES ('vcd_01', ?, ?, ?, 'USD', '1000.00', '83.50', '83500.00')
        "#,
    )
    .bind(&comp1)
    .bind(&post_res.voucher.id)
    .bind(&debtor_line_id)
    .execute(pool)
    .await
    .unwrap();

    // Verify list_open_foreign_bills detects the open bill correctly
    let open_bills = CurrencyService::list_open_foreign_bills(pool, &comp1, Some("led_debtor_us1"))
        .await
        .unwrap();

    assert_eq!(open_bills.len(), 1);
    let b = &open_bills[0];
    assert_eq!(b.bill_name, "INV-USD-101");
    assert_eq!(b.currency_code, "USD");
    assert_eq!(b.invoice_rate, Decimal::from_str("83.50").unwrap());
    assert_eq!(b.original_foreign, Decimal::from_str("1000.00").unwrap());
    assert_eq!(b.outstanding_foreign, Decimal::from_str("1000.00").unwrap());
    assert_eq!(b.original_inr, Decimal::from_str("83500.00").unwrap());
    assert_eq!(b.outstanding_inr, Decimal::from_str("83500.00").unwrap());
}

#[tokio::test]
async fn test_realized_forex_gain_and_loss_settlement_equilibrium() {
    let (_dir, db, comp1, _comp2) = setup_test_db().await;
    let pool = db.pool();

    let mut tx = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx, &comp1, "admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // 1. Post initial invoice: USD 1,000 @ 83.50 = INR 83,500
    let draft_res = VoucherDraftService::save_draft(
        pool,
        &comp1,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_sales".to_string(),
            series_id: "ser_sales_mc1".to_string(),
            date: "2026-04-15".to_string(),
            effective_date: None,
            reference_no: Some("INV-USD-101".to_string()),
            reference_date: None,
            narration: Some("Export USD 1,000".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_debtor_us1".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "83500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_sales_mc1".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "83500.00".to_string(),
                    narration: None,
                },
            ],
        },
    )
    .await
    .unwrap();

    let post_res =
        VoucherPostingService::post_voucher(pool, &comp1, &draft_res.voucher.id, Some("admin"))
            .await
            .unwrap();

    let debtor_line_id = post_res.lines[0].id.clone();
    let mut conn = pool.acquire().await.unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn,
        &comp1,
        &post_res.voucher.id,
        &debtor_line_id,
        "led_debtor_us1",
        "Dr",
        Decimal::from_str("83500.00").unwrap(),
        "2026-04-15",
        None,
        &[BillAllocationInput {
            allocation_type: AllocationType::NewRef,
            bill_name: "INV-USD-101".to_string(),
            amount: "83500.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: Some("2026-05-15".to_string()),
        }],
    )
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO voucher_currency_details (id, company_id, voucher_id, voucher_line_id, currency_code, foreign_amount, exchange_rate, base_amount)
        VALUES ('vcd_fx_01', ?, ?, ?, 'USD', '1000.00', '83.50', '83500.00')
        "#,
    )
    .bind(&comp1)
    .bind(&post_res.voucher.id)
    .bind(&debtor_line_id)
    .execute(pool)
    .await
    .unwrap();

    // 2. Settle bill when USD rate increased to 84.20
    // Gain = 1000 * (84.20 - 83.50) = +700.00 INR
    let settlement_res = CurrencyService::settle_foreign_bill_with_forex(
        pool,
        &comp1,
        SettleForeignBillInput {
            bill_name: "INV-USD-101".to_string(),
            party_ledger_id: "led_debtor_us1".to_string(),
            foreign_settled_amount: "1000.00".to_string(),
            settlement_date: "2026-05-10".to_string(),
            settlement_exchange_rate: Some("84.20".to_string()),
            payment_mode: "Bank".to_string(),
            bank_or_cash_ledger_id: "led_bank_mc1".to_string(),
            narration: Some("Foreign collection for INV-USD-101 @ 84.20".to_string()),
            cheque_instrument_number: None,
        },
        "accountant",
    )
    .await
    .unwrap();

    assert_eq!(
        settlement_res.foreign_settled,
        Decimal::from_str("1000.00").unwrap()
    );
    assert_eq!(
        settlement_res.invoice_rate,
        Decimal::from_str("83.50").unwrap()
    );
    assert_eq!(
        settlement_res.settlement_rate,
        Decimal::from_str("84.20").unwrap()
    );
    assert_eq!(
        settlement_res.book_inr,
        Decimal::from_str("83500.00").unwrap()
    );
    assert_eq!(
        settlement_res.settled_inr,
        Decimal::from_str("84200.00").unwrap()
    );
    assert_eq!(
        settlement_res.forex_difference,
        Decimal::from_str("700.00").unwrap()
    );
    assert!(settlement_res.is_gain);
    assert!(settlement_res.forex_journal_voucher_id.is_some());

    // 3. Verify Trial Balance Equilibrium: Total Debits == Total Credits, Difference == 0
    let dr_amounts: Vec<(String,)> = sqlx::query_as(
        "SELECT vl.amount FROM voucher_lines vl JOIN vouchers v ON vl.voucher_id = v.id WHERE v.company_id = ? AND v.status = 'posted' AND vl.entry_type = 'Dr'",
    )
    .bind(&comp1)
    .fetch_all(pool)
    .await
    .unwrap();
    let total_debit: Decimal = dr_amounts
        .into_iter()
        .map(|(a,)| Decimal::from_str(&a).unwrap())
        .sum();

    let cr_amounts: Vec<(String,)> = sqlx::query_as(
        "SELECT vl.amount FROM voucher_lines vl JOIN vouchers v ON vl.voucher_id = v.id WHERE v.company_id = ? AND v.status = 'posted' AND vl.entry_type = 'Cr'",
    )
    .bind(&comp1)
    .fetch_all(pool)
    .await
    .unwrap();
    let total_credit: Decimal = cr_amounts
        .into_iter()
        .map(|(a,)| Decimal::from_str(&a).unwrap())
        .sum();

    assert_eq!(total_debit, total_credit);
    assert_eq!(total_debit - total_credit, Decimal::ZERO);

    // 4. Verify Forex Gain / Loss ledger balance is credited ₹700.00
    let fx_amounts: Vec<(String,)> = sqlx::query_as(
        "SELECT vl.amount FROM voucher_lines vl JOIN vouchers v ON vl.voucher_id = v.id WHERE v.company_id = ? AND v.status = 'posted' AND vl.ledger_id = 'led_forex_mc1' AND vl.entry_type = 'Cr'",
    )
    .bind(&comp1)
    .fetch_all(pool)
    .await
    .unwrap();
    let fx_credit: Decimal = fx_amounts
        .into_iter()
        .map(|(a,)| Decimal::from_str(&a).unwrap())
        .sum();
    assert_eq!(fx_credit, Decimal::from_str("700.00").unwrap());

    // 5. Verify the bill is now completely closed (outstanding_foreign == 0)
    let open_after = CurrencyService::list_open_foreign_bills(pool, &comp1, Some("led_debtor_us1"))
        .await
        .unwrap();
    assert!(open_after.is_empty());
}

#[tokio::test]
async fn test_unrealized_forex_revaluation_report() {
    let (_dir, db, comp1, _comp2) = setup_test_db().await;
    let pool = db.pool();

    let mut tx = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx, &comp1, "admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Set initial rates
    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "USD".to_string(),
            effective_date: "2026-04-01".to_string(),
            standard_rate: "82.00".to_string(),
            selling_rate: None,
            buying_rate: None,
        },
        "admin",
    )
    .await
    .unwrap();

    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "EUR".to_string(),
            effective_date: "2026-04-01".to_string(),
            standard_rate: "92.00".to_string(),
            selling_rate: None,
            buying_rate: None,
        },
        "admin",
    )
    .await
    .unwrap();

    // Create 2 open bills: USD 2,000 @ 82.00 = INR 164,000; EUR 1,000 @ 92.00 = INR 92,000
    // Post bill 1 (USD)
    let draft1 = VoucherDraftService::save_draft(
        pool,
        &comp1,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_sales".to_string(),
            series_id: "ser_sales_mc1".to_string(),
            date: "2026-04-05".to_string(),
            effective_date: None,
            reference_no: Some("INV-USD-201".to_string()),
            reference_date: None,
            narration: Some("USD export".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_debtor_us1".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "164000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_sales_mc1".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "164000.00".to_string(),
                    narration: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    let post1 =
        VoucherPostingService::post_voucher(pool, &comp1, &draft1.voucher.id, Some("admin"))
            .await
            .unwrap();
    let mut conn1 = pool.acquire().await.unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn1,
        &comp1,
        &post1.voucher.id,
        &post1.lines[0].id,
        "led_debtor_us1",
        "Dr",
        Decimal::from_str("164000.00").unwrap(),
        "2026-04-05",
        None,
        &[BillAllocationInput {
            allocation_type: AllocationType::NewRef,
            bill_name: "INV-USD-201".to_string(),
            amount: "164000.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: Some("2026-05-30".to_string()),
        }],
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO voucher_currency_details (id, company_id, voucher_id, voucher_line_id, currency_code, foreign_amount, exchange_rate, base_amount) VALUES ('vcd_unr_1', ?, ?, ?, 'USD', '2000.00', '82.00', '164000.00')",
    )
    .bind(&comp1)
    .bind(&post1.voucher.id)
    .bind(&post1.lines[0].id)
    .execute(pool)
    .await
    .unwrap();

    // Post bill 2 (EUR)
    let draft2 = VoucherDraftService::save_draft(
        pool,
        &comp1,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_sales".to_string(),
            series_id: "ser_sales_mc1".to_string(),
            date: "2026-04-10".to_string(),
            effective_date: None,
            reference_no: Some("INV-EUR-202".to_string()),
            reference_date: None,
            narration: Some("EUR export".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_debtor_us1".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "92000.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_sales_mc1".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "92000.00".to_string(),
                    narration: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    let post2 =
        VoucherPostingService::post_voucher(pool, &comp1, &draft2.voucher.id, Some("admin"))
            .await
            .unwrap();
    let mut conn2 = pool.acquire().await.unwrap();
    BillWiseService::validate_and_post_allocations(
        &mut conn2,
        &comp1,
        &post2.voucher.id,
        &post2.lines[0].id,
        "led_debtor_us1",
        "Dr",
        Decimal::from_str("92000.00").unwrap(),
        "2026-04-10",
        None,
        &[BillAllocationInput {
            allocation_type: AllocationType::NewRef,
            bill_name: "INV-EUR-202".to_string(),
            amount: "92000.00".to_string(),
            dr_cr: Some("Dr".to_string()),
            due_date: Some("2026-05-30".to_string()),
        }],
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO voucher_currency_details (id, company_id, voucher_id, voucher_line_id, currency_code, foreign_amount, exchange_rate, base_amount) VALUES ('vcd_unr_2', ?, ?, ?, 'EUR', '1000.00', '92.00', '92000.00')",
    )
    .bind(&comp1)
    .bind(&post2.voucher.id)
    .bind(&post2.lines[0].id)
    .execute(pool)
    .await
    .unwrap();

    // Now set period-end revaluation rates as of 2026-06-30:
    // USD goes up from 82.00 to 85.00 (+3.00/USD -> Unrealized Gain: 2000 * +3 = +6000)
    // EUR goes down from 92.00 to 89.00 (-3.00/EUR -> Unrealized Loss: 1000 * -3 = -3000)
    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "USD".to_string(),
            effective_date: "2026-06-30".to_string(),
            standard_rate: "85.00".to_string(),
            selling_rate: None,
            buying_rate: None,
        },
        "admin",
    )
    .await
    .unwrap();

    CurrencyService::set_exchange_rate(
        pool,
        &comp1,
        SetExchangeRateInput {
            currency_code: "EUR".to_string(),
            effective_date: "2026-06-30".to_string(),
            standard_rate: "89.00".to_string(),
            selling_rate: None,
            buying_rate: None,
        },
        "admin",
    )
    .await
    .unwrap();

    // Evaluate revaluation report
    let report = CurrencyService::evaluate_unrealized_forex_revaluation(pool, &comp1, "2026-06-30")
        .await
        .unwrap();

    assert_eq!(report.bills.len(), 2);
    assert_eq!(
        report.total_book_value_inr,
        Decimal::from_str("256000.00").unwrap()
    );
    assert_eq!(
        report.total_revalued_value_inr,
        Decimal::from_str("259000.00").unwrap()
    );
    assert_eq!(
        report.total_unrealized_gain,
        Decimal::from_str("6000.00").unwrap()
    );
    assert_eq!(
        report.total_unrealized_loss,
        Decimal::from_str("3000.00").unwrap()
    );
    assert_eq!(
        report.net_unrealized_gain_loss,
        Decimal::from_str("3000.00").unwrap()
    );
}

#[tokio::test]
async fn test_cts2010_cheque_pdf_generation() {
    let (_dir, db, comp1, _comp2) = setup_test_db().await;
    let pool = db.pool();

    let mut tx = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx, &comp1, "admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Test date character formatting
    let date_chars = format_cts2010_date_chars("2026-05-15").expect("format date chars");
    assert_eq!(date_chars, ['1', '5', '0', '5', '2', '0', '2', '6']);

    // Post an F5 Payment voucher with Cheque
    let draft = VoucherDraftService::save_draft(
        pool,
        &comp1,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_payment".to_string(),
            series_id: "ser_payment_mc1".to_string(),
            date: "2026-05-15".to_string(),
            effective_date: None,
            reference_no: Some("CHQ-889900".to_string()),
            reference_date: None,
            narration: Some("Payment towards machinery spare parts".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_creditor_eu1".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "83500.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_bank_mc1".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "83500.00".to_string(),
                    narration: None,
                },
            ],
        },
    )
    .await
    .unwrap();

    let post = VoucherPostingService::post_voucher(pool, &comp1, &draft.voucher.id, Some("admin"))
        .await
        .unwrap();

    // Attach bank allocation
    sqlx::query(
        r#"
        INSERT INTO bank_allocations (id, company_id, voucher_id, voucher_line_id, bank_ledger_id, transaction_type, instrument_number, instrument_date, favouring_name, amount, dr_cr)
        VALUES ('ba_chq_1', ?, ?, ?, 'led_bank_mc1', 'Cheque', '889900', '2026-05-15', 'Euro Supplier GmbH', '83500.00', 'Cr')
        "#,
    )
    .bind(&comp1)
    .bind(&post.voucher.id)
    .bind(&post.lines[1].id)
    .execute(pool)
    .await
    .unwrap();

    // Generate cheque PDF bytes for HDFC Bank template with Account Payee crossing
    let templates = CurrencyService::list_cheque_templates(pool, &comp1)
        .await
        .unwrap();
    let hdfc_template = templates
        .iter()
        .find(|t| t.bank_name.contains("HDFC"))
        .expect("HDFC template");

    let pdf_bytes = generate_cheque_pdf_bytes(
        pool,
        &comp1,
        &post.voucher.id,
        Some(&hdfc_template.id),
        Some(true),
    )
    .await
    .unwrap();

    // Verify valid %PDF-1.4 header and trailer
    assert!(pdf_bytes.starts_with(b"%PDF-1.4"));
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(pdf_str.contains("%%EOF"));
    assert!(pdf_str.contains("A/C PAYEE ONLY"));
    assert!(pdf_str.contains("Euro Supplier GmbH"));
    assert!(pdf_str.contains("Rupees Eighty Three Thousand Five Hundred Only"));
    assert!(pdf_str.contains("** 83,500.00/-"));
}

#[tokio::test]
async fn test_period_locked_and_rbac_enforcement() {
    let (_dir, db, comp1, _comp2) = setup_test_db().await;
    let pool = db.pool();

    let mut tx = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx, &comp1, "admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // 1. Lock period up to 2026-04-30
    let draft_v = VoucherDraftService::save_draft(
        pool,
        &comp1,
        SaveVoucherDraftRequest {
            voucher_id: None,
            voucher_type_id: "vt_jnl".to_string(),
            series_id: "ser_jnl_mc1".to_string(),
            date: "2026-04-30".to_string(),
            effective_date: None,
            reference_no: Some("LOCK-REF".to_string()),
            reference_date: None,
            narration: Some("Year end lock voucher".to_string()),
            lines: vec![
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_debtor_us1".to_string(),
                    entry_type: "Dr".to_string(),
                    amount: "10.00".to_string(),
                    narration: None,
                },
                VoucherLineInput {
                    id: None,
                    ledger_id: "led_sales_mc1".to_string(),
                    entry_type: "Cr".to_string(),
                    amount: "10.00".to_string(),
                    narration: None,
                },
            ],
        },
    )
    .await
    .unwrap();
    let post_v =
        VoucherPostingService::post_voucher(pool, &comp1, &draft_v.voucher.id, Some("admin"))
            .await
            .unwrap();

    sqlx::query(
        r#"
        INSERT INTO financial_year_closings (id, company_id, financial_year_label, period_start, period_end, next_fy_start, closing_voucher_id, net_profit_or_loss, is_locked, closed_by)
        VALUES ('fyc_test_lock', ?, 'FY 2026-27', '2026-04-01', '2026-04-30', '2026-05-01', ?, '0.00', 1, 'admin')
        "#,
    )
    .bind(&comp1)
    .bind(&post_v.voucher.id)
    .execute(pool)
    .await
    .unwrap();

    // Settle foreign bill attempt inside locked period must fail
    let err = CurrencyService::settle_foreign_bill_with_forex(
        pool,
        &comp1,
        SettleForeignBillInput {
            bill_name: "NON_EXISTENT".to_string(),
            party_ledger_id: "led_debtor_us1".to_string(),
            foreign_settled_amount: "100.00".to_string(),
            settlement_date: "2026-04-20".to_string(),
            settlement_exchange_rate: Some("84.00".to_string()),
            payment_mode: "Bank".to_string(),
            bank_or_cash_ledger_id: "led_bank_mc1".to_string(),
            narration: None,
            cheque_instrument_number: None,
        },
        "accountant",
    )
    .await;

    assert!(matches!(err, Err(OpenTallyError::PeriodLocked(_))));

    // 2. RBAC Permissions Matrix Verification
    assert!(UserRole::Admin.can_perform(ActionPermission::ManageMultiCurrency));
    assert!(UserRole::Accountant.can_perform(ActionPermission::ManageMultiCurrency));
    assert!(!UserRole::DataEntry.can_perform(ActionPermission::ManageMultiCurrency));
    assert!(!UserRole::Auditor.can_perform(ActionPermission::ManageMultiCurrency));

    assert!(UserRole::Admin.can_perform(ActionPermission::PrintCheque));
    assert!(UserRole::Accountant.can_perform(ActionPermission::PrintCheque));
    assert!(UserRole::DataEntry.can_perform(ActionPermission::PrintCheque));
    assert!(!UserRole::Auditor.can_perform(ActionPermission::PrintCheque));
}

#[tokio::test]
async fn test_sha256_audit_log_chain_integrity() {
    let (_dir, db, comp1, _comp2) = setup_test_db().await;
    let pool = db.pool();

    let mut tx = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx, &comp1, "admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Create a new currency and rate
    CurrencyService::create_currency(
        pool,
        &comp1,
        CreateCurrencyInput {
            code: "JPY".to_string(),
            symbol: "¥".to_string(),
            formal_name: "Japanese Yen".to_string(),
            decimal_places: Some(0),
            is_base: Some(false),
        },
        "admin",
    )
    .await
    .unwrap();

    // Verify Audit Log Hash Chain
    type AuditLogRow = (
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
    );
    let rows: Vec<AuditLogRow> = sqlx::query_as(
        r#"
        SELECT id, company_id, entity_type, entity_id, action, timestamp, new_values, prev_hash, entry_hash
        FROM audit_log
        WHERE company_id = ?
        ORDER BY timestamp ASC, id ASC
        "#,
    )
    .bind(&comp1)
    .fetch_all(pool)
    .await
    .unwrap();

    assert!(
        !rows.is_empty(),
        "Audit log must contain entries for seeded and created currencies"
    );

    let mut expected_prev = "GENESIS".to_string();
    for row in rows {
        let (id, cid, etype, eid, act, ts, nvals, prev_h, ent_h) = row;
        assert_eq!(
            prev_h, expected_prev,
            "Chain link broken at audit id {}",
            id
        );
        let computed = AuditCertificationService::compute_entry_hash(
            &expected_prev,
            &id,
            &cid,
            &etype,
            &eid,
            &act,
            &ts,
            nvals.as_deref(),
        );
        assert_eq!(ent_h, computed, "Hash mismatch at audit id {}", id);
        expected_prev = ent_h;
    }
}

#[tokio::test]
async fn test_multi_company_isolation() {
    let (_dir, db, comp1, comp2) = setup_test_db().await;
    let pool = db.pool();

    let mut tx1 = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx1, &comp1, "admin")
        .await
        .unwrap();
    tx1.commit().await.unwrap();

    // Company 2 needs Indirect Incomes group before currency defaults can seed Forex ledger
    sqlx::query(
        r#"
        INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary)
        VALUES ('grp_ind_inc_mc2', 'cmp_mc_02', 'Indirect Incomes', NULL, 'Income', 1)
        "#,
    )
    .execute(pool)
    .await
    .expect("seed company 2 groups");

    let mut tx2 = pool.begin().await.unwrap();
    CurrencyService::seed_company_currency_defaults_tx(&mut tx2, &comp2, "admin")
        .await
        .unwrap();
    tx2.commit().await.unwrap();

    // Create custom currency in Company 1
    CurrencyService::create_currency(
        pool,
        &comp1,
        CreateCurrencyInput {
            code: "KWD".to_string(),
            symbol: "KD".to_string(),
            formal_name: "Kuwaiti Dinar".to_string(),
            decimal_places: Some(3),
            is_base: Some(false),
        },
        "admin",
    )
    .await
    .unwrap();

    // Verify Company 1 has KWD
    let c1_list = CurrencyService::list_currencies(pool, &comp1)
        .await
        .unwrap();
    assert!(c1_list.iter().any(|c| c.code == "KWD"));

    // Verify Company 2 DOES NOT have KWD
    let c2_list = CurrencyService::list_currencies(pool, &comp2)
        .await
        .unwrap();
    assert!(!c2_list.iter().any(|c| c.code == "KWD"));

    // Attempt to lookup KWD in Company 2 must return error
    let err = CurrencyService::lookup_exchange_rate(pool, &comp2, "KWD", "2026-05-01").await;
    assert!(err.is_err());
}
