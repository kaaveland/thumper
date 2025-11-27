use anyhow::{Context, anyhow};
use fxhash::{FxHashMap, FxHashSet};
use std::fs;
use std::path::PathBuf;

pub fn files_by_remote_name(
    root: &str,
    remote_root: &str,
) -> anyhow::Result<FxHashMap<String, PathBuf>> {
    let files = discover_files(root)?;
    let remote_root = remote_root.trim_start_matches("/").trim_end_matches("/");
    let mut by_name = FxHashMap::default();
    for file in files {
        let remote_name = file
            .strip_prefix(root)?
            .to_str()
            .context("Invalid utf8")?
            .to_owned();
        if remote_root.is_empty() {
            by_name.insert(remote_name, file);
        } else {
            by_name.insert(format!("{remote_root}/{remote_name}"), file);
        }
    }
    Ok(by_name)
}

fn discover_files(root: &str) -> anyhow::Result<FxHashSet<PathBuf>> {
    let root_path = PathBuf::from(root);
    let mut files = FxHashSet::default();

    if root_path.is_dir() {
        for entry in fs::read_dir(&root_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                files.extend(discover_files(path.to_str().context("Invalid utf8")?)?);
            } else if path.is_file() && !path.is_symlink() {
                files.insert(path);
            }
        }
        Ok(files)
    } else {
        Err(anyhow!("{root} is not a directory"))
    }
}

pub fn normalize_path(path: &str) -> String {
    if path.ends_with("/") {
        path.to_owned()
    } else {
        let mut path = path.to_owned();
        path.push('/');
        path
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_by_remote_name_smoketest() {
        let files = files_by_remote_name("src", "sources").unwrap();
        assert_eq!(
            files.get("sources/main.rs"),
            Some(&PathBuf::new().join("src").join("main.rs"))
        );
    }
}
