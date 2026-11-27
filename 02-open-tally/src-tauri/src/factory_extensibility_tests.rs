use crate::company::{CompanyService, CreateCompanyRequest};
use crate::database::DatabaseManager;
use crate::extensions::{
    CreateCustomFieldInput, CreateWebhookInput, CustomFieldService, CustomFieldValueInput,
    HardwareBridgeService, WebhookService,
};
use crate::inventory::bom::BomService;
use crate::inventory::job_work::{
    DispatchJobWorkOutRequest, JobWorkLineInput, JobWorkService, ReceiveJobWorkInRequest,
};
use rust_decimal::Decimal;
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn test_webhook_target_host_is_parsed_not_prefix_matched() {
    for url in ["http://localhost.attacker.test/hook", "http://127.0.0.1.attacker.test/hook", "http://localhost@attacker.test/hook", "https://"] {
        assert!(WebhookService::validate_target_url(url).is_err(), "accepted {url}");
    }
    for url in ["https://example.test/hook", "http://localhost:9001/hook", "http://127.0.0.1:9001/hook"] {
        assert!(WebhookService::validate_target_url(url).is_ok(), "rejected {url}");
    }
}

#[tokio::test]
async fn test_nested_bom_preview_honors_each_base_output_quantity() {
    use crate::inventory::bom::{CreateBomInput, CreateBomComponentInput};
    let (_dir, db, cid) = setup_m25_db().await;
    let pool = db.pool();
    let unit = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO units (id,company_id,symbol,formal_name) VALUES (?,?,'NOS','Number')")
        .bind(&unit).bind(&cid).execute(pool).await.unwrap();
    let group = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO stock_groups (id,company_id,name) VALUES (?,?,'Preview materials')")
        .bind(&group).bind(&cid).execute(pool).await.unwrap();
    let items: Vec<String> = (0..3).map(|_| Uuid::new_v4().to_string()).collect();
    for (index,item) in items.iter().enumerate() {
        sqlx::query("INSERT INTO stock_items (id,company_id,stock_group_id,name,base_unit_id) VALUES (?,?,?,?,?)")
            .bind(item).bind(&cid).bind(&group).bind(format!("Preview item {index}")).bind(&unit).execute(pool).await.unwrap();
    }
    let component = |id: &str, qty: &str| CreateBomComponentInput { raw_stock_item_id: id.into(), source_godown_id:None, required_qty:qty.into() };
    BomService::create_bom(pool,CreateBomInput { company_id:cid.clone(),finished_stock_item_id:items[1].clone(),bom_name:"Nested batch of three".into(),base_output_qty:"3".into(),default_godown_id:None,components:vec![component(&items[2],"9")] },"test").await.unwrap();
    let root = BomService::create_bom(pool,CreateBomInput { company_id:cid.clone(),finished_stock_item_id:items[0].clone(),bom_name:"Finished batch of two".into(),base_output_qty:"2".into(),default_godown_id:None,components:vec![component(&items[1],"3")] },"test").await.unwrap();
    let expanded = BomService::expand_multilevel_bom(pool,&cid,&root.id,Decimal::from(4)).await.unwrap();
    assert_eq!(expanded,vec![(items[2].clone(),Decimal::from(18))]);
}

async fn setup_m25_db() -> (TempDir, DatabaseManager, String) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("m25_factory_ext.db");
    let db = DatabaseManager::init(&db_path).await.expect("db init");
    crate::database::migrations::run_all_migrations(db.pool())
        .await
        .expect("migrations");
    let comp = CompanyService::create_company(
        db.pool(),
        CreateCompanyRequest {
            name: "M25 Factory Corp".to_string(),
            mailing_name: None,
            country: Some("India".to_string()),
            state: Some("Gujarat".to_string()),
            financial_year_begins: "2026-04-01".to_string(),
            books_beginning_from: Some("2026-04-01".to_string()),
            currency_symbol: Some("₹".to_string()),
            currency_formal_name: Some("INR".to_string()),
            currency_decimal_places: Some(2),
        },
    )
    .await
    .expect("create company");
    (dir, db, comp.id)
}

