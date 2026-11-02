use crate::errors::OpenTallyError;
use sqlx::SqliteConnection;
use std::collections::HashMap;
use uuid::Uuid;

pub struct PredefinedGroupDef {
    pub name: &'static str,
    pub nature: &'static str,
    pub parent_name: Option<&'static str>,
    pub is_primary: bool,
    pub is_hidden: bool,
    pub affects_gross_profit: bool,
}

pub const PREDEFINED_GROUPS: &[PredefinedGroupDef] = &[
    // 15 Primary User Groups
    PredefinedGroupDef {
        name: "Branch / Divisions",
        nature: "Liabilities",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Capital Account",
        nature: "Liabilities",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Current Assets",
        nature: "Assets",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Current Liabilities",
        nature: "Liabilities",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Direct Expenses",
        nature: "Expenses",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: true,
    },
    PredefinedGroupDef {
        name: "Direct Incomes",
        nature: "Income",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: true,
    },
    PredefinedGroupDef {
        name: "Fixed Assets",
        nature: "Assets",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Indirect Expenses",
        nature: "Expenses",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Indirect Incomes",
        nature: "Income",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Investments",
        nature: "Assets",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Loans (Liability)",
        nature: "Liabilities",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Misc. Expenses (ASSET)",
        nature: "Assets",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Purchase Accounts",
        nature: "Expenses",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: true,
    },
    PredefinedGroupDef {
        name: "Sales Accounts",
        nature: "Income",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: true,
    },
    PredefinedGroupDef {
        name: "Suspense A/c",
        nature: "Liabilities",
        parent_name: None,
        is_primary: true,
        is_hidden: false,
        affects_gross_profit: false,
    },
    // 1 Dedicated Hidden System Group for Profit & Loss
    PredefinedGroupDef {
        name: "Profit & Loss A/c (System)",
        nature: "Liabilities",
        parent_name: None,
        is_primary: true,
        is_hidden: true,
        affects_gross_profit: false,
    },
    // 13 Secondary User Groups
    PredefinedGroupDef {
        name: "Reserves & Surplus",
        nature: "Liabilities",
        parent_name: Some("Capital Account"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Bank Accounts",
        nature: "Assets",
        parent_name: Some("Current Assets"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Cash-in-Hand",
        nature: "Assets",
        parent_name: Some("Current Assets"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Deposits (Asset)",
        nature: "Assets",
        parent_name: Some("Current Assets"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Loans & Advances (Asset)",
        nature: "Assets",
        parent_name: Some("Current Assets"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Stock-in-Hand",
        nature: "Assets",
        parent_name: Some("Current Assets"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Sundry Debtors",
        nature: "Assets",
        parent_name: Some("Current Assets"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Duties & Taxes",
        nature: "Liabilities",
        parent_name: Some("Current Liabilities"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Provisions",
        nature: "Liabilities",
        parent_name: Some("Current Liabilities"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Sundry Creditors",
        nature: "Liabilities",
        parent_name: Some("Current Liabilities"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Bank OD A/c",
        nature: "Liabilities",
        parent_name: Some("Loans (Liability)"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Secured Loans",
        nature: "Liabilities",
        parent_name: Some("Loans (Liability)"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
    PredefinedGroupDef {
        name: "Unsecured Loans",
        nature: "Liabilities",
        parent_name: Some("Loans (Liability)"),
        is_primary: false,
        is_hidden: false,
        affects_gross_profit: false,
    },
];

/// Atomically seeds default configuration and predefined hierarchy within an existing transaction connection:
/// 1. Default company settings (GST, inventory, billwise, currency decimals, fiscal year bounds)
/// 2. Exactly 29 predefined account groups (15 primary, 13 secondary, 1 hidden system group)
/// 3. Exactly 2 reserved ledgers (Cash under Cash-in-Hand, Profit & Loss A/c under Profit & Loss A/c System)
pub async fn seed_company_defaults(
    conn: &mut SqliteConnection,
    company_id: &str,
    decimals: i64,
    fy_begins: &str,
    books_begins: &str,
) -> Result<(), OpenTallyError> {
    // 1. Seed Company Settings with parameterized query
    sqlx::query(
        "INSERT INTO company_settings (company_id, key, value, updated_at) VALUES \
         ($1, 'enable_gst', 'true', CURRENT_TIMESTAMP), \
         ($1, 'maintain_inventory', 'true', CURRENT_TIMESTAMP), \
         ($1, 'integrate_accounts_inventory', 'true', CURRENT_TIMESTAMP), \
         ($1, 'enable_billwise', 'true', CURRENT_TIMESTAMP), \
         ($1, 'rounding_method', 'half_up', CURRENT_TIMESTAMP), \
         ($1, 'currency_decimal_places', $2, CURRENT_TIMESTAMP), \
         ($1, 'financial_year_begins', $3, CURRENT_TIMESTAMP), \
         ($1, 'books_beginning_from', $4, CURRENT_TIMESTAMP)",
    )
    .bind(company_id)
    .bind(decimals.to_string())
    .bind(fy_begins)
    .bind(books_begins)
    .execute(&mut *conn)
    .await
    .map_err(OpenTallyError::Database)?;

    // 2. Predefined Account Groups
    let mut group_ids: HashMap<&'static str, String> = HashMap::new();

    // First pass: Primary groups and hidden root system group (parent_name is None)
    for g in PREDEFINED_GROUPS.iter().filter(|g| g.parent_name.is_none()) {
        let gid = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_predefined, is_hidden, affects_gross_profit, created_at, updated_at) \
             VALUES ($1, $2, $3, NULL, $4, $5, 1, $6, $7, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .bind(&gid)
        .bind(company_id)
        .bind(g.name)
        .bind(g.nature)
        .bind(if g.is_primary { 1 } else { 0 })
        .bind(if g.is_hidden { 1 } else { 0 })
        .bind(if g.affects_gross_profit { 1 } else { 0 })
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        group_ids.insert(g.name, gid);
    }

    // Second pass: Secondary groups with parent_id foreign key
    for g in PREDEFINED_GROUPS.iter().filter(|g| g.parent_name.is_some()) {
        let parent_name = g.parent_name.ok_or_else(|| {
            OpenTallyError::Internal(format!("Predefined group '{}' missing parent name", g.name))
        })?;
        let parent_id = group_ids.get(parent_name).ok_or_else(|| {
            OpenTallyError::Internal(format!(
                "Parent group '{}' not found for predefined group '{}'",
                parent_name, g.name
            ))
        })?;
        let gid = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO account_groups (id, company_id, name, parent_id, nature, is_primary, is_predefined, is_hidden, affects_gross_profit, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, 0, 1, 0, $6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .bind(&gid)
        .bind(company_id)
        .bind(g.name)
        .bind(parent_id)
        .bind(g.nature)
        .bind(if g.affects_gross_profit { 1 } else { 0 })
        .execute(&mut *conn)
        .await
        .map_err(OpenTallyError::Database)?;

        group_ids.insert(g.name, gid);
    }

    // 3. Seed Reserved Ledgers
    let cash_group_id = group_ids.get("Cash-in-Hand").ok_or_else(|| {
        OpenTallyError::Internal("Cash-in-Hand group not found in seed map".to_string())
    })?;
    let cash_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, description, is_reserved, ledger_classification, created_at, updated_at) \
         VALUES ($1, $2, $3, 'Cash', 'Default Cash ledger', 1, 'cash', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(&cash_id)
    .bind(company_id)
    .bind(cash_group_id)
    .execute(&mut *conn)
    .await
    .map_err(OpenTallyError::Database)?;

    let pnl_group_id = group_ids.get("Profit & Loss A/c (System)").ok_or_else(|| {
        OpenTallyError::Internal(
            "Profit & Loss A/c (System) group not found in seed map".to_string(),
        )
    })?;
    let pnl_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ledgers (id, company_id, group_id, name, description, is_reserved, ledger_classification, created_at, updated_at) \
         VALUES ($1, $2, $3, 'Profit & Loss A/c', 'System Profit & Loss accumulator', 1, 'profit_and_loss', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
    )
    .bind(&pnl_id)
    .bind(company_id)
    .bind(pnl_group_id)
    .execute(&mut *conn)
    .await
    .map_err(OpenTallyError::Database)?;

    Ok(())
}
