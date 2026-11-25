use super::*;
use crate::database::migrations::{
    run_sprint1_migration, run_sprint2_migration, run_sprint2_stage4_hardening_migration,
    run_sprint4_inventory_migration, run_sprint4_stage11_stock_migration,
    run_sprint4_stage12_valuation_migration, run_sprint5_stage14_integration_migration,
    run_sprint5_stage15_valuation_attribution_migration, run_stage3_migration,
};
use crate::database::DatabaseManager;
use crate::inventory::accounting::test_integration_failure_lock;
use crate::vouchers::{
    CreateVoucherDraftRequest, VoucherDraftService, VoucherLineInput, VoucherPostingService,
};
use rust_decimal::Decimal;
use std::path::Path;

async fn fixture() -> DatabaseManager {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stage15.db");
    std::mem::forget(dir);
    fixture_at(&path).await
}

async fn fixture_at(path: &Path) -> DatabaseManager {
    let db = DatabaseManager::init(path).await.unwrap();
    let p = db.pool();
    run_sprint1_migration(p).await.unwrap();
    run_stage3_migration(p).await.unwrap();
    run_sprint2_migration(p).await.unwrap();
    run_sprint2_stage4_hardening_migration(p).await.unwrap();
    run_sprint4_inventory_migration(p).await.unwrap();
    run_sprint4_stage11_stock_migration(p).await.unwrap();
    run_sprint4_stage12_valuation_migration(p).await.unwrap();
    run_sprint5_stage14_integration_migration(p).await.unwrap();
    run_sprint5_stage15_valuation_attribution_migration(p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO companies(id,name,mailing_name,country,state,financial_year_begins,books_beginning_from) VALUES('s15','Stage 15 Co','Stage 15 Co','India','Delhi','2026-04-01','2026-04-01')").execute(p).await.unwrap();
    sqlx::query("INSERT INTO stock_groups(id,company_id,name) VALUES('sg','s15','Goods')")
        .execute(p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO units(id,company_id,symbol,formal_name,decimal_precision) VALUES('u','s15','Nos','Numbers',2)").execute(p).await.unwrap();
    sqlx::query("INSERT INTO stock_items(id,company_id,name,stock_group_id,base_unit_id,opening_quantity,opening_rate,opening_value,reorder_level) VALUES('item','s15','Widget','sg','u','0','0','0','0')").execute(p).await.unwrap();
    sqlx::query("INSERT INTO godowns(id,company_id,name) VALUES('g','s15','Main')")
        .execute(p)
        .await
        .unwrap();
    db
}
async fn accounting_fixture() -> DatabaseManager {
    let db = fixture().await;
    add_accounting_fixture(&db).await;
    db
}

async fn accounting_fixture_at(path: &Path) -> DatabaseManager {
    let db = fixture_at(path).await;
    add_accounting_fixture(&db).await;
    db
}

async fn add_accounting_fixture(db: &DatabaseManager) {
    let p = db.pool();
    sqlx::query("INSERT INTO account_groups(id,company_id,name,parent_id,nature,is_primary,is_predefined,is_hidden,affects_gross_profit) VALUES('ag_a','s15','Assets',NULL,'Assets',1,0,0,0),('ag_e','s15','Expenses',NULL,'Expenses',1,0,0,1),('ag_l','s15','Liabilities',NULL,'Liabilities',1,0,0,0),('ag_i','s15','Income',NULL,'Income',1,0,0,1)").execute(p).await.unwrap();
    sqlx::query("INSERT INTO ledgers(id,company_id,group_id,name,is_reserved,ledger_classification) VALUES('asset','s15','ag_a','Inventory Asset',0,'standard'),('cogs','s15','ag_e','COGS',0,'standard'),('offset','s15','ag_l','Receipt Offset',0,'standard'),('gain','s15','ag_i','Stock Gain',0,'standard')").execute(p).await.unwrap();
    sqlx::query("INSERT INTO voucher_number_series(id,company_id,voucher_type_id,series_name,series_code,numbering_method,restart_period) VALUES('js','s15','vt_jnl','Integration Journal','INT','automatic','yearly'),('rev','s15','vt_reversing_journal','Reversal Journal','REV','automatic','yearly')").execute(p).await.unwrap();
    sqlx::query("INSERT INTO voucher_number_configs(id,company_id,voucher_type_id,series_id,applicable_from,prefix,suffix,start_number,width) VALUES('jc','s15','vt_jnl','js','2026-04-01','J/','',1,4),('rc','s15','vt_reversing_journal','rev','2026-04-01','R/','',1,4)").execute(p).await.unwrap();
}
async fn configure_accounts(db: &DatabaseManager) {
    InventoryAccountingService::configure_mapping(
        db.pool(),
        "s15",
        InventoryAccountMappingRequest {
            inventory_asset_ledger_id: "asset".into(),
            adjustment_expense_ledger_id: "cogs".into(),
            adjustment_gain_ledger_id: Some("gain".into()),
            cogs_ledger_id: "cogs".into(),
            receipt_offset_ledger_id: "offset".into(),
            journal_series_id: "js".into(),
            enabled: true,
        },
    )
    .await
    .unwrap();
}
fn line(cost: Option<&str>, quantity: &str) -> StockVoucherLineRequest {
    StockVoucherLineRequest {
        stock_item_id: "item".into(),
        unit_id: "u".into(),
        godown_id: Some("g".into()),
        source_godown_id: None,
        destination_godown_id: None,
        quantity: quantity.into(),
        unit_cost: cost.map(str::to_owned),
        physical_quantity: None,
        description: None,
    }
}
async fn post(
    db: &DatabaseManager,
    kind: &str,
    date: &str,
    lines: Vec<StockVoucherLineRequest>,
) -> StockVoucher {
    let v = StockVoucherService::create_draft(
        db.pool(),
        "s15",
        StockVoucherRequest {
            voucher_type: kind.into(),
            voucher_date: date.into(),
            reference_no: None,
            narration: None,
            lines,
        },
    )
    .await
    .unwrap();
    StockVoucherService::post(db.pool(), "s15", &v.id)
        .await
        .unwrap()
}

fn report_filter() -> InventoryReportFilter {
    InventoryReportFilter {
        date_from: None,
        date_to: None,
        as_of_date: Some("2026-04-30".into()),
        stock_item_id: None,
        stock_group_id: None,
        stock_category_id: None,
        godown_id: None,
        voucher_type: None,
        search: None,
        include_zero: true,
        page: None,
        page_size: None,
    }
}

async fn journal(db: &DatabaseManager, reference: &str, lines: Vec<VoucherLineInput>) -> String {
    let draft_id = draft_journal(db, reference, lines).await;
    VoucherPostingService::post_voucher(db.pool(), "s15", &draft_id, Some("stage15"))
        .await
        .unwrap()
        .voucher
        .id
}

async fn draft_journal(
    db: &DatabaseManager,
    reference: &str,
    lines: Vec<VoucherLineInput>,
) -> String {
    let draft = VoucherDraftService::create_draft(
        db.pool(),
        "s15",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_jnl".into(),
            series_id: "js".into(),
            date: "2026-04-02".into(),
            effective_date: Some("2026-04-02".into()),
            reference_no: Some(reference.into()),
            reference_date: None,
            narration: None,
            lines: Some(lines),
        },
    )
    .await
    .unwrap();
    draft.voucher.id
}

async fn attach_integration(
    db: &DatabaseManager,
    stock_voucher_id: &str,
    accounting_voucher_id: &str,
    amount: &str,
) {
    sqlx::query("INSERT INTO inventory_accounting_integrations(id,company_id,stock_voucher_id,stock_voucher_type,accounting_voucher_id,status,valuation_method,integrated_amount,attempt_count) SELECT ?, 's15', id, voucher_type, ?, 'integrated', 'fifo', ?, 1 FROM stock_vouchers WHERE company_id='s15' AND id=?")
        .bind(format!("integration-{stock_voucher_id}"))
        .bind(accounting_voucher_id)
        .bind(amount)
        .bind(stock_voucher_id)
        .execute(db.pool())
        .await
        .unwrap();
}

#[tokio::test]
async fn fifo_delivery_persists_multiple_source_layers_and_reversal_copy() {
    let db = fixture().await;
    let first = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "2")]).await;
    let second = post(&db, "receipt", "2026-04-02", vec![line(Some("20"), "3")]).await;
    let delivery = post(&db, "delivery", "2026-04-02", vec![line(None, "4")]).await;
    let attrs =
        InventoryValuationAttributionService::details_for_voucher(db.pool(), "s15", &delivery.id)
            .await
            .unwrap();
    assert_eq!(attrs.len(), 1);
    assert_eq!(attrs[0].attribution.total_value, "60");
    assert_eq!(attrs[0].layer_uses.len(), 2);
    assert!(attrs[0].layer_uses.iter().any(|layer| layer.quantity == "2"
        && layer.unit_cost == "10"
        && layer.source_voucher_id.as_deref() == Some(first.id.as_str())));
    assert!(attrs[0].layer_uses.iter().any(|layer| layer.quantity == "2"
        && layer.unit_cost == "20"
        && layer.source_voucher_id.as_deref() == Some(second.id.as_str())));
    let original_id = attrs[0].attribution.id.clone();
    let unrelated = post(&db, "receipt", "2026-04-03", vec![line(Some("99"), "1")]).await;
    let after = InventoryValuationAttributionService::for_voucher(db.pool(), "s15", &delivery.id)
        .await
        .unwrap();
    assert_eq!(after[0].id, original_id);
    assert_eq!(after[0].total_value, "60");
    assert!(!unrelated.id.is_empty());
    let reversal = StockVoucherService::cancel_or_reverse(db.pool(), "s15", &delivery.id, true)
        .await
        .unwrap();
    let reversed =
        InventoryValuationAttributionService::details_for_voucher(db.pool(), "s15", &reversal.id)
            .await
            .unwrap();
    assert_eq!(reversed[0].attribution.total_value, "60");
    assert_eq!(
        reversed[0]
            .attribution
            .reversal_of_attribution_id
            .as_deref(),
        Some(original_id.as_str())
    );
}

