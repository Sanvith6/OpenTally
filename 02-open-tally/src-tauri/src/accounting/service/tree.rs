use super::group::GroupService;
use super::ledger::LedgerService;
use super::opening_balance::OpeningBalanceService;
use crate::accounting::model::{
    AccountGroup, BalanceType, ChartOfAccountsTree, ChartOfAccountsTreeNode, OpeningBalance,
    OpeningBalanceSummary,
};
use crate::company::CompanyService;
use crate::errors::OpenTallyError;
use rust_decimal::Decimal;
use sqlx::SqlitePool;
use std::str::FromStr;

pub struct TreeService;

impl TreeService {
    /// Builds the complete Chart of Accounts hierarchical tree for the active company.
    pub async fn get_chart_of_accounts_tree(
        pool: &SqlitePool,
        company_id: &str,
    ) -> Result<ChartOfAccountsTree, OpenTallyError> {
        let company = CompanyService::get_company(pool, company_id).await?;
        let precision = company.currency_decimal_places as usize;

        let groups = GroupService::list_groups(pool, company_id, false).await?;
        let ledgers = LedgerService::list_ledgers(pool, company_id, None, None).await?;

        // Query all opening balances for current financial year
        let opbs: Vec<OpeningBalance> = sqlx::query_as::<_, OpeningBalance>(
            "SELECT id, company_id, ledger_id, financial_year_begins, balance_type, amount, created_at, updated_at
             FROM opening_balances
             WHERE company_id = ? AND financial_year_begins = ?"
        )
        .bind(company_id)
        .bind(&company.financial_year_begins)
        .fetch_all(pool)
        .await?;

        let opb_map: std::collections::HashMap<String, OpeningBalance> =
            opbs.into_iter().map(|b| (b.ledger_id.clone(), b)).collect();

        // 1. Convert ledgers to leaf nodes
        let mut group_children_map: std::collections::HashMap<
            String,
            Vec<ChartOfAccountsTreeNode>,
        > = std::collections::HashMap::new();

        for ledger in &ledgers {
            let mut dr_val = Decimal::ZERO;
            let mut cr_val = Decimal::ZERO;
            let opb_summary = if let Some(opb) = opb_map.get(&ledger.id) {
                let amt = Decimal::from_str(&opb.amount).unwrap_or(Decimal::ZERO);
                let btype = match opb.balance_type.as_str() {
                    "Dr" => {
                        dr_val = amt;
                        BalanceType::Dr
                    }
                    _ => {
                        cr_val = amt;
                        BalanceType::Cr
                    }
                };
                Some(OpeningBalanceSummary {
                    balance_type: btype,
                    amount: opb.amount.clone(),
                })
            } else {
                None
            };

            let node = ChartOfAccountsTreeNode {
                id: ledger.id.clone(),
                name: ledger.name.clone(),
                node_type: "ledger".to_string(),
                nature: "".to_string(),
                is_primary: false,
                is_predefined: false,
                is_reserved: ledger.is_reserved,
                parent_id: Some(ledger.group_id.clone()),
                opening_balance: opb_summary,
                debit_total: format!("{:.precision$}", dr_val, precision = precision),
                credit_total: format!("{:.precision$}", cr_val, precision = precision),
                children: Vec::new(),
            };

            group_children_map
                .entry(ledger.group_id.clone())
                .or_default()
                .push(node);
        }

        // 2. Build recursive tree for groups
        fn build_group_tree(
            group: &AccountGroup,
            all_groups: &[AccountGroup],
            children_map: &mut std::collections::HashMap<String, Vec<ChartOfAccountsTreeNode>>,
            precision: usize,
        ) -> ChartOfAccountsTreeNode {
            let mut my_children = children_map.remove(&group.id).unwrap_or_default();

            // Find child groups
            let child_groups: Vec<&AccountGroup> = all_groups
                .iter()
                .filter(|g| g.parent_id.as_deref() == Some(&group.id))
                .collect();

            for cg in child_groups {
                let child_node = build_group_tree(cg, all_groups, children_map, precision);
                my_children.push(child_node);
            }

            // Sum totals
            let mut total_dr = Decimal::ZERO;
            let mut total_cr = Decimal::ZERO;
            for child in &my_children {
                let dr = Decimal::from_str(&child.debit_total).unwrap_or(Decimal::ZERO);
                let cr = Decimal::from_str(&child.credit_total).unwrap_or(Decimal::ZERO);
                total_dr += dr;
                total_cr += cr;
            }

            ChartOfAccountsTreeNode {
                id: group.id.clone(),
                name: group.name.clone(),
                node_type: "group".to_string(),
                nature: group.nature.clone(),
                is_primary: group.is_primary,
                is_predefined: group.is_predefined,
                is_reserved: false,
                parent_id: group.parent_id.clone(),
                opening_balance: None,
                debit_total: format!("{:.precision$}", total_dr, precision = precision),
                credit_total: format!("{:.precision$}", total_cr, precision = precision),
                children: my_children,
            }
        }

        let primary_groups: Vec<&AccountGroup> = groups
            .iter()
            .filter(|g| g.is_primary && g.parent_id.is_none())
            .collect();

        let mut root_nodes = Vec::new();
        for pg in primary_groups {
            let root_node = build_group_tree(pg, &groups, &mut group_children_map, precision);
            root_nodes.push(root_node);
        }

        let equilibrium = OpeningBalanceService::get_equilibrium(pool, company_id, None).await?;

        Ok(ChartOfAccountsTree {
            nodes: root_nodes,
            equilibrium,
        })
    }
}
