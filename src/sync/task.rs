use std::io;
use std::{fs, path::PathBuf};
use sha2::{Digest, Sha256};

use crate::{api::StorageZoneClient, sync::plan::{Action, Execution}};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Task {
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

impl Task {
    pub fn execute(
        &self,
        client: &StorageZoneClient,
        dry_run: bool,
        lockfile: &str,
    ) -> anyhow::Result<(String, &'static str)> {
        let Execution { remote, action } = self.plan(fs::read)?;

        let event = match &action {
            Action::Put { .. } => "put",
            Action::Ignore => "unchanged",
            Action::Delete => "delete",
        };

        if !dry_run {
            match action {
                Action::Put { content, mime_type } => {
                    client.put_file(remote, content, mime_type)?;
                }
                Action::Delete if remote != lockfile => {
                    client.delete_file(remote)?;
                }
                _ => {}
            }
        }

        Ok((remote.to_string(), event))
    }


    pub fn plan<'a, F>(&'a self, read: F) -> anyhow::Result<Execution<'a>>
    where
        F: Fn(&'a PathBuf) -> io::Result<Vec<u8>>,
    {
        match self {
            Task::Put { local, remote } => {
                let content = fs::read(local)?;
                let mime_type = infer::get_from_path(local)?.map(|t| t.mime_type());
                Ok(Execution {
                    remote,
                    action: Action::Put { content, mime_type },
                })
            }
            Task::Replace {
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
                        action: Action::Put { content, mime_type },
                    })
                } else {
                    Ok(Execution {
                        remote,
                        action: Action::Ignore,
                    })
                }
            }
            Task::Delete { remote } => Ok(Execution {
                remote,
                action: Action::Delete,
            }),
        }
    }

}

#[cfg(test)]
impl Task {
    pub fn remote(&self) -> &str {
        match self {
            Task::Put { local: _, remote } => &remote,
            Task::Replace {
                local: _,
                remote,
                remote_checksum: _,
            } => &remote,
            Task::Delete { remote } => &remote,
        }
    }
}
