use serde::{de::DeserializeOwned, Serialize};
use std::path::PathBuf;

/// XDG-compatible cache directory: ~/.cache/grit/ (Linux) or ~/Library/Caches/grit/ (macOS)
fn cache_dir() -> Option<PathBuf> {
    let dir = dirs::cache_dir()?.join("grit");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn cache_path(key: &str) -> Option<PathBuf> {
    Some(cache_dir()?.join(format!("{}.json", key)))
}

/// Read a cached value. Returns None if missing or corrupt.
pub fn read<T: DeserializeOwned>(key: &str) -> Option<T> {
    let path = cache_path(key)?;
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write a value to cache. Silently ignores errors.
pub fn write<T: Serialize>(key: &str, value: &T) {
    if let Some(path) = cache_path(key) {
        if let Ok(data) = serde_json::to_string(value) {
            let _ = std::fs::write(path, data);
        }
    }
}

/// Sanitize owner/repo into a safe cache key segment
pub fn repo_key(owner: &str, repo: &str) -> String {
    format!("{}_{}", owner.replace('/', "_"), repo.replace('/', "_"))
}

/// Namespace cache key by forge name for multi-forge support
#[allow(dead_code)]
pub fn forge_repo_key(forge: &str, owner: &str, repo: &str) -> String {
    format!("{}_{}", forge, repo_key(owner, repo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_key_sanitizes_slashes() {
        assert_eq!(repo_key("foo/bar", "baz/qux"), "foo_bar_baz_qux");
    }

    #[test]
    fn repo_key_normal_input() {
        assert_eq!(repo_key("owner", "repo"), "owner_repo");
    }

    #[test]
    fn repo_key_empty_strings() {
        assert_eq!(repo_key("", ""), "_");
    }
}
