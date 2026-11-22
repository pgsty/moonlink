/// Manifest manager for deletion vectors, which correspond to one iceberg table, and one table snapshot.
use crate::storage::table::iceberg::deletion_vector::{
    DELETION_VECTOR_CADINALITY, DELETION_VECTOR_REFERENCED_DATA_FILE,
};
use crate::storage::table::iceberg::manifest_utils;
use crate::storage::table::iceberg::manifest_utils::ManifestEntryType;
use crate::storage::table::iceberg::puffin_writer_proxy::PuffinFileMetadata;
use iceberg::puffin::BlobMetadata;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use iceberg::io::FileIO;
use iceberg::puffin::DELETION_VECTOR_V1;
use iceberg::spec::{
    DataContentType, DataFile, DataFileBuilder, DataFileFormat, ManifestEntry, ManifestFile,
    ManifestMetadata, ManifestWriter, TableMetadata,
};
use iceberg::Result as IcebergResult;

pub(crate) struct DeletionVectorManifestManager<'a> {
    table_metadata: &'a TableMetadata,
    file_io: &'a FileIO,
    data_files_to_remove: &'a HashSet<String>,
    writer: Option<ManifestWriter>,
    // Map from referenced data file to deletion vector manifest entry.
    existing_deletion_vector_entries: HashMap<String, Arc<ManifestEntry>>,
}

impl<'a> DeletionVectorManifestManager<'a> {
    pub(crate) fn new(
        table_metadata: &'a TableMetadata,
        file_io: &'a FileIO,
        data_files_to_remove: &'a HashSet<String>,
    ) -> Self {
        DeletionVectorManifestManager {
            table_metadata,
            file_io,
            data_files_to_remove,
            writer: None,
            existing_deletion_vector_entries: HashMap::new(),
        }
    }

    fn init_writer_for_once(&mut self) -> IcebergResult<()> {
        if self.writer.is_some() {
            return Ok(());
        }
        let new_writer_builder =
            manifest_utils::create_manifest_writer_builder(self.table_metadata, self.file_io)?;
        let new_writer = new_writer_builder.build_v2_deletes();
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
            ManifestEntryType::DeletionVector
        );
        for cur_manifest_entry in manifest_entries.into_iter() {
            // Skip deletion vectors which are requested to remove (due to compaction).
            let referenced_data_file = cur_manifest_entry
                .data_file()
                .referenced_data_file()
                .unwrap();
            if self.data_files_to_remove.contains(&referenced_data_file) {
                continue;
            }
            let old_entry = self.existing_deletion_vector_entries.insert(
                cur_manifest_entry
                    .data_file()
                    .referenced_data_file()
                    .unwrap(),
                cur_manifest_entry,
            );
            assert!(
                old_entry.is_none(),
                "Deletion vector for the same data file {:?} appeared for multiple times!",
                old_entry.unwrap().data_file().file_path()
            );
        }
        Ok(())
    }

    pub(crate) fn add_new_puffin_blobs(
        &mut self,
        deletion_vector_blobs_to_add: &HashMap<String, PuffinFileMetadata>,
    ) -> IcebergResult<()> {
        for (puffin_filepath, blob_metadata) in deletion_vector_blobs_to_add.iter() {
            for cur_blob_metadata in blob_metadata.blobs.iter() {
                let (referenced_data_filepath, data_file) = get_data_file_for_deletion_vector(
                    puffin_filepath,
                    blob_metadata.size,
                    cur_blob_metadata,
                )?;
                self.existing_deletion_vector_entries
                    .remove(&referenced_data_filepath);
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
    pub(crate) async fn finalize(mut self) -> IcebergResult<Option<ManifestFile>> {
        if !self.existing_deletion_vector_entries.is_empty() {
            self.init_writer_for_once()?;
        }

        // Add old deletion vector entries which doesn't get overwritten.
        for (_, cur_manifest_entry) in self.existing_deletion_vector_entries.drain() {
            self.writer.as_mut().unwrap().add_file(
                cur_manifest_entry.data_file().clone(),
                cur_manifest_entry.sequence_number().unwrap(),
            )?;
        }

        if let Some(writer) = self.writer {
            let manifest_file = writer.write_manifest_file().await?;
            return Ok(Some(manifest_file));
        }
        Ok(None)
    }
}

fn get_data_file_for_deletion_vector(
    puffin_filepath: &str,
    size: u64,
    blob_metadata: &BlobMetadata,
) -> IcebergResult<(String, DataFile)> {
    if blob_metadata.blob_type() != DELETION_VECTOR_V1 {
        return Err(iceberg::Error::new(
            iceberg::ErrorKind::DataInvalid,
            "unexpected Puffin blob type",
        ));
    }
    let referenced = blob_metadata
        .properties()
        .get(DELETION_VECTOR_REFERENCED_DATA_FILE)
        .ok_or_else(|| {
            iceberg::Error::new(
                iceberg::ErrorKind::DataInvalid,
                "missing referenced data file",
            )
        })?;
    let count = blob_metadata
        .properties()
        .get(DELETION_VECTOR_CADINALITY)
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| {
            iceberg::Error::new(
                iceberg::ErrorKind::DataInvalid,
                "invalid Puffin cardinality",
            )
        })?;
    let file = DataFileBuilder::default()
        .content(DataContentType::PositionDeletes)
        .file_path(puffin_filepath.to_owned())
        .file_format(DataFileFormat::Puffin)
        .record_count(count)
        .file_size_in_bytes(size)
        .referenced_data_file(Some(referenced.clone()))
        .content_offset(Some(i64::try_from(blob_metadata.offset())?))
        .content_size_in_bytes(Some(i64::try_from(blob_metadata.length())?))
        .build()
        .map_err(|error| iceberg::Error::new(iceberg::ErrorKind::DataInvalid, error.to_string()))?;
    Ok((referenced.clone(), file))
}
