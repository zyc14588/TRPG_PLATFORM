use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use trpg_security_governance::secret::{
    LedgerCheckpoint, LedgerCheckpointStore, LEDGER_CHECKPOINT_GENESIS_HASH,
};
use trpg_shared_kernel::{KernelResult, TrpgError};

#[derive(Clone)]
pub struct TestFileCheckpointStore {
    path: PathBuf,
}

#[derive(Deserialize, Serialize)]
struct StoredCheckpoint {
    ledger_id: String,
    sequence: u64,
    previous_chain_head: String,
    chain_head: String,
    integrity_key_id: String,
    checkpoint_mac: String,
}

impl TestFileCheckpointStore {
    pub fn shared(path: impl AsRef<Path>) -> Arc<dyn LedgerCheckpointStore> {
        Arc::new(Self {
            path: path.as_ref().to_path_buf(),
        })
    }

    fn open(&self) -> KernelResult<File> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        options
            .open(&self.path)
            .map_err(|_| TrpgError::AuditIntegrityViolation)
    }

    fn records(file: &mut File) -> KernelResult<Vec<StoredCheckpoint>> {
        file.seek(SeekFrom::Start(0))
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let mut encoded = String::new();
        file.read_to_string(&mut encoded)
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        encoded
            .lines()
            .map(|line| serde_json::from_str(line).map_err(|_| TrpgError::AuditIntegrityViolation))
            .collect()
    }
}

impl LedgerCheckpointStore for TestFileCheckpointStore {
    fn latest(&self, ledger_id: &str) -> KernelResult<Option<LedgerCheckpoint>> {
        let mut file = self.open()?;
        file.lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let latest = Self::records(&mut file)?
            .into_iter()
            .rfind(|record| record.ledger_id == ledger_id);
        let result = latest
            .map(|record| {
                LedgerCheckpoint::new(
                    record.sequence,
                    record.previous_chain_head,
                    record.chain_head,
                    record.integrity_key_id,
                    record.checkpoint_mac,
                )
            })
            .transpose();
        let _ = file.unlock();
        result
    }

    fn append(&self, ledger_id: &str, checkpoint: &LedgerCheckpoint) -> KernelResult<()> {
        let mut file = self.open()?;
        file.lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let records = Self::records(&mut file)?;
        let latest = records.iter().rfind(|record| record.ledger_id == ledger_id);
        if latest.is_some_and(|record| {
            record.sequence == checkpoint.sequence()
                && record.previous_chain_head == checkpoint.previous_chain_head()
                && record.chain_head == checkpoint.chain_head()
                && record.integrity_key_id == checkpoint.integrity_key_id()
                && record.checkpoint_mac == checkpoint.checkpoint_mac()
        }) {
            let _ = file.unlock();
            return Ok(());
        }
        let valid_predecessor = latest.map_or(
            checkpoint.sequence() == 1
                && checkpoint.previous_chain_head() == LEDGER_CHECKPOINT_GENESIS_HASH,
            |record| {
                record
                    .sequence
                    .checked_add(1)
                    .is_some_and(|sequence| sequence == checkpoint.sequence())
                    && record.chain_head == checkpoint.previous_chain_head()
            },
        );
        if !valid_predecessor {
            let _ = file.unlock();
            return Err(TrpgError::AuditIntegrityViolation);
        }
        let stored = StoredCheckpoint {
            ledger_id: ledger_id.to_owned(),
            sequence: checkpoint.sequence(),
            previous_chain_head: checkpoint.previous_chain_head().to_owned(),
            chain_head: checkpoint.chain_head().to_owned(),
            integrity_key_id: checkpoint.integrity_key_id().to_owned(),
            checkpoint_mac: checkpoint.checkpoint_mac().to_owned(),
        };
        file.seek(SeekFrom::End(0))
            .and_then(|_| {
                serde_json::to_writer(&mut file, &stored).map_err(std::io::Error::other)?;
                file.write_all(b"\n")
            })
            .and_then(|()| file.sync_all())
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let _ = file.unlock();
        Ok(())
    }
}
