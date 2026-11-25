use crate::backup::{
    create_database_backup, get_backup_status, restore_database_backup, BackupManifest,
};
use crate::database::migrations::*;
use crate::database::DatabaseManager;
use crate::errors::OpenTallyError;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn test_backup_creation_and_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("opentally_active.db");
    let db = DatabaseManager::init(&db_path).await.unwrap();
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
    run_sprint6_stage17_party_migration(p).await.unwrap();
    run_sprint6_stage18_sales_migration(p).await.unwrap();
    run_milestone1_commercial_posting_migration(p)
        .await
        .unwrap();
    run_milestone2_billwise_migration(p).await.unwrap();
    run_milestone3_gst_migration(p).await.unwrap();
    run_milestone5_notes_migration(p).await.unwrap();

    // Insert a test company
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_bk', 'Backup Test Ltd', 'Backup Test Ltd', 'India', 'Delhi', '2026-04-01', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    let backup_dest = dir.path().join("backups").join("snapshot_01.db");
    let manifest = create_database_backup(p, &db_path, &backup_dest)
        .await
        .unwrap();

    assert!(backup_dest.exists(), "Backup destination file must exist");
    assert_eq!(manifest.destination_path, backup_dest.to_str().unwrap());
    assert_eq!(manifest.schema_version, "v0.7.0-notes");

    // Verify sidecar manifest file
    let sidecar_path = dir
        .path()
        .join("backups")
        .join("snapshot_01.db.manifest.json");
    assert!(sidecar_path.exists(), "Sidecar manifest file must exist");
    let manifest_content = std::fs::read_to_string(&sidecar_path).unwrap();
    let parsed_manifest: BackupManifest = serde_json::from_str(&manifest_content).unwrap();
    assert_eq!(parsed_manifest.sha256_checksum, manifest.sha256_checksum);

    // Verify SHA-256 matches actual file bytes
    let file_bytes = std::fs::read(&backup_dest).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(&file_bytes);
    let real_sha256 = format!("{:x}", hasher.finalize());
    assert_eq!(real_sha256, manifest.sha256_checksum);

    // Verify SQLite file header
    assert!(file_bytes.starts_with(b"SQLite format 3\0"));

    // Verify database_backups table has BACKUP_CREATED entry
    let backup_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM database_backups WHERE action = 'BACKUP_CREATED'")
            .fetch_one(p)
            .await
            .unwrap();
    assert_eq!(backup_count, 1);

    // Verify get_backup_status reflects this backup
    let status = get_backup_status(p, &db_path).await.unwrap();
    assert!(status.is_wal_active);
    assert_eq!(status.current_schema_version, "v0.7.0-notes");
    assert!(status.last_backup_file.is_some());
    assert_eq!(
        status.last_backup_sha256.as_deref(),
        Some(manifest.sha256_checksum.as_str())
    );
}

#[tokio::test]
async fn test_corrupted_backup_rejected_on_restore() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("opentally_corrupt_test.db");
    let db = DatabaseManager::init(&db_path).await.unwrap();
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
    run_sprint6_stage17_party_migration(p).await.unwrap();
    run_sprint6_stage18_sales_migration(p).await.unwrap();
    run_milestone1_commercial_posting_migration(p)
        .await
        .unwrap();
    run_milestone2_billwise_migration(p).await.unwrap();
    run_milestone3_gst_migration(p).await.unwrap();
    run_milestone5_notes_migration(p).await.unwrap();

    let backup_dest = dir.path().join("backups").join("valid_before_corrupt.db");
    let _manifest = create_database_backup(p, &db_path, &backup_dest)
        .await
        .unwrap();

    // Corrupt the backup file
    let mut corrupted_bytes = std::fs::read(&backup_dest).unwrap();
    for byte in corrupted_bytes.iter_mut().take(50).skip(20) {
        *byte = 0xFF;
    }
    std::fs::write(&backup_dest, corrupted_bytes).unwrap();

    // Attempt restore
    let restore_res = restore_database_backup(p, &db_path, &backup_dest).await;
    assert!(
        matches!(restore_res, Err(OpenTallyError::IntegrityViolation(_))),
        "Expected IntegrityViolation error, got {:?}",
        restore_res
    );
}

#[tokio::test]
async fn test_backup_restore_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("opentally_roundtrip.db");
    let db = DatabaseManager::init(&db_path).await.unwrap();
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
    run_sprint6_stage17_party_migration(p).await.unwrap();
    run_sprint6_stage18_sales_migration(p).await.unwrap();
    run_milestone1_commercial_posting_migration(p)
        .await
        .unwrap();
    run_milestone2_billwise_migration(p).await.unwrap();
    run_milestone3_gst_migration(p).await.unwrap();
    run_milestone5_notes_migration(p).await.unwrap();

    // 1. Initial State: Company A exists
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_orig', 'Original Corp', 'Original Corp', 'India', 'Delhi', '2026-04-01', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    // 2. Take Backup
    let backup_dest = dir.path().join("backups").join("clean_state.db");
    create_database_backup(p, &db_path, &backup_dest)
        .await
        .unwrap();

    // 3. Mutate Active Database: Insert Company B
    sqlx::query(
        "INSERT INTO companies (id, name, mailing_name, country, state, financial_year_begins, books_beginning_from)
         VALUES ('cmp_unwanted', 'Unwanted Corp', 'Unwanted Corp', 'India', 'Delhi', '2026-04-01', '2026-04-01')",
    )
    .execute(p)
    .await
    .unwrap();

    let count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM companies")
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(count_before, 2);

    // 4. Restore Backup
    let restore_result = restore_database_backup(p, &db_path, &backup_dest)
        .await
        .unwrap();

    assert!(restore_result.success);
    assert_eq!(restore_result.schema_version, "v0.7.0-notes");
    assert!(restore_result.safety_backup_path.is_some());

    // 5. Verify restored state: Only Company A exists (Unwanted Corp rolled back)
    let count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM companies")
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(count_after, 1);

    let comp_name: String = sqlx::query_scalar("SELECT name FROM companies WHERE id = 'cmp_orig'")
        .fetch_one(p)
        .await
        .unwrap();
    assert_eq!(comp_name, "Original Corp");

    // 6. Verify database_backups has RESTORE_COMPLETED entry
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM database_backups WHERE action = 'RESTORE_COMPLETED'",
    )
    .fetch_one(p)
    .await
    .unwrap();
    assert!(audit_count >= 1);
}
