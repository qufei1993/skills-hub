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
    #[serde(default)]
    pub visibility: RepositoryVisibility,
    #[serde(default)]
    pub public_upload_confirmed: bool,
    pub provider: ProviderId,
    pub remote_url: String,
    pub branch: String,
    pub username: Option<String>,
    pub credential_key: Option<String>,
    pub auto_check: bool,
    pub auto_sync: bool,
    #[serde(default)]
    pub auto_sync_schedule: Option<super::scheduler::SyncSchedule>,
    pub last_synced_commit: Option<String>,
}

impl Default for DeviceSyncConfig {
    fn default() -> Self {
        Self {
            visibility: RepositoryVisibility::Unknown,
            public_upload_confirmed: false,
            provider: ProviderId::Github,
            remote_url: String::new(),
            branch: "main".to_string(),
            username: None,
            credential_key: None,
            auto_check: false,
            auto_sync: false,
            auto_sync_schedule: None,
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
    #[serde(default)]
    pub visibility: RepositoryVisibility,
    pub name: String,
    pub web_url: String,
    pub clone_url: String,
    pub ssh_url: Option<String>,
    pub private: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RepositoryVisibility {
    Public,
    Private,
    Internal,
    #[default]
    Unknown,
}

impl DeviceSyncConfig {
    pub fn uses_https(&self) -> bool {
        reqwest::Url::parse(&self.remote_url).is_ok_and(|url| url.scheme() == "https")
    }

    pub fn needs_visibility_confirmation(&self) -> bool {
        self.uses_https() && self.visibility == RepositoryVisibility::Unknown
    }

    pub fn needs_public_upload_confirmation(&self) -> bool {
        self.visibility == RepositoryVisibility::Public && !self.public_upload_confirmed
    }
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncChangeItem {
    pub skill_id: String,
    pub name: String,
    pub kind: String,
    pub direction: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SyncChangeSummary {
    #[serde(default)]
    pub items: Vec<SyncChangeItem>,
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
    #[serde(default)]
    pub tool_issues: Vec<ToolSyncIssue>,
    #[serde(default)]
    pub schedule_status: Option<super::scheduler::ScheduleSummary>,
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
pub struct ToolSyncIssue {
    pub skill_name: String,
    pub tool: String,
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
    pub items: Option<Vec<SyncChangeItem>>,
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

#[cfg(test)]
mod tests {
    use super::DeviceSyncConfig;

    #[test]
    fn device_sync_defaults_do_not_access_remote_credentials_at_startup() {
        let config = DeviceSyncConfig::default();

        assert!(!config.auto_check);
        assert!(!config.auto_sync);
    }

    #[test]
    fn legacy_startup_sync_does_not_opt_into_recurring_sync() {
        let mut value = serde_json::to_value(DeviceSyncConfig::default()).unwrap();
        value.as_object_mut().unwrap().remove("auto_sync_schedule");
        value["auto_sync"] = serde_json::json!(true);
        let legacy: DeviceSyncConfig = serde_json::from_value(value).unwrap();
        assert!(legacy.auto_sync_schedule.is_none());
    }
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
