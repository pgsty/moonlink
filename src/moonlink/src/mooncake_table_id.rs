use sha2::{Digest, Sha256};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::hash::Hash;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MooncakeTableId {
    pub database: String,
    pub table: String,
}

impl MooncakeTableId {
    pub fn storage_key(&self) -> String {
        let mut digest = Sha256::new();
        digest.update((self.database.len() as u64).to_be_bytes());
        digest.update(self.database.as_bytes());
        digest.update(self.table.as_bytes());
        format!("v2-{:x}", digest.finalize())
    }

    pub fn cache_path(&self, base: &Path) -> PathBuf {
        base.join("temp")
            .join("write_cache")
            .join(self.storage_key())
    }

    pub fn safe_catalog_component(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 255
            && value != "."
            && value != ".."
            && !value
                .chars()
                .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
    }

    pub fn safe_table_name(value: &str) -> bool {
        Self::safe_catalog_component(value) && !value.eq_ignore_ascii_case("indicator.text")
    }

    pub fn safe_namespace(value: &str) -> bool {
        Self::safe_catalog_component(value)
            && !matches!(
                value.to_ascii_lowercase().as_str(),
                "temp"
                    | "initial_copy"
                    | ".moonlink_atomic_tmp"
                    | "read_through_cache"
                    | "_wal"
                    | "source_fingerprints"
                    | "errored_tables"
                    | "moonlink_metadata_store.sqlite"
                    | "moonlink_metadata_store.sqlite-wal"
                    | "moonlink_metadata_store.sqlite-shm"
                    | "moonlink_metadata_store.sqlite-journal"
                    | "moonlink.lock"
                    | "moonlink.sock"
            )
    }
}

impl Display for MooncakeTableId {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}.{}", self.database, self.table)
    }
}
