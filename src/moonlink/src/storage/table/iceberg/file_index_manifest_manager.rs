use crate::storage::table::iceberg::index::{
    MOONCAKE_HASH_INDEX_V1, MOONCAKE_HASH_INDEX_V1_CARDINALITY,
};
use crate::storage::table::iceberg::manifest_utils;
use crate::storage::table::iceberg::manifest_utils::ManifestEntryType;
use crate::storage::table::iceberg::puffin_writer_proxy::PuffinFileMetadata;
use iceberg::puffin::BlobMetadata;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use iceberg::io::FileIO;
use iceberg::spec::{
    DataContentType, DataFile, DataFileBuilder, DataFileFormat, ManifestEntry, ManifestFile,
    ManifestMetadata, ManifestWriter, TableMetadata,
};
use iceberg::Result as IcebergResult;

pub(crate) struct FileIndexManifestManager<'a> {
    table_metadata: &'a TableMetadata,
    file_io: &'a FileIO,
    index_puffin_blobs_to_remove: &'a HashSet<String>,
    writer: Option<ManifestWriter>,
}

impl<'a> FileIndexManifestManager<'a> {
    pub(crate) fn new(
        table_metadata: &'a TableMetadata,
        file_io: &'a FileIO,
        index_puffin_blobs_to_remove: &'a HashSet<String>,
    ) -> FileIndexManifestManager<'a> {
        Self {
            table_metadata,
            file_io,
            index_puffin_blobs_to_remove,
            writer: None,
        }
    }

    fn init_writer_for_once(&mut self) -> IcebergResult<()> {
        if self.writer.is_some() {
            return Ok(());
        }
        let new_writer_builder =
            manifest_utils::create_manifest_writer_builder(self.table_metadata, self.file_io)?;
        let new_writer = new_writer_builder.build_v2_data();
        self.writer = Some(new_writer);
        Ok(())
    }

    pub(crate) fn add_manifest_entries(
        &mut self,
        manifest_entries: Vec<Arc<ManifestEntry>>,
        manifest_metadata: ManifestMetadata,
    ) -> IcebergResult<()> {
        assert_eq!(
            manifest_utils::get_manifest_entry_type(&manifest_entries, &manifest_metadata),
            ManifestEntryType::FileIndex
        );
        for cur_manifest_entry in manifest_entries.into_iter() {
            // Skip file indices which are requested to remove (due to index merge and data file compaction).
            if self
                .index_puffin_blobs_to_remove
                .contains(cur_manifest_entry.data_file().file_path())
            {
                continue;
            }

            // Keep file indices which are not requested to remove.
            self.init_writer_for_once()?;
            self.writer.as_mut().unwrap().add_file(
                cur_manifest_entry.data_file().clone(),
                cur_manifest_entry.sequence_number().unwrap(),
            )?;
        }
        Ok(())
    }

    pub(crate) fn add_new_puffin_blobs(
        &mut self,
        file_index_blobs_to_add: &HashMap<String, PuffinFileMetadata>,
    ) -> IcebergResult<()> {
        for (puffin_filepath, blob_metadata) in file_index_blobs_to_add.iter() {
            for cur_blob_metadata in blob_metadata.blobs.iter() {
                let data_file = get_data_file_for_file_index(
                    puffin_filepath,
                    blob_metadata.size,
                    cur_blob_metadata,
                )?;
                self.init_writer_for_once()?;
                self.writer
                    .as_mut()
                    .unwrap()
                    .add_file(data_file, cur_blob_metadata.sequence_number())?;
            }
        }
        Ok(())
    }

    /// Finalize the current manifest file and return.
    pub(crate) async fn finalize(self) -> IcebergResult<Option<ManifestFile>> {
        if let Some(writer) = self.writer {
            let manifest_file = writer.write_manifest_file().await?;
            return Ok(Some(manifest_file));
        }
        Ok(None)
    }
}

fn get_data_file_for_file_index(
    puffin_filepath: &str,
    size: u64,
    blob_metadata: &BlobMetadata,
) -> IcebergResult<DataFile> {
    if blob_metadata.blob_type() != MOONCAKE_HASH_INDEX_V1 {
        return Err(iceberg::Error::new(
            iceberg::ErrorKind::DataInvalid,
            "unexpected Puffin blob type",
        ));
    }
    let count = blob_metadata
        .properties()
        .get(MOONCAKE_HASH_INDEX_V1_CARDINALITY)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| {
            iceberg::Error::new(
                iceberg::ErrorKind::DataInvalid,
                "invalid Puffin cardinality",
            )
        })?;
    let file = DataFileBuilder::default()
        .content(DataContentType::Data)
        .file_path(puffin_filepath.to_owned())
        .file_format(DataFileFormat::Puffin)
        .record_count(count)
        .file_size_in_bytes(size)
        .build()
        .map_err(|error| iceberg::Error::new(iceberg::ErrorKind::DataInvalid, error.to_string()))?;
    Ok(file)
}