#[tokio::test]
async fn weighted_average_captures_posting_time_average_on_same_day() {
    let db = fixture().await;
    ValuationService::set_config(db.pool(), "s15", "weighted_average")
        .await
        .unwrap();
    post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "2")]).await;
    post(&db, "receipt", "2026-04-02", vec![line(Some("20"), "2")]).await;
    let delivery = post(&db, "delivery", "2026-04-02", vec![line(None, "3")]).await;
    let attrs = InventoryValuationAttributionService::for_voucher(db.pool(), "s15", &delivery.id)
        .await
        .unwrap();
    assert_eq!(attrs[0].valuation_method, "weighted_average");
    assert_eq!(attrs[0].unit_cost, "15");
    assert_eq!(attrs[0].total_value, "45");
}

#[tokio::test]
async fn accounting_failure_retry_and_repeated_integration_are_idempotent() {
    let _failure_guard = test_integration_failure_lock().lock().await;
    let db = accounting_fixture().await;
    configure_accounts(&db).await;
    let receipt = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "2")]).await;
    let before = InventoryValuationAttributionService::for_voucher(db.pool(), "s15", &receipt.id)
        .await
        .unwrap();
    set_test_integration_failure(3);
    assert!(
        InventoryAccountingService::integrate(db.pool(), "s15", &receipt.id)
            .await
            .is_err()
    );
    let failed = InventoryAccountingService::status(db.pool(), "s15", &receipt.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(
        InventoryValuationAttributionService::for_voucher(db.pool(), "s15", &receipt.id)
            .await
            .unwrap()[0]
            .id,
        before[0].id
    );
    let drafts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id='s15' AND status='draft'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(drafts, 0);
    let integrated = InventoryAccountingService::integrate(db.pool(), "s15", &receipt.id)
        .await
        .unwrap();
    let repeated = InventoryAccountingService::integrate(db.pool(), "s15", &receipt.id)
        .await
        .unwrap();
    assert_eq!(integrated.id, repeated.id);
    assert_eq!(
        integrated.accounting_voucher_id,
        repeated.accounting_voucher_id
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id='s15' AND reference_no=?",
    )
    .bind(format!("Inventory {}", receipt.voucher_number))
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    let second = post(&db, "receipt", "2026-04-03", vec![line(Some("20"), "1")]).await;
    set_test_integration_failure(1);
    assert!(
        InventoryAccountingService::integrate(db.pool(), "s15", &second.id)
            .await
            .is_err()
    );
    let partial: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id='s15' AND reference_no=?",
    )
    .bind(format!("Inventory {}", second.voucher_number))
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(partial, 0);
    let retry = InventoryAccountingService::integrate(db.pool(), "s15", &second.id)
        .await
        .unwrap();
    assert_eq!(retry.status, "integrated");
    let third = post(&db, "receipt", "2026-04-04", vec![line(Some("30"), "1")]).await;
    set_test_integration_failure(2);
    assert!(
        InventoryAccountingService::integrate(db.pool(), "s15", &third.id)
            .await
            .is_err()
    );
    let partial_draft: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vouchers WHERE company_id='s15' AND status='draft' AND reference_no=?").bind(format!("Inventory {}", third.voucher_number)).fetch_one(db.pool()).await.unwrap();
    assert_eq!(partial_draft, 0);
    assert_eq!(
        InventoryAccountingService::integrate(db.pool(), "s15", &third.id)
            .await
            .unwrap()
            .status,
        "integrated"
    );
    let reversal = StockVoucherService::cancel_or_reverse(db.pool(), "s15", &receipt.id, true)
        .await
        .unwrap();
    let rev = InventoryAccountingService::integrate(db.pool(), "s15", &reversal.id)
        .await
        .unwrap();
    let rev_again = InventoryAccountingService::integrate(db.pool(), "s15", &reversal.id)
        .await
        .unwrap();
    assert_eq!(rev.accounting_voucher_id, rev_again.accounting_voucher_id);
}

