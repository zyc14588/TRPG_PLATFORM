
impl DeletionBatchProgress {
    fn complete(cursor: u64) -> Result<Self, PrivacyError> {
        if cursor == 0 {
            return Err(PrivacyError::InvalidPersistedState);
        }
        Ok(Self {
            next_cursor: cursor,
            complete: true,
        })
    }
}

const S3_LIST_PAGE_SIZE: i32 = 100;
const S3_DELETE_REQUEST_SIZE: usize = 1_000;
const S3_MAX_LIST_PAGES: usize = 100_000;
const S3_MAX_ERASURE_PASSES: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum S3VersioningMode {
    NeverVersioned,
    VersionHistory,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct S3VersionIdentifier {
    key: String,
    version_id: Option<String>,
}

#[derive(Clone, Debug)]
struct S3ListedObjects {
    identifiers: Vec<S3VersionIdentifier>,
    page_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct S3DeleteReceiptSummary {
    pub request_count: u64,
    pub requested_count: u64,
    pub confirmed_count: u64,
    pub error_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct S3ErasureEvidence {
    pub bucket: String,
    pub prefix: String,
    pub page_count: u64,
    pub version_count: u64,
    pub delete_receipt_summary: S3DeleteReceiptSummary,
    pub final_verified_at_unix_ms: u64,
    pub manifest_sha256: String,
}

#[derive(Clone)]
pub struct S3ObjectDeletionSurface {
    client: S3Client,
    bucket: String,
    last_evidence: std::sync::Arc<std::sync::Mutex<Option<S3ErasureEvidence>>>,
}

impl std::fmt::Debug for S3ObjectDeletionSurface {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("S3ObjectDeletionSurface")
            .field("bucket", &self.bucket)
            .field("endpoint", &"[REDACTED]")
            .field("credentials", &"[REDACTED]")
            .finish()
    }
}
