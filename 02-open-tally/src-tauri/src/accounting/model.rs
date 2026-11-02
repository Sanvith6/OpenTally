use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountGroupNature {
    Assets,
    Liabilities,
    Income,
    Expenses,
}

impl AccountGroupNature {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Assets => "Assets",
            Self::Liabilities => "Liabilities",
            Self::Income => "Income",
            Self::Expenses => "Expenses",
        }
    }
}

impl std::str::FromStr for AccountGroupNature {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Assets" => Ok(Self::Assets),
            "Liabilities" => Ok(Self::Liabilities),
            "Income" => Ok(Self::Income),
            "Expenses" => Ok(Self::Expenses),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AccountGroup {
    pub id: String,
    pub company_id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub nature: String,
    pub is_primary: bool,
    pub is_predefined: bool,
    pub is_hidden: bool,
    pub affects_gross_profit: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountGroupRequest {
    pub name: String,
    pub parent_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAccountGroupRequest {
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LedgerClassification {
    Standard,
    Cash,
    ProfitAndLoss,
    Bank,
    Tax,
    Party,
}

impl LedgerClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Cash => "cash",
            Self::ProfitAndLoss => "profit_and_loss",
            Self::Bank => "bank",
            Self::Tax => "tax",
            Self::Party => "party",
        }
    }
}

impl std::str::FromStr for LedgerClassification {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "standard" => Ok(Self::Standard),
            "cash" => Ok(Self::Cash),
            "profit_and_loss" => Ok(Self::ProfitAndLoss),
            "bank" => Ok(Self::Bank),
            "tax" => Ok(Self::Tax),
            "party" => Ok(Self::Party),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Ledger {
    pub id: String,
    pub company_id: String,
    pub group_id: String,
    pub name: String,
    pub ledger_classification: String,
    pub is_reserved: bool,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateLedgerRequest {
    pub name: String,
    pub group_id: String,
    pub description: Option<String>,
    pub opening_balance: Option<SetOpeningBalanceRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateLedgerRequest {
    pub name: String,
    pub group_id: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceType {
    Dr,
    Cr,
}

impl BalanceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Dr => "Dr",
            Self::Cr => "Cr",
        }
    }
}

impl std::str::FromStr for BalanceType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Dr" => Ok(Self::Dr),
            "Cr" => Ok(Self::Cr),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OpeningBalance {
    pub id: String,
    pub company_id: String,
    pub ledger_id: String,
    pub financial_year_begins: String,
    pub balance_type: String,
    pub amount: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetOpeningBalanceRequest {
    pub ledger_id: String,
    pub financial_year_begins: Option<String>,
    pub balance_type: BalanceType,
    pub amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningBalanceEquilibrium {
    pub total_debit: String,
    pub total_credit: String,
    pub difference: String,
    pub difference_type: Option<BalanceType>,
    pub is_balanced: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartOfAccountsTree {
    pub nodes: Vec<ChartOfAccountsTreeNode>,
    pub equilibrium: OpeningBalanceEquilibrium,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartOfAccountsTreeNode {
    pub id: String,
    pub name: String,
    pub node_type: String, // "group" or "ledger"
    pub nature: String,
    pub is_primary: bool,
    pub is_predefined: bool,
    pub is_reserved: bool,
    pub parent_id: Option<String>,
    pub opening_balance: Option<OpeningBalanceSummary>,
    pub debit_total: String,
    pub credit_total: String,
    pub children: Vec<ChartOfAccountsTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpeningBalanceSummary {
    pub balance_type: BalanceType,
    pub amount: String,
}