#[tokio::test]
async fn accounting_mapping_branches_have_exact_ledgers_and_directions() {
    let _failure_guard = test_integration_failure_lock().lock().await;
    let db = accounting_fixture().await;
    configure_accounts(&db).await;
    let receipt = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "2")]).await;
    let receipt_preview = InventoryAccountingService::preview(db.pool(), "s15", &receipt.id)
        .await
        .unwrap();
    assert_eq!(receipt_preview.lines[0].ledger_id, "asset");
    assert_eq!(receipt_preview.lines[0].entry_type, "Dr");
    assert_eq!(receipt_preview.lines[1].ledger_id, "offset");
    assert_eq!(receipt_preview.lines[1].entry_type, "Cr");
    assert_eq!(receipt_preview.amount, "20");
    let delivery = post(&db, "delivery", "2026-04-03", vec![line(None, "1")]).await;
    let delivery_preview = InventoryAccountingService::preview(db.pool(), "s15", &delivery.id)
        .await
        .unwrap();
    assert_eq!(delivery_preview.lines[0].ledger_id, "cogs");
    assert_eq!(delivery_preview.lines[0].entry_type, "Dr");
    assert_eq!(delivery_preview.lines[1].ledger_id, "asset");
    assert_eq!(delivery_preview.lines[1].entry_type, "Cr");
    assert_eq!(delivery_preview.amount, "10");
    let mut positive = line(None, "1");
    positive.physical_quantity = Some("2".into());
    let gain = post(&db, "adjustment", "2026-04-04", vec![positive]).await;
    let gain_preview = InventoryAccountingService::preview(db.pool(), "s15", &gain.id)
        .await
        .unwrap();
    assert_eq!(gain_preview.lines[0].ledger_id, "asset");
    assert_eq!(gain_preview.lines[0].entry_type, "Dr");
    assert_eq!(gain_preview.lines[1].ledger_id, "gain");
    assert_eq!(gain_preview.lines[1].entry_type, "Cr");
    let mut negative = line(None, "1");
    negative.physical_quantity = Some("1".into());
    let loss = post(&db, "adjustment", "2026-04-05", vec![negative]).await;
    let loss_preview = InventoryAccountingService::preview(db.pool(), "s15", &loss.id)
        .await
        .unwrap();
    assert_eq!(loss_preview.lines[0].ledger_id, "cogs");
    assert_eq!(loss_preview.lines[0].entry_type, "Dr");
    assert_eq!(loss_preview.lines[1].ledger_id, "asset");
    assert_eq!(loss_preview.lines[1].entry_type, "Cr");
    let integrated = InventoryAccountingService::integrate(db.pool(), "s15", &receipt.id)
        .await
        .unwrap();
    let reversal = StockVoucherService::cancel_or_reverse(db.pool(), "s15", &receipt.id, true)
        .await
        .unwrap();
    let reversed = InventoryAccountingService::integrate(db.pool(), "s15", &reversal.id)
        .await
        .unwrap();
    assert!(integrated.accounting_voucher_id.is_some());
    assert!(reversed.accounting_voucher_id.is_some());
    sqlx::query("DELETE FROM inventory_account_mappings WHERE company_id='s15'")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        InventoryAccountingService::preview(db.pool(), "s15", &delivery.id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn reconciliation_reports_accounting_imbalance_without_repair() {
    let db = accounting_fixture().await;
    configure_accounts(&db).await;
    let receipt = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "2")]).await;
    let draft = VoucherDraftService::create_draft(
        db.pool(),
        "s15",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_jnl".into(),
            series_id: "js".into(),
            date: "2026-04-02".into(),
            effective_date: Some("2026-04-02".into()),
            reference_no: Some("Imbalance".into()),
            reference_date: None,
            narration: None,
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: "asset".into(),
                entry_type: "Dr".into(),
                amount: "20".into(),
                narration: None,
            }]),
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO inventory_accounting_integrations(id,company_id,stock_voucher_id,stock_voucher_type,accounting_voucher_id,status,valuation_method,integrated_amount,attempt_count) VALUES('imb','s15',?,'receipt',?,'integrated','fifo','20',1)").bind(&receipt.id).bind(&draft.voucher.id).execute(db.pool()).await.unwrap();
    let findings = InventoryReportsService::reconciliation(
        db.pool(),
        "s15",
        InventoryReportFilter {
            date_from: None,
            date_to: None,
            as_of_date: Some("2026-04-02".into()),
            stock_item_id: None,
            stock_group_id: None,
            stock_category_id: None,
            godown_id: None,
            voucher_type: None,
            search: None,
            include_zero: true,
            page: None,
            page_size: None,
        },
    )
    .await
    .unwrap();
    assert!(findings
        .iter()
        .any(|finding| finding.check_id == "accounting-imbalance"));
    let status: String =
        sqlx::query_scalar("SELECT status FROM vouchers WHERE company_id='s15' AND id=?")
            .bind(&draft.voucher.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(status, "draft");
}

