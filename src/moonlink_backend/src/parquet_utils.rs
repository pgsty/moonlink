use crate::error::{Error, Result};

use std::io::SeekFrom;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;

/// Parquet file footer size.
const FOOTER_SIZE: u64 = 8;
/// Parquet file magic bytes ("PAR1").
const PARQUET_MAGIC: &[u8; 4] = b"PAR1";

/// Get serialized uncompressed parquet metadata from the given local filepath.
/// TODO(hjiang): Currently it only supports local filepath.
pub(crate) async fn get_parquet_serialized_metadata(filepath: &str) -> Result<Vec<u8>> {
    let mut file = tokio::fs::File::open(&filepath)
        .await
        .map_err(|e| Error::io(format!("Failed to open file {filepath} with error {e:?}")))?;

    // Validate file size.
    let file_len = file.metadata().await?.len();
    if file_len < FOOTER_SIZE {
        return Err(Error::invalid_argument(format!(
            "File {filepath} is too small to be parquet"
        )));
    }

    // Read last 8 bytes (metadata length + magic bytes).
    file.seek(SeekFrom::End(-(FOOTER_SIZE as i64))).await?;
    let mut footer = [0u8; FOOTER_SIZE as usize];
    file.read_exact(&mut footer).await?;

    // Validate magic bytes.
    if &footer[4..] != PARQUET_MAGIC {
        return Err(Error::data_corruption(format!(
            "File {filepath} magic bytes are corrupted"
        )));
    }

    // Parse metadata length.
    let metadata_len = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]) as u64;

    // File metadata length validation.
    if metadata_len + FOOTER_SIZE > file_len {
        return Err(Error::data_corruption(format!(
            "File {filepath} metadata length is {metadata_len}, file size is {file_len}"
        )));
    }

    // Seek to metadata start and read.
    let metadata_start = file_len - FOOTER_SIZE - metadata_len;
    file.seek(SeekFrom::Start(metadata_start)).await?;

    let mut buf = vec![0u8; metadata_len as usize];
    file.read_exact(&mut buf).await?;

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File as StdFile;

    use arrow_array::{Int32Array, RecordBatch};
    use arrow_schema::{DataType, Field, Schema};
    use parquet::arrow::arrow_writer::ArrowWriter;
    use parquet::file::metadata::ParquetMetaDataReader;
    use parquet::file::statistics::Statistics;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_get_parquet_serialized_metadata_basic_stats() {
        let schema = Schema::new(vec![Field::new("x", DataType::Int32, true)]);
        let data = Int32Array::from(vec![Some(1), Some(2), Some(2), Some(5), None]);
        let batch = RecordBatch::try_new(
            std::sync::Arc::new(schema.clone()),
            vec![std::sync::Arc::new(data)],
        )
        .unwrap();

        let tmp_dir = tempdir().unwrap();
        let parquet_path = format!("{}/test.parquet", tmp_dir.path().to_str().unwrap());

        {
            let file = StdFile::create(&parquet_path).unwrap();
            let mut writer =
                ArrowWriter::try_new(file, std::sync::Arc::new(schema), /*prop=*/ None).unwrap();
            writer.write(&batch).unwrap();
            let _file_metadata = writer.close().unwrap();
        }
        let buf = get_parquet_serialized_metadata(&parquet_path)
            .await
            .unwrap();
        let metadata = ParquetMetaDataReader::decode_metadata(&buf).unwrap();
        assert_eq!(metadata.file_metadata().num_rows(), 5);
        assert_eq!(metadata.num_row_groups(), 1);
        let group = metadata.row_group(0);
        assert_eq!(group.num_columns(), 1);
        let Some(Statistics::Int32(stats)) = group.column(0).statistics() else {
            panic!("expected Int32 column statistics");
        };
        assert_eq!(stats.null_count_opt(), Some(1));
        assert_eq!(stats.min_opt(), Some(&1));
        assert_eq!(stats.max_opt(), Some(&5));
    }
}
