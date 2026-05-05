//! Resource path resolution helpers.
//!
//! Runtime resources may be loaded from the current working directory during
//! development, or from locations relative to the executable in packaged builds.

use std::path::{Path, PathBuf};

fn candidate_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            roots.push(exe_dir.to_path_buf());

            if let Some(parent) = exe_dir.parent() {
                roots.push(parent.to_path_buf());
                roots.push(parent.join("Resources"));

                if let Some(grandparent) = parent.parent() {
                    roots.push(grandparent.to_path_buf());
                    roots.push(grandparent.join("Resources"));
                }
            }
        }
    }

    let mut unique = Vec::new();
    for root in roots {
        if !unique.iter().any(|r: &PathBuf| r == &root) {
            unique.push(root);
        }
    }
    unique
}

pub fn find_resource_dir(name: &str) -> Option<PathBuf> {
    candidate_roots()
        .into_iter()
        .map(|root| root.join(name))
        .find(|candidate| candidate.is_dir())
}

pub fn find_resource_file(relative_path: &Path) -> Option<PathBuf> {
    if relative_path.is_absolute() {
        return relative_path.exists().then(|| relative_path.to_path_buf());
    }

    candidate_roots()
        .into_iter()
        .map(|root| root.join(relative_path))
        .find(|candidate| candidate.is_file())
}

pub fn resolve_media_path(path: &Path) -> PathBuf {
    if path.is_absolute() || path.exists() {
        return path.to_path_buf();
    }

    if let Some(found) = find_resource_file(path) {
        return found;
    }

    let media_prefixed = Path::new("media").join(path);
    if let Some(found) = find_resource_file(&media_prefixed) {
        return found;
    }

    path.to_path_buf()
}