#[tokio::test]
async fn test_m25_job_work_sec143_itc04_and_over_return_rejection() {
    let (_dir, db, company_id) = setup_m25_db().await;
    let pool = db.pool();

    // Sec 143 due dates
    assert_eq!(
        JobWorkService::compute_sec143_due_date("2026-04-01", "Input").unwrap(),
        "2027-04-01"
    );
    assert_eq!(
        JobWorkService::compute_sec143_due_date("2026-04-01", "CapitalGood").unwrap(),
        "2029-03-31"
    );

    let ledger_id: String =
        sqlx::query_scalar("SELECT id FROM ledgers WHERE company_id = ? LIMIT 1")
            .bind(&company_id)
            .fetch_one(pool)
            .await
            .expect("ledger");

    let ext_godown_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO godowns (id, company_id, name, is_external) VALUES (?, ?, 'JobWorker Yard', 1)",
    )
    .bind(&ext_godown_id)
    .bind(&company_id)
    .execute(pool)
    .await
    .expect("ext godown");

    let unit_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO units (id, company_id, symbol, formal_name) VALUES (?, ?, 'KG', 'Kilogram')",
    )
    .bind(&unit_id)
    .bind(&company_id)
    .execute(pool)
    .await
    .expect("unit");

    let sg_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO stock_groups (id, company_id, name) VALUES (?, ?, 'Raw Materials')")
        .bind(&sg_id)
        .bind(&company_id)
        .execute(pool)
        .await
        .expect("stock_group");

    let item_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO stock_items (id, company_id, stock_group_id, name, base_unit_id, barcode)
         VALUES (?, ?, ?, 'Steel Coil', ?, '8901234567890')",
    )
    .bind(&item_id)
    .bind(&company_id)
    .bind(&sg_id)
    .bind(&unit_id)
    .execute(pool)
    .await
    .expect("item");

    let out_challan = JobWorkService::dispatch_job_work_material_out(
        pool,
        &company_id,
        DispatchJobWorkOutRequest {
            challan_number: "JW-OUT-001".to_string(),
            challan_date: "2026-05-01".to_string(),
            job_worker_ledger_id: ledger_id,
            source_godown_id: ext_godown_id.clone(),
            external_godown_id: ext_godown_id.clone(),
            goods_category: "Input".to_string(),
            narration: Some("Coils for slitting".to_string()),
            lines: vec![JobWorkLineInput {
                stock_item_id: item_id.clone(),
                qty: "100".to_string(),
                unit_cost: "50.00".to_string(),
                scrap_qty: None,
            }],
        },
    )
    .await
    .expect("dispatch");
    assert_eq!(out_challan.statutory_due_date, "2027-05-01");
    assert_eq!(out_challan.status, "Open");

    // Over-return must be rejected
    let over_err = JobWorkService::receive_job_work_material_in(
        pool,
        &company_id,
        ReceiveJobWorkInRequest {
            challan_number: "JW-IN-ERR".to_string(),
            challan_date: "2026-05-10".to_string(),
            linked_out_challan_id: out_challan.id.clone(),
            destination_godown_id: ext_godown_id.clone(),
            finished_stock_item_id: item_id.clone(),
            finished_qty: "95".to_string(),
            returned_raw_qty: "95".to_string(),
            scrap_qty: "10".to_string(),
            job_work_charges: "500.00".to_string(),
            scrap_recovery_value: "50.00".to_string(),
            narration: None,
        },
    )
    .await
    .unwrap_err();
    assert!(over_err.to_string().contains("Over-return"));

    // Valid partial/full return with scrap
    let in_challan = JobWorkService::receive_job_work_material_in(
        pool,
        &company_id,
        ReceiveJobWorkInRequest {
            challan_number: "JW-IN-001".to_string(),
            challan_date: "2026-05-10".to_string(),
            linked_out_challan_id: out_challan.id.clone(),
            destination_godown_id: ext_godown_id,
            finished_stock_item_id: item_id,
            finished_qty: "95".to_string(),
            returned_raw_qty: "95".to_string(),
            scrap_qty: "5".to_string(),
            job_work_charges: "500.00".to_string(),
            scrap_recovery_value: "50.00".to_string(),
            narration: None,
        },
    )
    .await
    .expect("receive");
    assert_eq!(in_challan.challan_type, "MaterialIn");

    let itc04 = JobWorkService::get_itc04_report(pool, &company_id, "2026-04-01", "2026-06-30")
        .await
        .expect("itc04");
    assert_eq!(itc04.table_4.len(), 1);
    assert_eq!(itc04.table_5a.len(), 1);
    assert_eq!(itc04.table_5a[0].pending_qty, "0");

    let json = JobWorkService::export_itc04_json(pool, &company_id, "2026-04-01", "2026-06-30")
        .await
        .expect("itc04 json");
    assert!(json.contains("JW-OUT-001"));
    assert!(json.contains("JW-IN-001"));

    // Barcode lookup
    let found =
        HardwareBridgeService::lookup_stock_item_by_barcode(pool, &company_id, "8901234567890")
            .await
            .expect("barcode")
            .expect("found");
    assert_eq!(found.1, "Steel Coil");
}