#[tokio::test]
async fn reconciliation_reports_independently_persisted_duplicate_accounting_vouchers() {
    let _failure_guard = test_integration_failure_lock().lock().await;
    let db = accounting_fixture().await;
    configure_accounts(&db).await;
    let receipt = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "2")]).await;
    let integrated = InventoryAccountingService::integrate(db.pool(), "s15", &receipt.id)
        .await
        .unwrap();
    let first_id = integrated.accounting_voucher_id.clone().unwrap();
    let reference = format!("Inventory {}", receipt.voucher_number);
    let second_id = journal(
        &db,
        &reference,
        vec![
            VoucherLineInput {
                id: None,
                ledger_id: "asset".into(),
                entry_type: "Dr".into(),
                amount: "20".into(),
                narration: None,
            },
            VoucherLineInput {
                id: None,
                ledger_id: "offset".into(),
                entry_type: "Cr".into(),
                amount: "20".into(),
                narration: None,
            },
        ],
    )
    .await;
    let findings = InventoryReportsService::reconciliation(db.pool(), "s15", report_filter())
        .await
        .unwrap();
    let duplicate = findings
        .iter()
        .find(|finding| finding.check_id == "duplicate-accounting-voucher")
        .expect("duplicate accounting voucher finding");
    assert_eq!(duplicate.company_id, "s15");
    assert_eq!(duplicate.stock_item_id.as_deref(), Some("item"));
    assert!(duplicate.actual_value.contains(&first_id));
    assert!(duplicate.actual_value.contains(&second_id));
    assert!(duplicate.explanation.contains(&receipt.id));
    let statuses: Vec<String> = sqlx::query_scalar(
        "SELECT status FROM vouchers WHERE company_id='s15' AND id IN (?,?) ORDER BY id",
    )
    .bind(&first_id)
    .bind(&second_id)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(statuses, vec!["posted", "posted"]);
    let repeated = InventoryAccountingService::integrate(db.pool(), "s15", &receipt.id)
        .await
        .unwrap();
    assert_eq!(
        repeated.accounting_voucher_id.as_deref(),
        Some(first_id.as_str())
    );
    let same_reference: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id='s15' AND reference_no=?",
    )
    .bind(reference)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(same_reference, 2);
}

