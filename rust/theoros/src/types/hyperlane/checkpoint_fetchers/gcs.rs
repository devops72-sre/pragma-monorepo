// Source:
// https://github.com/hyperlane-xyz/hyperlane-monorepo/blob/3e90734310fb1ca9a607ce3d334015fa7aaa9208/rust/hyperlane-base/src/types/gcs_storage.rs#L63
use std::fmt;

use anyhow::Result;
use async_trait::async_trait;
use ya_gcp::{storage::StorageClient, AuthFlow, ClientBuilder, ClientBuilderConfig};

use crate::types::hyperlane::{FetchFromStorage, SignedCheckpointWithMessageId};

const ANNOUNCEMENT_KEY: &str = "gcsAnnouncementKey";

/// Path to GCS users_secret file
pub const GCS_USER_SECRET: &str = "GCS_USER_SECRET";
/// Path to GCS Service account key
pub const GCS_SERVICE_ACCOUNT_KEY: &str = "GCS_SERVICE_ACCOUNT_KEY";

#[derive(Debug)]
pub struct GcsStorageClientBuilder {
    auth: AuthFlow,
}

impl GcsStorageClientBuilder {
    /// Creates a new [GcsStorageClientBuilder].
    pub fn new(auth: AuthFlow) -> Self {
        GcsStorageClientBuilder { auth }
    }

    /// Builds a [GcsStorageClient].
    pub async fn build(self, bucket_name: impl Into<String>, folder: Option<String>) -> Result<GcsStorageClient> {
        let inner = ClientBuilder::new(ClientBuilderConfig::new().auth_flow(self.auth)).await?.build_storage_client();
        let bucket = if let Some(folder) = folder {
            format! {"{}/{}", bucket_name.into(), folder}
        } else {
            bucket_name.into()
        };

        Ok(GcsStorageClient { inner, bucket })
    }
}

/// Google Cloud Storage client
/// Enables use of any of service account key OR user secrets to authenticate
/// For anonymous access to public data provide `(None, None)` to Builder
pub struct GcsStorageClient {
    // GCS storage client
    // # Details: <https://docs.rs/ya-gcp/latest/ya_gcp/storage/struct.StorageClient.html>
    inner: StorageClient,
    // bucket name of this client's storage
    bucket: String,
}

#[allow(unused)]
impl GcsStorageClient {
    fn get_checkpoint_key(index: u32) -> String {
        format!("checkpoint_{index}_with_id.json")
    }

    fn get_latest_checkpoint_key() -> String {
        "checkpoint_latest_index.json".to_string()
    }
}

#[async_trait]
impl FetchFromStorage for GcsStorageClient {
    async fn fetch(&self, index: u32) -> Result<Option<SignedCheckpointWithMessageId>> {
        let res = self.inner.get_object(&self.bucket, GcsStorageClient::get_checkpoint_key(index)).await?;
        Ok(Some(serde_json::from_slice(res.as_ref())?))
    }

    fn announcement_location(&self) -> String {
        format!("gs://{}/{}", &self.bucket, ANNOUNCEMENT_KEY)
    }
}

impl fmt::Debug for GcsStorageClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("S3Storage").field("bucket", &self.bucket).finish()
    }
}
