use crate::errors::OpenTallyError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    Accountant,
    DataEntry,
    Auditor,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Admin => "Admin",
            Self::Accountant => "Accountant",
            Self::DataEntry => "DataEntry",
            Self::Auditor => "Auditor",
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = OpenTallyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Admin" => Ok(Self::Admin),
            "Accountant" => Ok(Self::Accountant),
            "DataEntry" => Ok(Self::DataEntry),
            "Auditor" => Ok(Self::Auditor),
            _ => Err(OpenTallyError::Validation(format!(
                "Invalid user role '{}'. Allowed roles: Admin, Accountant, DataEntry, Auditor",
                s
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionPermission {
    ManageUsers,
    BackupDatabase,
    RestoreDatabase,
    YearEndClosing,
    PostVoucher,
    CancelOrReverseVoucher,
    ViewFinancialStatements,
    ViewStandardReports,
    ManageBom,
    ExecuteManufacturingJournal,
    ManagePayroll,
}

impl UserRole {
    pub fn can_perform(&self, action: ActionPermission) -> bool {
        match action {
            ActionPermission::ManageUsers => matches!(self, Self::Admin),
            ActionPermission::BackupDatabase => matches!(self, Self::Admin | Self::Accountant),
            ActionPermission::RestoreDatabase => matches!(self, Self::Admin),
            ActionPermission::YearEndClosing => matches!(self, Self::Admin),
            ActionPermission::PostVoucher => {
                matches!(self, Self::Admin | Self::Accountant | Self::DataEntry)
            }
            ActionPermission::CancelOrReverseVoucher => {
                matches!(self, Self::Admin | Self::Accountant)
            }
            ActionPermission::ViewFinancialStatements => {
                matches!(self, Self::Admin | Self::Accountant | Self::Auditor)
            }
            ActionPermission::ViewStandardReports => {
                matches!(
                    self,
                    Self::Admin | Self::Accountant | Self::DataEntry | Self::Auditor
                )
            }
            ActionPermission::ManageBom => matches!(self, Self::Admin | Self::Accountant),
            ActionPermission::ExecuteManufacturingJournal => {
                matches!(self, Self::Admin | Self::Accountant | Self::DataEntry)
            }
            ActionPermission::ManagePayroll => matches!(self, Self::Admin | Self::Accountant),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanySecurityConfig {
    pub company_id: String,
    pub security_enabled: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CompanyUser {
    pub id: String,
    pub company_id: String,
    pub username: String,
    pub role: String,
    pub is_active: i64,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSessionInfo {
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
    pub logged_in_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityStatusDto {
    pub security_enabled: bool,
    pub has_users: bool,
    pub active_user: Option<AuthSessionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableSecurityInput {
    pub company_id: String,
    pub admin_username: String,
    pub admin_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisableSecurityInput {
    pub company_id: String,
    pub admin_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginInput {
    pub company_id: String,
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserInput {
    pub company_id: String,
    pub username: String,
    pub password: String,
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRoleInput {
    pub company_id: String,
    pub user_id: String,
    pub role: UserRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserPasswordInput {
    pub company_id: String,
    pub user_id: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeactivateUserInput {
    pub company_id: String,
    pub user_id: String,
}