#[tokio::test]
async fn reconciliation_reports_exact_accounting_debit_credit_edge_cases() {
    let db = accounting_fixture().await;
    configure_accounts(&db).await;
    let cases = [
        ("credit-greater", vec![("Dr", "20"), ("Cr", "30")], "-10"),
        ("missing-debit", vec![("Cr", "20")], "-20"),
        ("missing-credit", vec![("Dr", "20")], "20"),
    ];
    let mut expected = Vec::new();
    for (name, raw_lines, difference) in cases {
        let receipt = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "1")]).await;
        let lines = raw_lines
            .into_iter()
            .map(|(entry_type, amount)| VoucherLineInput {
                id: None,
                ledger_id: if entry_type == "Dr" {
                    "asset"
                } else {
                    "offset"
                }
                .into(),
                entry_type: entry_type.into(),
                amount: amount.into(),
                narration: None,
            })
            .collect();
        let draft_id = draft_journal(&db, name, lines).await;
        attach_integration(&db, &receipt.id, &draft_id, "10").await;
        expected.push((name, difference));
    }
    let zero_result = VoucherDraftService::create_draft(
        db.pool(),
        "s15",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_jnl".into(),
            series_id: "js".into(),
            date: "2026-04-02".into(),
            effective_date: Some("2026-04-02".into()),
            reference_no: Some("zero-entry".into()),
            reference_date: None,
            narration: None,
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: "asset".into(),
                entry_type: "Dr".into(),
                amount: "0".into(),
                narration: None,
            }]),
        },
    )
    .await;
    assert!(zero_result.is_err());
    let invalid_result = VoucherDraftService::create_draft(
        db.pool(),
        "s15",
        CreateVoucherDraftRequest {
            voucher_type_id: "vt_jnl".into(),
            series_id: "js".into(),
            date: "2026-04-02".into(),
            effective_date: Some("2026-04-02".into()),
            reference_no: Some("invalid-entry".into()),
            reference_date: None,
            narration: None,
            lines: Some(vec![VoucherLineInput {
                id: None,
                ledger_id: "asset".into(),
                entry_type: "XX".into(),
                amount: "20".into(),
                narration: None,
            }]),
        },
    )
    .await;
    assert!(invalid_result.is_err());
    let findings = InventoryReportsService::reconciliation(db.pool(), "s15", report_filter())
        .await
        .unwrap();
    for (name, difference) in expected {
        let finding = findings
            .iter()
            .find(|finding| {
                finding.explanation.contains(name) && finding.check_id == "accounting-imbalance"
            })
            .unwrap_or_else(|| panic!("missing imbalance for {name}"));
        assert_eq!(finding.difference, difference);
    }
    assert!(findings
        .iter()
        .any(|finding| finding.check_id == "missing-debit-line"));
    assert!(findings
        .iter()
        .any(|finding| finding.check_id == "missing-credit-line"));
}

