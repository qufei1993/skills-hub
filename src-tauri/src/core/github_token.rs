use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};

use super::device_sync::credentials::{
    inspect_personal_access_token, resolve_personal_access_token, save_personal_access_token,
    CredentialStore, PersonalAccessTokenLookup,
};
use super::device_sync::types::{CredentialUsage, ProviderId};
use super::skill_store::SkillStore;

const LEGACY_GITHUB_TOKEN_SETTING: &str = "github_token";
const GITHUB_TOKEN_SECURE_CLEANUP_PENDING_SETTING: &str = "github_token_secure_cleanup_pending";
pub const GITHUB_TOKEN_CREDENTIAL_KEY: &str = "github-search-personal-access-token-v1";
pub(crate) const GITHUB_TOKEN_KEYRING_SERVICE: &str = "com.skills-hub.github-token";
static GITHUB_TOKEN_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock_github_token() -> Result<std::sync::MutexGuard<'static, ()>> {
    GITHUB_TOKEN_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow::anyhow!("GitHub token credential lock is poisoned"))
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemGithubTokenStore;

impl CredentialStore for SystemGithubTokenStore {
    fn set(&self, key: &str, secret: &str) -> Result<()> {
        let entry = keyring::Entry::new(GITHUB_TOKEN_KEYRING_SERVICE, key)
            .context("open GitHub token credential store")?;
        entry
            .set_password(secret)
            .context("save GitHub token credential")
    }

    fn get(&self, key: &str) -> Result<Option<String>> {
        let entry = keyring::Entry::new(GITHUB_TOKEN_KEYRING_SERVICE, key)
            .context("open GitHub token credential store")?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err).context("read GitHub token credential"),
        }
    }

    fn delete(&self, key: &str) -> Result<()> {
        let entry = keyring::Entry::new(GITHUB_TOKEN_KEYRING_SERVICE, key)
            .context("open GitHub token credential store")?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err).context("delete GitHub token credential"),
        }
    }
}

fn migrate_legacy_github_token(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> Result<()> {
    let legacy_token = store
        .get_setting(LEGACY_GITHUB_TOKEN_SETTING)?
        .unwrap_or_default();
    let legacy_token = legacy_token.trim();
    if legacy_token.is_empty() {
        if store
            .get_setting(GITHUB_TOKEN_SECURE_CLEANUP_PENDING_SETTING)?
            .is_some()
        {
            securely_delete_legacy_github_token(store)?;
        }
        return Ok(());
    }

    let usage = CredentialUsage::official(ProviderId::Github);
    let existing = inspect_personal_access_token(credentials, GITHUB_TOKEN_CREDENTIAL_KEY, &usage)?;
    if matches!(
        existing,
        PersonalAccessTokenLookup::Missing | PersonalAccessTokenLookup::Invalid
    ) {
        save_personal_access_token(
            credentials,
            GITHUB_TOKEN_CREDENTIAL_KEY,
            &usage,
            legacy_token,
        )
        .context("migrate GitHub token to system credential store")?;
    }
    securely_delete_legacy_github_token(store)?;
    Ok(())
}

fn securely_delete_legacy_github_token(store: &SkillStore) -> Result<()> {
    store.secure_delete_setting_with_pending_marker(
        LEGACY_GITHUB_TOKEN_SETTING,
        GITHUB_TOKEN_SECURE_CLEANUP_PENDING_SETTING,
    )
}

pub fn resolve_github_token(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
) -> Result<Option<String>> {
    let _guard = lock_github_token()?;
    migrate_legacy_github_token(store, credentials)?;
    resolve_personal_access_token(
        credentials,
        GITHUB_TOKEN_CREDENTIAL_KEY,
        &CredentialUsage::official(ProviderId::Github),
    )
}

pub fn has_github_token(store: &SkillStore, credentials: &dyn CredentialStore) -> Result<bool> {
    Ok(resolve_github_token(store, credentials)?.is_some())
}

pub fn set_github_token(
    store: &SkillStore,
    credentials: &dyn CredentialStore,
    token: &str,
) -> Result<()> {
    let _guard = lock_github_token()?;
    let token = token.trim();
    if token.is_empty() {
        migrate_legacy_github_token(store, credentials)?;
        credentials
            .delete(GITHUB_TOKEN_CREDENTIAL_KEY)
            .context("delete GitHub token from system credential store")?;
    } else {
        save_personal_access_token(
            credentials,
            GITHUB_TOKEN_CREDENTIAL_KEY,
            &CredentialUsage::official(ProviderId::Github),
            token,
        )
        .context("save GitHub token to system credential store")?;
        securely_delete_legacy_github_token(store)?;
    }
    Ok(())
}
