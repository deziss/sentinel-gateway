use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    ReadOnly,
    User,
    TenantAdmin,
    SuperAdmin,
}

impl Role {
    pub fn can(&self, required: &Role) -> bool {
        self >= required
    }

    pub fn is_super_admin(&self) -> bool {
        matches!(self, Role::SuperAdmin)
    }

    pub fn is_at_least_tenant_admin(&self) -> bool {
        matches!(self, Role::TenantAdmin | Role::SuperAdmin)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::ReadOnly => write!(f, "read_only"),
            Role::User => write!(f, "user"),
            Role::TenantAdmin => write!(f, "tenant_admin"),
            Role::SuperAdmin => write!(f, "super_admin"),
        }
    }
}