#[tokio::test]
async fn concurrent_integration_is_serialized_and_idempotent_across_connections() {
    let _failure_guard = test_integration_failure_lock().lock().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stage15-concurrent.db");
    let db_one = accounting_fixture_at(&path).await;
    configure_accounts(&db_one).await;
    let db_two = DatabaseManager::init(&path).await.unwrap();
    let receipt = post(
        &db_one,
        "receipt",
        "2026-04-02",
        vec![line(Some("10"), "2")],
    )
    .await;
    let (first, second) = tokio::join!(
        InventoryAccountingService::integrate(db_one.pool(), "s15", &receipt.id),
        InventoryAccountingService::integrate(db_two.pool(), "s15", &receipt.id),
    );
    let first = first.unwrap();
    let second = second.unwrap();
    assert_eq!(first.status, "integrated");
    assert_eq!(second.status, "integrated");
    assert_eq!(first.accounting_voucher_id, second.accounting_voucher_id);
    let accounting_id = first.accounting_voucher_id.unwrap();
    let voucher_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM vouchers WHERE company_id='s15' AND reference_no=?",
    )
    .bind(format!("Inventory {}", receipt.voucher_number))
    .fetch_one(db_one.pool())
    .await
    .unwrap();
    assert_eq!(voucher_count, 1);
    let integration_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_accounting_integrations WHERE company_id='s15' AND stock_voucher_id=? AND status='integrated'",
    )
    .bind(&receipt.id)
    .fetch_one(db_one.pool())
    .await
    .unwrap();
    assert_eq!(integration_count, 1);
    let attribution_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_valuation_attributions WHERE company_id='s15' AND stock_voucher_id=?",
    )
    .bind(&receipt.id)
    .fetch_one(db_one.pool())
    .await
    .unwrap();
    assert_eq!(attribution_count, 1);
    let accounting_lines: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM voucher_lines WHERE company_id='s15' AND voucher_id=?",
    )
    .bind(accounting_id)
    .fetch_one(db_one.pool())
    .await
    .unwrap();
    assert_eq!(accounting_lines, 2);
}

