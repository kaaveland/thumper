use std::{thread};

use crossbeam::channel::unbounded;

use crate::api::StorageZoneClient;
use crate::lock::Lock;
use crate::sync::local_path::{files_by_remote_name, normalize_path};
use crate::sync::plan::{plan_sync};

mod local_path;
mod plan;
mod task;

pub struct SyncJob {
    client: StorageZoneClient,
    remote_path: String,
    local_path: String,
    force: bool,
    dry_run: bool,
    verbose: bool,
    lockfile: String,
    ignore: Vec<String>,
    concurrency: usize,
}

impl SyncJob {
    pub fn new(
        api_key: &str,
        endpoint: &str,
        storage_zone: &str,
        local_path: &str,
        remote_path: &str,
        lockfile: &str,
        force: bool,
        dry_run: bool,
        verbose: bool,
        ignore: Vec<String>,
        concurrency: Option<usize>,
    ) -> anyhow::Result<Self> {
        let client = StorageZoneClient::new(api_key, endpoint, storage_zone);

        let concurrency = concurrency.unwrap_or_else(num_cpus::get);

        Ok(SyncJob {
            client,
            remote_path: normalize_path(remote_path),
            local_path: normalize_path(local_path),
            lockfile: lockfile.to_owned(),
            force,
            dry_run,
            verbose,
            ignore,
            concurrency
        })
    }

    pub fn execute(&self) -> anyhow::Result<()> {
        #[allow(unused_variables)]
        let lock = if !self.dry_run {
            Some(Lock::new(&self.client, &self.lockfile, self.force)?)
        } else {
            None
        };
        
        let local = files_by_remote_name(&self.local_path, &self.remote_path)?;
        let remote = self.client.list_files(&self.remote_path, &self.ignore, self.concurrency)?;
        let tasks = plan_sync(&local, &remote, &self.ignore);

        let (send_work, receive_work) = unbounded();
        let (send_result, receive_result) = unbounded();
        let expected = tasks.len();

        thread::scope(move |scope| {
            for task in tasks {
                send_work.send(task)?;
            }

            for _ in 0..self.concurrency {
                let receive_work = receive_work.clone();
                let send_result = send_result.clone();

                scope.spawn(move || {
                    while let Ok(task) = receive_work.recv() {
                        let r = task.execute(&self.client, self.dry_run, &self.lockfile);
                        send_result.send(r)?;
                    }
                    Ok::<(), anyhow::Error>(())
                });
            }

            for _ in 0..expected {
                let (remote, event) = receive_result.recv()??;
                if self.verbose || self.dry_run {
                    println!("{remote}: {event}");
                }
            }

            drop(send_work);

            Ok::<_, anyhow::Error>(())
        })
    }
}

