use crate::api::StorageZoneClient;
use anyhow::anyhow;
use chrono::Local;

pub struct Lock<'a> {
    client: &'a StorageZoneClient,

    lockfile: String,
}

impl<'a> Lock<'a> {
    pub fn new(client: &'a StorageZoneClient, lockfile: &str, force: bool) -> anyhow::Result<Self> {
        if let Ok(sync_time) = client.read_file(lockfile) {
            eprintln!("WARNING: Remote is locked since {sync_time}");
            if !force {
                return Err(anyhow!("Dangling lock in {lockfile} prevents sync"));
            }
        }
        let now = Local::now();
        let ts = now.to_rfc3339();

        client.put_file(lockfile, ts.bytes().collect(), Some("text/plain"))?;

        Ok(Lock {
            client,
            lockfile: lockfile.to_owned(),
        })
    }
}

impl<'a> Drop for Lock<'a> {
    fn drop(&mut self) {
        match self.client.delete_file(&self.lockfile) {
            Ok(_) => (),
            Err(e) => eprintln!("WARNING: Unable to remove lockfile: {}", e),
        }
    }
}

