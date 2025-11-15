use crate::api::FileMeta;
use fxhash::{FxHashMap, FxHashSet};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::{fs, io};

fn must_remove<'a>(
    local_files: &'a FxHashMap<String, PathBuf>,
    remote_files: &'a FxHashMap<String, FileMeta>,
    ignored_prefix: &[String],
) -> FxHashSet<&'a str> {
    remote_files
        .keys()
        .filter(|p| !local_files.contains_key(p.as_str()))
        .filter(|p| !ignored_prefix.iter().any(|prefix| p.starts_with(prefix)))
        .map(|s| s.as_str())
        .collect()
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SyncPlan {
    Put {
        local: PathBuf,
        remote: String,
    },
    Replace {
        local: PathBuf,
        remote: String,
        remote_checksum: Option<[u8; 32]>,
    },
    Delete {
        remote: String,
    },
}

#[cfg(test)]
impl SyncPlan {
    fn remote(&self) -> &str {
        match self {
            SyncPlan::Put { local: _, remote } => remote.as_str(),
            SyncPlan::Replace {
                local: _,
                remote,
                remote_checksum: _,
            } => remote.as_str(),
            SyncPlan::Delete { remote } => remote.as_str(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SyncAction {
    Put {
        content: Vec<u8>,
        mime_type: Option<&'static str>,
    },
    Ignore,
    Delete,
}

pub struct Execution<'a> {
    pub remote: &'a str,
    pub action: SyncAction,
}

pub fn plan_sync<'a>(
    local: &'a FxHashMap<String, PathBuf>,
    remote_content: &'a FxHashMap<String, FileMeta>,
    ignore: &[String],
) -> Vec<SyncPlan> {
    let mut job = Vec::with_capacity(local.len());
    let mut local_paths_ordered: Vec<_> = local.keys().map(|path| path.as_str()).collect();
    local_paths_ordered
        .sort_by_key(|path| (path.ends_with(".html") || path.ends_with(".htm"), *path));

    for remote_path in local_paths_ordered {
        // safe; this is the key of local
        let physical_path = local.get(remote_path).unwrap();
        if let Some(on_remote) = remote_content.get(remote_path) {
            job.push(SyncPlan::Replace {
                local: physical_path.to_owned(),
                remote: remote_path.to_owned(),
                remote_checksum: on_remote.checksum,
            });
        } else {
            job.push(SyncPlan::Put {
                local: physical_path.to_owned(),
                remote: remote_path.to_owned(),
            });
        }
    }
    job.extend(
        must_remove(local, remote_content, ignore)
            .into_iter()
            .map(|remote| SyncPlan::Delete {
                remote: remote.to_owned(),
            }),
    );
    job
}

pub fn plan_execution<'a, F>(plan: &'a SyncPlan, read: F) -> anyhow::Result<Execution<'a>>
where
    F: Fn(&'a PathBuf) -> io::Result<Vec<u8>>,
{
    match plan {
        SyncPlan::Put { local, remote } => {
            let content = fs::read(local)?;
            let mime_type = infer::get_from_path(local)?.map(|t| t.mime_type());
            Ok(Execution {
                remote,
                action: SyncAction::Put { content, mime_type },
            })
        }
        SyncPlan::Replace {
            local,
            remote,
            remote_checksum,
        } => {
            let content = read(local)?;
            let mime_type = infer::get_from_path(local)?.map(|t| t.mime_type());
            let digest: [u8; 32] = Sha256::digest(&content).into();
            if &Some(digest) != remote_checksum {
                Ok(Execution {
                    remote,
                    action: SyncAction::Put { content, mime_type },
                })
            } else {
                Ok(Execution {
                    remote,
                    action: SyncAction::Ignore,
                })
            }
        }
        SyncPlan::Delete { remote } => Ok(Execution {
            remote,
            action: SyncAction::Delete,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{Execution, SyncAction, SyncPlan, plan_execution, plan_sync};
    use crate::api::FileMeta;
    use fxhash::FxHashMap;
    use sha2::{Digest, Sha256};
    use std::path::PathBuf;

    #[test]
    fn replaces_when_checksum_mismatch() {
        let content_remote = "hei";
        let remote_checksum: [u8; 32] = Sha256::digest(content_remote.as_bytes()).into();
        let local_content = "hallois";
        let local = PathBuf::new().join("README.md");
        let plan = SyncPlan::Replace {
            local,
            remote: "remote".to_string(),
            remote_checksum: Some(remote_checksum),
        };
        let Execution { remote: _, action } =
            plan_execution(&plan, |_| Ok(local_content.as_bytes().to_vec())).unwrap();
        assert_eq!(
            action,
            SyncAction::Put {
                content: local_content.as_bytes().to_vec(),
                mime_type: None
            }
        );
    }

    #[test]
    fn ignores_when_checksum_match() {
        let content_remote = "hei";
        let remote_checksum: [u8; 32] = Sha256::digest(content_remote.as_bytes()).into();
        let local_content = "hei";
        let local = PathBuf::new().join("README.md");
        let plan = SyncPlan::Replace {
            local,
            remote: "remote".to_string(),
            remote_checksum: Some(remote_checksum),
        };
        let Execution { remote: _, action } =
            plan_execution(&plan, |_| Ok(local_content.as_bytes().to_vec())).unwrap();
        assert_eq!(action, SyncAction::Ignore);
    }

    #[test]
    fn deletes_everything_with_empty_local() {
        let local = FxHashMap::default();
        let mut remote = FxHashMap::default();
        remote.insert("subfolder/index.html".into(), FileMeta { checksum: None });
        let job = plan_sync(&local, &remote, &[]);
        assert_eq!(
            job,
            vec![SyncPlan::Delete {
                remote: "subfolder/index.html".to_string()
            }]
        );
    }

    #[test]
    fn skips_deleting_ignored_prefixes() {
        let local = FxHashMap::default();
        let mut remote = FxHashMap::default();
        remote.insert("subfolder/index.html".into(), FileMeta { checksum: None });
        remote.insert(
            "other_subfolder/index.html".into(),
            FileMeta { checksum: None },
        );
        let job = plan_sync(&local, &remote, &["other_subfolder".into()]);
        assert_eq!(
            job,
            vec![SyncPlan::Delete {
                remote: "subfolder/index.html".to_string()
            }]
        );
    }

    #[test]
    fn syncs_missing_files() {
        let mut local = FxHashMap::default();
        local.insert("subfolder/index.html".into(), PathBuf::new());
        let remote = FxHashMap::default();
        let job = plan_sync(&local, &remote, &[]);
        assert_eq!(
            job,
            vec![SyncPlan::Put {
                remote: "subfolder/index.html".to_string(),
                local: PathBuf::new()
            }]
        );
    }

    #[test]
    fn compares_files_in_both() {
        let mut local = FxHashMap::default();
        local.insert("subfolder/index.html".into(), PathBuf::new());
        let mut remote = FxHashMap::default();
        remote.insert("subfolder/index.html".into(), FileMeta { checksum: None });
        let job = plan_sync(&local, &remote, &[]);
        assert_eq!(
            job,
            vec![SyncPlan::Replace {
                remote: "subfolder/index.html".to_string(),
                local: PathBuf::new(),
                remote_checksum: None
            }]
        );
    }

    #[test]
    fn sorts_html_files_last() {
        let mut local = FxHashMap::default();
        local.insert("z.txt".into(), PathBuf::new());
        local.insert("a.html".into(), PathBuf::new());
        local.insert("b.htm".into(), PathBuf::new());
        local.insert("c.jpg".into(), PathBuf::new());

        let remote = FxHashMap::default();
        let job = plan_sync(&local, &remote, &[]);

        // HTML files should be at the end
        assert_eq!(job[0].remote(), "c.jpg");
        assert_eq!(job[1].remote(), "z.txt");
        // Then HTML files
        assert!(job[2].remote() == "a.html" || job[2].remote() == "b.htm");
        assert!(job[3].remote() == "a.html" || job[3].remote() == "b.htm");
    }

    #[test]
    fn replaces_when_remote_checksum_is_none() {
        let local_content = "content";
        let local = PathBuf::new().join("README.md");
        let plan = SyncPlan::Replace {
            local,
            remote: "remote".to_string(),
            remote_checksum: None,
        };
        let execution = plan_execution(&plan, |_| Ok(local_content.as_bytes().to_vec())).unwrap();
        assert_eq!(
            execution.action,
            SyncAction::Put {
                content: local_content.as_bytes().to_vec(),
                mime_type: None
            }
        );
    }

    #[test]
    fn test_must_remove() {
        let mut local = FxHashMap::default();
        local.insert("file1.txt".into(), PathBuf::new());
        local.insert("file2.txt".into(), PathBuf::new());

        let mut remote = FxHashMap::default();
        remote.insert("file1.txt".into(), FileMeta { checksum: None });
        remote.insert("file3.txt".into(), FileMeta { checksum: None });
        remote.insert("ignored/file4.txt".into(), FileMeta { checksum: None });

        let to_remove = super::must_remove(&local, &remote, &["ignored".to_string()]);

        assert_eq!(to_remove.len(), 1);
        assert!(to_remove.contains("file3.txt"));
    }
}
