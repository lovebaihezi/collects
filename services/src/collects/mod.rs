use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[sqlx(type_name = "privacy_kind", rename_all = "lowercase")]
pub enum PrivacyKind {
    Public,
    Private,
    Protected,
}

impl Default for PrivacyKind {
    fn default() -> Self {
        PrivacyKind::Public
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Collect {
    pub id: i32,
    pub author_id: String,
    pub content: String,
    #[serde(default)]
    pub privacy_level: PrivacyKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}