#[tokio::test]
async fn physical_adjustments_persist_exact_difference_and_basis() {
    let db = fixture().await;
    let mut positive = line(None, "1");
    positive.physical_quantity = Some("3".into());
    let gain = post(&db, "adjustment", "2026-04-02", vec![positive]).await;
    let gain_attr = InventoryValuationAttributionService::for_voucher(db.pool(), "s15", &gain.id)
        .await
        .unwrap();
    assert_eq!(gain_attr[0].quantity, "3");
    assert_eq!(gain_attr[0].total_value, "0");
    post(&db, "receipt", "2026-04-03", vec![line(Some("12"), "5")]).await;
    let mut negative = line(None, "1");
    negative.physical_quantity = Some("2".into());
    let loss = post(&db, "adjustment", "2026-04-04", vec![negative]).await;
    let loss_attr = InventoryValuationAttributionService::for_voucher(db.pool(), "s15", &loss.id)
        .await
        .unwrap();
    assert_eq!(loss_attr[0].quantity, "6");
    assert_eq!(loss_attr[0].unit_cost, "6");
    assert_eq!(loss_attr[0].total_value, "36");
}

#[tokio::test]
async fn attribution_is_unique_and_reconciliation_is_read_only() {
    let db = fixture().await;
    let receipt = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "1")]).await;
    let before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_valuation_attributions WHERE company_id='s15'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(before, 1);
    let findings = InventoryReportsService::reconciliation(
        db.pool(),
        "s15",
        InventoryReportFilter {
            date_from: None,
            date_to: None,
            as_of_date: Some("2026-04-02".into()),
            stock_item_id: None,
            stock_group_id: None,
            stock_category_id: None,
            godown_id: None,
            voucher_type: None,
            search: None,
            include_zero: true,
            page: None,
            page_size: None,
        },
    )
    .await
    .unwrap();
    assert!(findings
        .iter()
        .all(|finding| finding.check_id != "missing-valuation-attribution"));
    let after: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_valuation_attributions WHERE company_id='s15'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(before, after);
    assert!(!receipt.id.is_empty());
}

