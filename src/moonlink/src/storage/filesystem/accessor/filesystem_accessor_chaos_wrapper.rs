use opendal::raw::oio::{Delete, List, Read, ReadStream, Write};
use opendal::raw::{
    oio, Layer, OpCopier, OpCopy, OpCreateDir, OpDelete, OpList, OpPresign, OpRead, OpRename,
    OpStat, OpWrite, RpCreateDir, RpPresign, RpRead, RpRename, RpStat, Service, ServiceInfo,
    Servicer,
};
/// A customized opendal layer, which provides chaos features like injected delay, intended errors, etc.
use opendal::BytesRange;
use opendal::{Buffer, Capability, Metadata, OperationContext, Result};
use std::sync::Arc;

use crate::storage::filesystem::accessor::chaos_generator::ChaosGenerator;
use crate::storage::filesystem::accessor_config::ChaosConfig;

/// A wrapper that delegates all operations to an inner [`FileSystemAccessor`].
#[derive(Debug)]
pub struct ChaosLayer {
    /// Chaos generator.
    chaos_generator: ChaosGenerator,
}

impl ChaosLayer {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            chaos_generator: ChaosGenerator::new(config),
        }
    }
}

impl Layer for ChaosLayer {
    fn apply_service(&self, inner: Servicer) -> Servicer {
        Arc::new(ChaosAccessor {
            chaos_generator: self.chaos_generator.clone(),
            inner,
        })
    }
}

#[derive(Debug)]
struct ChaosAccessor {
    /// Chaos generator.
    chaos_generator: ChaosGenerator,
    /// Inner accessor.
    inner: Servicer,
}

impl Service for ChaosAccessor {
    type Reader = ChaosIo<oio::Reader>;
    type Writer = ChaosIo<oio::Writer>;
    type Lister = ChaosIo<oio::Lister>;
    type Deleter = ChaosIo<oio::Deleter>;
    type Copier = oio::Copier;

    fn info(&self) -> ServiceInfo {
        self.inner.info()
    }

    fn capability(&self) -> Capability {
        self.inner.capability()
    }

    fn read(&self, ctx: &OperationContext, path: &str, args: OpRead) -> Result<Self::Reader> {
        Ok(ChaosIo::new(
            self.inner.read(ctx, path, args)?,
            &self.chaos_generator,
        ))
    }

    fn write(&self, ctx: &OperationContext, path: &str, args: OpWrite) -> Result<Self::Writer> {
        Ok(ChaosIo::new(
            self.inner.write(ctx, path, args)?,
            &self.chaos_generator,
        ))
    }

    fn list(&self, ctx: &OperationContext, path: &str, args: OpList) -> Result<Self::Lister> {
        Ok(ChaosIo::new(
            self.inner.list(ctx, path, args)?,
            &self.chaos_generator,
        ))
    }

    fn delete(&self, ctx: &OperationContext) -> Result<Self::Deleter> {
        Ok(ChaosIo::new(self.inner.delete(ctx)?, &self.chaos_generator))
    }

    fn copy(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpCopy,
        opts: OpCopier,
    ) -> Result<Self::Copier> {
        self.inner.copy(ctx, from, to, args, opts)
    }

    async fn create_dir(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpCreateDir,
    ) -> Result<RpCreateDir> {
        self.inner.create_dir(ctx, path, args).await
    }

    async fn rename(
        &self,
        ctx: &OperationContext,
        from: &str,
        to: &str,
        args: OpRename,
    ) -> Result<RpRename> {
        self.inner.rename(ctx, from, to, args).await
    }

    async fn stat(&self, ctx: &OperationContext, path: &str, args: OpStat) -> Result<RpStat> {
        self.inner.stat(ctx, path, args).await
    }

    async fn presign(
        &self,
        ctx: &OperationContext,
        path: &str,
        args: OpPresign,
    ) -> Result<RpPresign> {
        self.inner.presign(ctx, path, args).await
    }
}

struct ChaosIo<Inner> {
    chaos_generator: ChaosGenerator,
    inner: Inner,
}

impl<Inner> ChaosIo<Inner> {
    fn new(inner: Inner, chaos_generator: &ChaosGenerator) -> Self {
        Self {
            chaos_generator: chaos_generator.clone(),
            inner,
        }
    }
}

