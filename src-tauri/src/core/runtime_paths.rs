use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::skill_store::{migrate_legacy_db_if_needed_in_data_dir, SkillStore};

#[allow(dead_code)]
pub const PRODUCT_IDENTIFIER: &str = "com.qufei1993.skillshub";
#[allow(dead_code)]
pub const DEVELOPMENT_PRODUCT_IDENTIFIER: &str = "com.qufei1993.skillshub.dev";
const TEST_PRODUCT_IDENTIFIER: &str = "com.qufei1993.skillshub.test";
const DATABASE_FILE_NAME: &str = "skills_hub.db";
const GIT_CACHE_DIR_NAME: &str = "skills-hub-git-cache";

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProfile {
    Production,
    Development,
    Test,
}

impl RuntimeProfile {
    pub const fn current() -> Self {
        if cfg!(debug_assertions) {
            Self::Development
        } else {
            Self::Production
        }
    }

    #[allow(dead_code)]
    const fn identifier(self) -> &'static str {
        match self {
            Self::Production => PRODUCT_IDENTIFIER,
            Self::Development => DEVELOPMENT_PRODUCT_IDENTIFIER,
            Self::Test => TEST_PRODUCT_IDENTIFIER,
        }
    }

    const fn central_repo_name(self) -> &'static str {
        match self {
            Self::Production => ".skillshub",
            Self::Development => ".skillshub-dev",
            Self::Test => ".skillshub-test",
        }
    }

    const fn cli_bridge_name(self) -> &'static str {
        match self {
            Self::Production => ".skills-hub",
            Self::Development => ".skills-hub-dev",
            Self::Test => ".skills-hub-test",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimePaths {
    pub profile: RuntimeProfile,
    pub app_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub default_central_repo: PathBuf,
    pub git_cache_dir: PathBuf,
    pub recycle_bin_dir: PathBuf,
    pub cli_bridge_dir: PathBuf,
}

impl RuntimePaths {
    #[allow(dead_code)]
    pub fn from_roots(
        profile: RuntimeProfile,
        home_root: impl AsRef<Path>,
        data_root: impl AsRef<Path>,
    ) -> Self {
        let app_data_dir = data_root.as_ref().join(profile.identifier());
        Self::from_tauri(profile, home_root, app_data_dir.clone(), app_data_dir)
    }

    #[allow(dead_code)]
    pub fn for_cli(profile: RuntimeProfile) -> Result<Self> {
        let home_root = dirs::home_dir().context("failed to resolve home directory")?;
        let data_root = dirs::data_dir().context("failed to resolve data directory")?;
        let cache_root = dirs::cache_dir().context("failed to resolve cache directory")?;
        let identifier = profile.identifier();
        Ok(Self::from_tauri(
            profile,
            home_root,
            data_root.join(identifier),
            cache_root.join(identifier),
        ))
    }

    pub fn from_tauri(
        profile: RuntimeProfile,
        home_root: impl AsRef<Path>,
        app_data_dir: impl Into<PathBuf>,
        app_cache_dir: impl AsRef<Path>,
    ) -> Self {
        let app_data_dir = app_data_dir.into();
        Self {
            profile,
            database_path: app_data_dir.join(DATABASE_FILE_NAME),
            default_central_repo: home_root.as_ref().join(profile.central_repo_name()),
            git_cache_dir: app_cache_dir.as_ref().join(GIT_CACHE_DIR_NAME),
            recycle_bin_dir: app_data_dir.join("recycle-bin"),
            cli_bridge_dir: home_root
                .as_ref()
                .join(profile.cli_bridge_name())
                .join("bin"),
            app_data_dir,
        }
    }
}

pub fn open_store(paths: &RuntimePaths) -> Result<SkillStore> {
    std::fs::create_dir_all(&paths.app_data_dir)
        .with_context(|| format!("failed to create app data dir {:?}", paths.app_data_dir))?;
    let data_root = paths
        .app_data_dir
        .parent()
        .context("app data directory has no data root")?;
    migrate_legacy_db_if_needed_in_data_dir(&paths.database_path, data_root)?;
    let store = SkillStore::new(paths.database_path.clone());
    store.ensure_schema()?;
    store.migrate_device_sync_startup_credential_consent()?;
    Ok(store)
}
