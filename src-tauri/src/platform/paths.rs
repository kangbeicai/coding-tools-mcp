use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{AppError, AppResult};

const APP_CONFIG_DIR_NAME: &str = "coding-tools-mcp";
const LEGACY_APP_CONFIG_DIR_NAME: &str = "coding-tools-mcp-desktop";

static APP_CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn app_config_dir() -> AppResult<PathBuf> {
    let base = dirs::config_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| AppError::Message("config dir not found".into()))?;
    let resolved = APP_CONFIG_DIR.get_or_init(|| {
        let resolution = resolve_app_config_dir_with(&base, |from, to| fs::rename(from, to));
        if let Some(warning) = resolution.migration_warning.as_deref() {
            eprintln!("warning: {warning}");
        }
        resolution.path
    });
    Ok(resolved.clone())
}

#[derive(Debug, PartialEq, Eq)]
struct AppConfigDirResolution {
    path: PathBuf,
    migration_warning: Option<String>,
}

fn resolve_app_config_dir_with<F>(base: &Path, rename: F) -> AppConfigDirResolution
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    let canonical = base.join(APP_CONFIG_DIR_NAME);
    if canonical.exists() {
        return AppConfigDirResolution {
            path: canonical,
            migration_warning: None,
        };
    }

    let legacy = base.join(LEGACY_APP_CONFIG_DIR_NAME);
    if !legacy.exists() {
        return AppConfigDirResolution {
            path: canonical,
            migration_warning: None,
        };
    }

    match rename(&legacy, &canonical) {
        Ok(()) => AppConfigDirResolution {
            path: canonical,
            migration_warning: None,
        },
        Err(_) if canonical.exists() => AppConfigDirResolution {
            path: canonical,
            migration_warning: None,
        },
        Err(error) => AppConfigDirResolution {
            path: legacy.clone(),
            migration_warning: Some(format!(
                "配置目录迁移失败（{} -> {}）：{error}；本次继续使用旧目录",
                legacy.display(),
                canonical.display()
            )),
        },
    }
}

pub fn resolve_from_path(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let paths = env::split_paths(&path_var);
    let mut candidates = vec![name.to_string()];
    if cfg!(windows) && Path::new(name).extension().is_none() {
        candidates.push(format!("{name}.exe"));
    }

    for dir in paths {
        for candidate in &candidates {
            let full = dir.join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

pub fn append_if_exists(paths: &mut Vec<PathBuf>, candidate: impl AsRef<Path>) {
    let candidate = candidate.as_ref();
    if candidate.is_file() {
        paths.push(candidate.to_path_buf());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_dir_defaults_to_canonical_without_legacy_data() {
        let temp = tempfile::tempdir().unwrap();
        let resolved = resolve_app_config_dir_with(temp.path(), |from, to| fs::rename(from, to));

        assert_eq!(resolved.path, temp.path().join(APP_CONFIG_DIR_NAME));
        assert!(resolved.migration_warning.is_none());
    }

    #[test]
    fn config_dir_renames_legacy_root_to_canonical_root() {
        let temp = tempfile::tempdir().unwrap();
        let legacy = temp.path().join(LEGACY_APP_CONFIG_DIR_NAME);
        fs::create_dir_all(legacy.join("data")).unwrap();
        fs::write(legacy.join("data/profiles.json"), "legacy").unwrap();

        let resolved = resolve_app_config_dir_with(temp.path(), |from, to| fs::rename(from, to));
        let canonical = temp.path().join(APP_CONFIG_DIR_NAME);

        assert_eq!(resolved.path, canonical);
        assert!(resolved.migration_warning.is_none());
        assert!(!legacy.exists());
        assert_eq!(
            fs::read_to_string(canonical.join("data/profiles.json")).unwrap(),
            "legacy"
        );
    }

    #[test]
    fn config_dir_prefers_existing_canonical_root_without_merging_legacy() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().join(APP_CONFIG_DIR_NAME);
        let legacy = temp.path().join(LEGACY_APP_CONFIG_DIR_NAME);
        fs::create_dir_all(&canonical).unwrap();
        fs::create_dir_all(&legacy).unwrap();
        fs::write(canonical.join("marker"), "new").unwrap();
        fs::write(legacy.join("marker"), "old").unwrap();

        let resolved = resolve_app_config_dir_with(temp.path(), |_from, _to| {
            panic!("rename must not run when canonical root already exists")
        });

        assert_eq!(resolved.path, canonical);
        assert!(resolved.migration_warning.is_none());
        assert_eq!(fs::read_to_string(canonical.join("marker")).unwrap(), "new");
        assert_eq!(fs::read_to_string(legacy.join("marker")).unwrap(), "old");
    }

    #[test]
    fn config_dir_falls_back_to_legacy_root_when_rename_fails() {
        let temp = tempfile::tempdir().unwrap();
        let canonical = temp.path().join(APP_CONFIG_DIR_NAME);
        let legacy = temp.path().join(LEGACY_APP_CONFIG_DIR_NAME);
        fs::create_dir_all(&legacy).unwrap();

        let resolved = resolve_app_config_dir_with(temp.path(), |_from, _to| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "blocked for test",
            ))
        });

        assert_eq!(resolved.path, legacy);
        assert!(resolved.migration_warning.is_some());
        assert!(!canonical.exists());
    }
}