impl Read for ChaosIo<oio::Reader> {
    async fn open(&self, range: BytesRange) -> Result<(RpRead, Box<dyn oio::ReadStreamDyn>)> {
        self.chaos_generator.perform_wrapper_function().await?;
        let (result, stream) = self.inner.open(range).await?;
        Ok((
            result,
            Box::new(ChaosIo::new(stream, &self.chaos_generator)),
        ))
    }

    async fn read(&self, range: BytesRange) -> Result<(RpRead, Buffer)> {
        self.chaos_generator.perform_wrapper_function().await?;
        self.inner.read(range).await
    }
}

impl ReadStream for ChaosIo<Box<dyn oio::ReadStreamDyn>> {
    async fn read(&mut self) -> Result<Buffer> {
        self.chaos_generator.perform_wrapper_function().await?;
        self.inner.read().await
    }
}

impl List for ChaosIo<oio::Lister> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        self.chaos_generator.perform_wrapper_function().await?;
        self.inner.next().await
    }
}

impl Delete for ChaosIo<oio::Deleter> {
    async fn delete(&mut self, path: &str, args: OpDelete) -> Result<()> {
        self.chaos_generator.perform_wrapper_function().await?;
        self.inner.delete(path, args).await
    }

    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }
}

impl Write for ChaosIo<oio::Writer> {
    async fn write(&mut self, bs: opendal::Buffer) -> Result<()> {
        self.chaos_generator.perform_wrapper_function().await?;
        self.inner.write(bs).await
    }

    async fn abort(&mut self) -> Result<()> {
        self.chaos_generator.perform_wrapper_function().await?;
        self.inner.abort().await
    }

    async fn close(&mut self) -> Result<Metadata> {
        self.chaos_generator.perform_wrapper_function().await?;
        self.inner.close().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::filesystem::accessor::base_filesystem_accessor::BaseFileSystemAccess;
    use crate::storage::filesystem::accessor::filesystem_accessor::FileSystemAccessor;
    use crate::storage::filesystem::accessor_config::AccessorConfig;
    use crate::storage::filesystem::accessor_config::RetryConfig;
    use crate::storage::filesystem::accessor_config::TimeoutConfig;
    use crate::storage::filesystem::storage_config::StorageConfig;
    use tempfile::{tempdir, TempDir};

    /// Test util function to create a filesystem accessor, based on the given chaos option.
    fn create_filesystem_accessor(
        temp_dir: &TempDir,
        chaos_config: ChaosConfig,
        timeout_config: TimeoutConfig,
    ) -> FileSystemAccessor {
        let storage_config = StorageConfig::FileSystem {
            root_directory: temp_dir.path().to_str().unwrap().to_string(),
            atomic_write_dir: None,
        };
        let accessor_config = AccessorConfig {
            storage_config,
            chaos_config: Some(chaos_config),
            retry_config: RetryConfig::default(),
            throttle_config: None,
            timeout_config,
        };
        FileSystemAccessor::new(accessor_config)
    }

    /// Test util function to write and read an object, which should succeed whether delay injected.
    async fn perform_read_write_op(filesystem_accessor: &FileSystemAccessor) {
        // Write object.
        let filename = "test_object.txt".to_string();
        let content = b"helloworld".to_vec();
        filesystem_accessor
            .write_object(&filename, content.clone())
            .await
            .unwrap();

        // Read object.
        let read_content = filesystem_accessor.read_object(&filename).await.unwrap();
        assert_eq!(read_content, content);
    }

    #[tokio::test]
    async fn test_no_delay_no_error() {
        let temp_dir = tempdir().unwrap();
        let chaos_config = ChaosConfig {
            random_seed: None,
            min_latency: std::time::Duration::ZERO,
            max_latency: std::time::Duration::ZERO,
            err_prob: 0,
        };
        let filesystem_accessor =
            create_filesystem_accessor(&temp_dir, chaos_config, TimeoutConfig::default());
        perform_read_write_op(&filesystem_accessor).await;
    }

    #[tokio::test]
    async fn test_delay_injected() {
        let temp_dir = tempdir().unwrap();
        let chaos_config = ChaosConfig {
            random_seed: None,
            min_latency: std::time::Duration::from_millis(5000),
            max_latency: std::time::Duration::from_millis(5000),
            err_prob: 0,
        };
        let timeout_config = TimeoutConfig {
            timeout: std::time::Duration::from_millis(500),
        };
        // Timeout is less than injected delay.
        let filesystem_accessor =
            create_filesystem_accessor(&temp_dir, chaos_config, timeout_config);
        let res = filesystem_accessor.read_object("FAKE_FILEPATH").await;
        assert!(res.is_err());
    }
}
