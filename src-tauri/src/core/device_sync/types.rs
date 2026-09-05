use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    #[default]
    Github,
    Gitlab,
    Gitee,
}

impl ProviderId {
    pub fn official_git_host(self) -> &'static str {
        match self {
            Self::Github => "github.com",
            Self::Gitlab => "gitlab.com",
            Self::Gitee => "gitee.com",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CredentialUsage {
    pub provider: ProviderId,
    pub origin: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeviceSyncConfig {
    pub provider: ProviderId,
    pub remote_url: String,
    pub branch: String,
    pub username: Option<String>,
    pub credential_key: Option<String>,
    pub auto_check: bool,
    pub auto_sync: bool,
    pub last_synced_commit: Option<String>,
}

impl Default for DeviceSyncConfig {
    fn default() -> Self {
        Self {
            provider: ProviderId::Github,
            remote_url: String::new(),
            branch: "main".to_string(),
            username: None,
            credential_key: None,
            auto_check: true,
            auto_sync: false,
            last_synced_commit: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProviderAccount {
    pub login: String,
    pub display_name: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RemoteRepository {
    pub name: String,
    pub web_url: String,
    pub clone_url: String,
    pub ssh_url: Option<String>,
    pub private: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OAuthProviderAvailability {
    pub provider: ProviderId,
    pub available: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OAuthStartResult {
    pub flow_id: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: Option<String>,
    pub expires_at: i64,
    pub interval_seconds: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OAuthPollStatus {
    Pending,
    Authorized,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct OAuthPollResult {
    pub provider: ProviderId,
    pub status: OAuthPollStatus,
    pub interval_seconds: u64,
    pub credential_key: Option<String>,
    pub account: Option<ProviderAccount>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PendingOAuthAuthorization {
    pub provider: ProviderId,
    pub credential_key: String,
    pub account: ProviderAccount,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncChangeSummary {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub conflicted: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncRunResult {
    pub status: String,
    pub commit: Option<String>,
    pub changes: SyncChangeSummary,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncStatus {
    pub configured: bool,
    pub is_running: bool,
    pub provider: ProviderId,
    pub remote_url: String,
    pub auto_check: bool,
    pub auto_sync: bool,
    pub last_synced_commit: Option<String>,
    pub repository_head_commit: Option<String>,
    pub pending_local_changes: usize,
    pub conflict_count: usize,
    pub last_run_status: Option<String>,
    pub last_run_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DeviceSyncDevice {
    pub id: String,
    pub name: String,
    pub alias: Option<String>,
    pub last_commit: Option<String>,
    pub last_seen_at: i64,
    pub is_current: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncHistoryEntry {
    pub id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub status: String,
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub conflicted: usize,
    pub commit: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncConflict {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub base_commit: Option<String>,
    pub local_commit: String,
    pub remote_commit: String,
    pub files: Vec<String>,
    pub created_at: i64,
    pub status: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    KeepLocal,
    UseRemote,
    KeepBoth,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct TrashEntry {
    pub id: String,
    pub skill_id: String,
    pub skill_name: String,
    pub trash_path: String,
    pub deleted_at: i64,
    pub expires_at: i64,
}
