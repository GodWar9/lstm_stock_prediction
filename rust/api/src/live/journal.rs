use super::Failure;
use chrono::Utc;
use quant_data::capture::{checksum, SCHEMA_VERSION};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{
    io::AsyncWriteExt,
    time::{timeout, Instant},
};

pub(super) struct Journal {
    file: tokio::fs::File,
    dir: PathBuf,
    _lock: fs::File,
    pub relative: String,
    pub bytes: u64,
    pub durable_seq: u64,
    seq: u64,
    chain: String,
    session_id: String,
    last_received: i64,
    segment: u64,
    segment_bytes: u64,
    segment_limit: u64,
    total_limit: u64,
    total_bytes: u64,
    last_sync: Instant,
}

fn failure() -> Failure {
    Failure::Fatal(
        "Live journal I/O failed, timed out, or storage limit reached; ingestion stopped".into(),
    )
}

impl Journal {
    pub async fn create(root: &Path, feed: &str, symbols: &[String]) -> Result<Self, Failure> {
        Self::with_limits(
            root,
            feed,
            symbols,
            16 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
        )
        .await
    }
    async fn with_limits(
        root: &Path,
        feed: &str,
        symbols: &[String],
        segment_limit: u64,
        total_limit: u64,
    ) -> Result<Self, Failure> {
        let directory = root.join("datasets/live");
        fs::create_dir_all(&directory).map_err(|_| failure())?;
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(directory.join("capture.lock"))
            .map_err(|_| failure())?;
        lock.try_lock().map_err(|_| {
            Failure::Fatal("Another ingestion process holds this workspace capture lock".into())
        })?;
        let mut total_bytes = 0;
        let mut count = 0;
        for entry in fs::read_dir(&directory).map_err(|_| failure())? {
            let entry = entry.map_err(|_| failure())?;
            let kind = entry.file_type().map_err(|_| failure())?;
            if kind.is_symlink() {
                return Err(failure());
            }
            if kind.is_dir() {
                for segment in fs::read_dir(entry.path()).map_err(|_| failure())? {
                    let segment = segment.map_err(|_| failure())?;
                    if !segment.file_type().map_err(|_| failure())?.is_file() {
                        return Err(failure());
                    }
                    total_bytes += segment.metadata().map_err(|_| failure())?.len();
                    count += 1;
                    if count > 100_000 {
                        return Err(failure());
                    }
                }
            } else {
                total_bytes += entry.metadata().map_err(|_| failure())?.len();
            }
        }
        if total_bytes >= total_limit {
            return Err(failure());
        }
        let session_id = uuid::Uuid::new_v4().to_string();
        let relative = format!("datasets/live/{session_id}");
        let dir = root.join(&relative);
        fs::create_dir(&dir).map_err(|_| failure())?;
        let file = tokio::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(dir.join("000001.ndjson"))
            .await
            .map_err(|_| failure())?;
        let mut journal = Self {
            file,
            dir,
            _lock: lock,
            relative,
            bytes: 0,
            durable_seq: 0,
            seq: 0,
            chain: String::new(),
            session_id,
            last_received: 0,
            segment: 1,
            segment_bytes: 0,
            segment_limit,
            total_limit,
            total_bytes,
            last_sync: Instant::now(),
        };
        journal
            .append(&json!({"type":"session","feed":feed,"symbols":symbols}))
            .await?;
        journal.sync().await?;
        Ok(journal)
    }
    pub async fn append(&mut self, value: &Value) -> Result<(), Failure> {
        let mut value = value.clone();
        let received = value["received_at_ms"]
            .as_i64()
            .unwrap_or_else(|| Utc::now().timestamp_millis());
        if received < self.last_received {
            return Err(Failure::Fatal(
                "Receipt clock moved backwards; ingestion stopped".into(),
            ));
        }
        value["schema_version"] = json!(SCHEMA_VERSION);
        value["seq"] = json!(self.seq + 1);
        value["session_id"] = json!(self.session_id);
        value["received_at_ms"] = json!(received);
        value["previous_checksum"] = json!(self.chain);
        let digest = checksum(&value);
        value["checksum"] = json!(digest);
        let mut bytes = serde_json::to_vec(&value).map_err(|_| failure())?;
        bytes.push(b'\n');
        let n = bytes.len() as u64;
        if n > quant_data::capture::MAX_RECORD_BYTES || self.total_bytes + n > self.total_limit {
            return Err(failure());
        }
        if self.segment_bytes > 0 && self.segment_bytes + n > self.segment_limit {
            self.sync().await?;
            self.segment += 1;
            self.file = timeout(
                Duration::from_secs(10),
                tokio::fs::OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(self.dir.join(format!("{:06}.ndjson", self.segment))),
            )
            .await
            .map_err(|_| failure())?
            .map_err(|_| failure())?;
            self.segment_bytes = 0;
        }
        timeout(Duration::from_secs(10), self.file.write_all(&bytes))
            .await
            .map_err(|_| failure())?
            .map_err(|_| failure())?;
        self.seq += 1;
        self.chain = digest;
        self.last_received = received;
        self.bytes += n;
        self.total_bytes += n;
        self.segment_bytes += n;
        if self.last_sync.elapsed() >= Duration::from_secs(1) {
            self.sync().await?;
        }
        Ok(())
    }
    pub async fn sync(&mut self) -> Result<(), Failure> {
        timeout(Duration::from_secs(10), async {
            self.file.flush().await?;
            self.file.sync_all().await
        })
        .await
        .map_err(|_| failure())?
        .map_err(|_| failure())?;
        self.durable_seq = self.seq;
        self.last_sync = Instant::now();
        Ok(())
    }
    pub async fn finish(&mut self, reason: &str) -> Result<(), Failure> {
        self.append(&json!({"type":"stop","reason":reason})).await?;
        self.sync().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn rotation_chain_durability_lock_and_clean_shutdown() {
        let tmp = tempfile::tempdir().unwrap();
        let mut journal = Journal::with_limits(tmp.path(), "iex", &["AAPL".into()], 500, 100_000)
            .await
            .unwrap();
        assert!(Journal::create(tmp.path(), "iex", &["AAPL".into()])
            .await
            .is_err());
        journal.append(&json!({"type":"gap"})).await.unwrap();
        journal.finish("shutdown").await.unwrap();
        assert_eq!(journal.durable_seq, 3);
        let path = tmp.path().join(&journal.relative);
        let audit = quant_data::capture::verify(&path).unwrap();
        assert!(audit.clean_shutdown);
        assert_eq!(audit.gaps, 1);
        assert!(fs::read_dir(&path).unwrap().count() > 1);
        drop(journal);
        assert!(Journal::create(tmp.path(), "iex", &["AAPL".into()])
            .await
            .is_ok());
    }
    #[tokio::test]
    async fn capacity_and_backward_clock_fail_closed() {
        let tmp = tempfile::tempdir().unwrap();
        let mut journal = Journal::with_limits(tmp.path(), "iex", &["AAPL".into()], 500, 500)
            .await
            .unwrap();
        assert!(journal
            .append(&json!({"type":"gap","received_at_ms":1}))
            .await
            .is_err());
        assert!(journal
            .append(&json!({"type":"gap","padding":"x".repeat(600)}))
            .await
            .is_err());
        let path = tmp.path().join(&journal.relative);
        journal.sync().await.unwrap();
        assert!(!quant_data::capture::verify(&path).unwrap().clean_shutdown);
    }
}