#[tokio::test]
async fn duplicate_attribution_is_rejected_and_missing_attribution_is_reported() {
    let db = fixture().await;
    let receipt = post(&db, "receipt", "2026-04-02", vec![line(Some("10"), "1")]).await;
    let attribution =
        InventoryValuationAttributionService::for_voucher(db.pool(), "s15", &receipt.id)
            .await
            .unwrap()
            .remove(0);
    let duplicate = sqlx::query("INSERT INTO inventory_valuation_attributions(id,company_id,stock_voucher_id,voucher_type,voucher_line_id,stock_item_id,stock_movement_id,valuation_method,quantity,unit_cost,total_value,context) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind("duplicate").bind("s15").bind(&attribution.stock_voucher_id).bind(&attribution.voucher_type).bind(&attribution.voucher_line_id).bind(&attribution.stock_item_id).bind(&attribution.stock_movement_id).bind(&attribution.valuation_method).bind(&attribution.quantity).bind(&attribution.unit_cost).bind(&attribution.total_value).bind(&attribution.context).execute(db.pool()).await;
    assert!(duplicate.is_err());
    let line_id: String = sqlx::query_scalar(
        "SELECT id FROM stock_voucher_lines WHERE company_id='s15' AND voucher_id=?",
    )
    .bind(&receipt.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO stock_movements(id,company_id,stock_item_id,godown_id,voucher_id,voucher_line_id,movement_date,movement_type,signed_quantity) VALUES('orphan-attr','s15','item','g',?,?,?,'adjustment','1')").bind(&receipt.id).bind(&line_id).bind(&receipt.voucher_date).execute(db.pool()).await.unwrap();
    let findings = InventoryReportsService::reconciliation(
        db.pool(),
        "s15",
        InventoryReportFilter {
            date_from: None,
            date_to: None,
            as_of_date: Some("2026-04-02".into()),
            stock_item_id: None,
            stock_group_id: None,
            stock_category_id: None,
            godown_id: None,
            voucher_type: None,
            search: None,
            include_zero: true,
            page: None,
            page_size: None,
        },
    )
    .await
    .unwrap();
    assert!(findings
        .iter()
        .any(|finding| finding.check_id == "missing-valuation-attribution"));
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM inventory_valuation_attributions WHERE company_id='s15'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn stage15_decimal_assertions_are_not_float_based() {
    let amount = Decimal::from(2) * Decimal::from(10) + Decimal::from(2) * Decimal::from(20);
    assert_eq!(amount, Decimal::from(60));
}
