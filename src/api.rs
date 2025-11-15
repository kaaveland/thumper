use anyhow::anyhow;
use crossbeam::channel::unbounded;
use fxhash::FxHashMap;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::thread;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FileInfo {
    pub path: String,
    pub object_name: String,
    pub checksum: Option<String>,
    pub is_directory: bool,
}

#[derive(Debug)]
pub struct FileMeta {
    pub checksum: Option<[u8; 32]>,
}

#[derive(Clone)]
pub struct StorageZoneClient {
    client: Client,
    access_key: String,
    endpoint: String,
    storage_zone: String,
}

impl StorageZoneClient {
    pub fn new(access_key: String, endpoint: String, storage_zone: String) -> Self {
        StorageZoneClient {
            client: Client::new(),
            access_key,
            endpoint,
            storage_zone,
        }
    }

    pub fn read_file(&self, path: &str) -> anyhow::Result<String> {
        let response = self
            .client
            .get(self.url_for(path))
            .header("AccessKey", self.access_key.as_str())
            .send()?;
        if response.status().is_success() {
            Ok(response.text()?)
        } else {
            Err(anyhow!("Unable to read: {:?}", response.status()))
        }
    }

    fn url_for(&self, path: &str) -> String {
        format!("https://{}/{}/{path}", self.endpoint, self.storage_zone)
    }

    fn ls_dir(&self, path: &str) -> anyhow::Result<Vec<FileInfo>> {
        let response = self
            .client
            .get(self.url_for(path))
            .header("AccessKey", self.access_key.as_str())
            .send()?;
        Ok(response.json()?)
    }

    fn concurrent_discover_files(
        &self,
        path: &str,
        skip: &[String],
        concurrency: usize,
    ) -> anyhow::Result<Vec<FileInfo>> {
        let (post_work, receive_work) = unbounded();
        let (post_result, receive_result) = unbounded();

        post_work.send(path.to_string())?;

        thread::scope(|scope| {
            let mut files = vec![];

            // Spawn workers
            let mut workers = Vec::with_capacity(concurrency);
            for _ in 0..concurrency {
                let receive_work = receive_work.clone();
                let send_result = post_result.clone();
                workers.push(scope.spawn(move || {
                    while let Ok(path) = receive_work.recv() {
                        send_result.send(self.ls_dir(path.as_str()))?;
                    }
                    // Channel closed
                    Ok::<(), anyhow::Error>(())
                }));
            }

            let global_prefix = format!("/{}/", self.storage_zone);
            let mut responses_needed = 1;

            while responses_needed > 0 {
                let new = receive_result.recv()??;
                responses_needed -= 1;
                for child in new {
                    if child.is_directory {
                        let subtree = format!(
                            "{}/{}/",
                            child
                                .path
                                .trim_start_matches(global_prefix.as_str())
                                .trim_end_matches('/'),
                            child.object_name.as_str()
                        );
                        if skip.iter().any(|skip| subtree.starts_with(skip)) {
                            continue;
                        }
                        responses_needed += 1;
                        post_work.send(subtree)?;
                    } else {
                        files.push(child);
                    }
                }
            }
            // Close channel to shut down workers
            drop(post_work);
            Ok::<Vec<_>, anyhow::Error>(files)
        })
    }

    pub fn list_files(
        &self,
        path: &str,
        skip: &[String],
        concurrency: usize,
    ) -> anyhow::Result<FxHashMap<String, FileMeta>> {
        let files = self.concurrent_discover_files(path, skip, concurrency)?;
        let mut files_by_name = FxHashMap::default();
        let trim_prefix = format!("/{}/", self.storage_zone);
        for fi in files {
            let checksum = fi
                .checksum
                .map(|hex_checksum| {
                    let mut checksum = [0; 32];
                    hex::decode_to_slice(hex_checksum.as_bytes(), &mut checksum)?;
                    Ok::<[u8; 32], anyhow::Error>(checksum)
                })
                .transpose()?;
            files_by_name.insert(
                format!(
                    "{}{}",
                    fi.path.trim_start_matches(trim_prefix.as_str()),
                    fi.object_name
                ),
                FileMeta { checksum },
            );
        }
        Ok(files_by_name)
    }

    pub fn put_file(
        &self,
        path: &str,
        body: Vec<u8>,
        content_type: Option<&str>,
    ) -> anyhow::Result<()> {
        let url = self.url_for(path);

        let response = self
            .client
            .put(url)
            .header("AccessKey", self.access_key.as_str())
            .header(
                "Content-Type",
                content_type.unwrap_or("application/octet-stream"),
            )
            .body(body)
            .send()?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("Request failed: {:?}", response.status()))
        }
    }

    pub fn delete_file(&self, path: &str) -> anyhow::Result<()> {
        let response = self
            .client
            .delete(self.url_for(path))
            .header("AccessKey", self.access_key.as_str())
            .send()?;
        Ok(response.error_for_status().map(|_| ())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EX: &str = "{
    \"StorageZoneName\": \"eugene-docs\",
    \"Path\": \"/eugene-docs/\",
    \"ObjectName\": \"404.html\",
    \"Length\": 9665,
    \"LastChanged\": \"2025-04-15T16:52:33.824\",
    \"ArrayNumber\": 2,
    \"IsDirectory\": false,
    \"ContentType\": \"\",
    \"DateCreated\": \"2025-04-15T16:52:33.824\",
    \"Checksum\": \"FD9495967478FCD8B9FB08F70EAF2806BD50F4AB2261BE16A9BEAA542C37A441\",
    \"ReplicatedZones\": \"SE,UK,LA,SG,BR,NY\"
  }";

    #[test]
    fn test_parse() {
        let _: FileInfo = serde_json::from_str(EX).unwrap();
    }
}