#[tokio::test]
async fn test_m25_bom_byproduct_costing_and_extensions() {
    let (_dir, db, company_id) = setup_m25_db().await;
    let pool = db.pool();

    // Scrap / by-product recovery reduces net unit cost
    let net_unit = BomService::compute_net_unit_cost_with_byproducts(
        Decimal::from(1000),
        Decimal::from(200),
        Decimal::from(150),
        Decimal::from(10),
    )
    .unwrap();
    assert_eq!(net_unit, Decimal::new(1050000, 4));

    // Custom fields + validation
    let def = CustomFieldService::create_definition(
        pool,
        &company_id,
        CreateCustomFieldInput {
            entity_scope: "Voucher".to_string(),
            field_key: "vehicle_net_wt".to_string(),
            label: "Vehicle Net Wt".to_string(),
            field_type: "Decimal".to_string(),
            is_required: false,
            show_on_pdf: true,
        },
    )
    .await
    .expect("create cf");

    CustomFieldService::set_custom_field_values(
        pool,
        &company_id,
        "vch-1",
        vec![CustomFieldValueInput {
            definition_id: def.id,
            value_text: "100.00".to_string(),
        }],
    )
    .await
    .expect("set cf");

    let pdf_fields = CustomFieldService::get_pdf_custom_fields(pool, &company_id, "vch-1")
        .await
        .expect("pdf cf");
    assert_eq!(pdf_fields.len(), 1);
    assert_eq!(pdf_fields[0].0, "Vehicle Net Wt");
    assert_eq!(pdf_fields[0].1, "100.00");

    // Webhook HTTPS validation + HMAC-SHA256
    assert!(WebhookService::create_subscription(
        pool,
        &company_id,
        CreateWebhookInput {
            name: "Bad HTTP".to_string(),
            event_type: "voucher.posted".to_string(),
            target_url: "http://example.com/hook".to_string(),
            hmac_secret: "sec".to_string(),
        },
    )
    .await
    .is_err());

    let sig = WebhookService::hmac_sha256_hex("my-secret", r#"{"event":"voucher.posted"}"#);
    assert!(sig.starts_with("sha256="));
    assert_eq!(sig.len(), 7 + 64);

    // Weighbridge RS232 frame parsing
    let reading =
        HardwareBridgeService::parse_weighbridge_reading("ST,GS,+00125.50kg", "25.50").unwrap();
    assert_eq!(reading.gross_weight, "125.50");
    assert_eq!(reading.tare_weight, "25.50");
    assert_eq!(reading.net_weight, "100.00");
    assert_eq!(reading.unit, "KG");
}
