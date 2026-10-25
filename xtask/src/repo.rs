//! A snapshot of the repository's files: every path, and the contents of the
//! Markdown and litmus sources the checks read.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Directories that are never part of the checked sources.
const SKIPPED_DIRS: &[&str] = &[".git", "target", "node_modules"];

pub struct Repo {
    /// Every file, as a path relative to the root with `/` separators.
    pub paths: BTreeSet<String>,
    /// Contents of the `.md` and `.lc` files.
    pub texts: BTreeMap<String, String>,
}

impl Repo {
    pub fn load(root: &Path) -> Result<Repo, String> {
        let mut paths = BTreeSet::new();
        walk(root, "", &mut paths)?;
        let mut texts = BTreeMap::new();
        for path in &paths {
            if path.ends_with(".md") || path.ends_with(".lc") {
                let text = fs::read_to_string(root.join(path))
                    .map_err(|e| format!("{path}: cannot read: {e}"))?;
                texts.insert(path.clone(), text);
            }
        }
        Ok(Repo { paths, texts })
    }

    #[cfg(test)]
    pub fn from_files(files: &[(&str, &str)]) -> Repo {
        Repo {
            paths: files.iter().map(|(p, _)| p.to_string()).collect(),
            texts: files
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string()))
                .collect(),
        }
    }

    pub fn text(&self, path: &str) -> Option<&str> {
        self.texts.get(path).map(String::as_str)
    }

    /// Text files whose path starts with `prefix` and ends with `suffix`.
    pub fn texts_matching<'a>(
        &'a self,
        prefix: &'a str,
        suffix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.texts
            .iter()
            .filter(move |(p, _)| p.starts_with(prefix) && p.ends_with(suffix))
            .map(|(p, t)| (p.as_str(), t.as_str()))
    }

    /// Whether `path` names a file or a directory containing files.
    pub fn exists(&self, path: &str) -> bool {
        if path.is_empty() || self.paths.contains(path) {
            return true;
        }
        let dir = format!("{}/", path.trim_end_matches('/'));
        self.paths
            .range(dir.clone()..)
            .next()
            .is_some_and(|p| p.starts_with(&dir))
    }
}

fn walk(root: &Path, relative: &str, paths: &mut BTreeSet<String>) -> Result<(), String> {
    let dir = root.join(relative);
    let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        let kind = entry.file_type().map_err(|e| format!("{path}: {e}"))?;
        if kind.is_dir() {
            if !SKIPPED_DIRS.contains(&name.as_str()) {
                walk(root, &path, paths)?;
            }
        } else {
            paths.insert(path);
        }
    }
    Ok(())
}

/// Resolves `target` relative to the directory containing `from`, removing
/// `.` and `..` components. Returns `None` if it would leave the repository.
pub fn resolve(from: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// The relative path from the directory containing `from` to `to`.
pub fn relative(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = from.split('/').collect();
    let from_dir = &from_dir[..from_dir.len() - 1];
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from_dir
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; from_dir.len() - common];
    parts.extend(&to_parts[common..]);
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(resolve("spec/a.md", "b.md").as_deref(), Some("spec/b.md"));
        assert_eq!(
            resolve("spec/a.md", "../litmus/x/y.accept.lc").as_deref(),
            Some("litmus/x/y.accept.lc")
        );
        assert_eq!(
            resolve("README.md", "./VISION.md").as_deref(),
            Some("VISION.md")
        );
        assert_eq!(resolve("README.md", "../outside.md"), None);
    }

    #[test]
    fn computes_relative_paths() {
        assert_eq!(relative("spec/a.md", "spec/b.md"), "b.md");
        assert_eq!(relative("spec/a.md", "litmus/t.md"), "../litmus/t.md");
        assert_eq!(relative("index.md", "spec/a.md"), "spec/a.md");
        assert_eq!(relative("design/rfcs/x.md", "VISION.md"), "../../VISION.md");
    }

    #[test]
    fn directories_exist_when_they_contain_files() {
        let repo = Repo::from_files(&[("litmus/README.md", ""), ("litmusx", "")]);
        assert!(repo.exists("litmus"));
        assert!(repo.exists("litmus/"));
        assert!(repo.exists("litmus/README.md"));
        assert!(!repo.exists("litmus/missing.md"));
        assert!(!repo.exists("spec"));
    }
}
