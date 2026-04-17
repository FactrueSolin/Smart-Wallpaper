use std::path::PathBuf;

use thiserror::Error;

use crate::wallpaper_manager::domain::{BatchSetReport, ScreenId};

#[derive(Debug, Error)]
pub enum WallpaperError {
    #[error("screen not found: {0}")]
    ScreenNotFound(ScreenId),
    #[error("invalid image path: {0}")]
    InvalidImagePath(PathBuf),
    #[error("unsupported image format: {0}")]
    UnsupportedImageFormat(PathBuf),
    #[error("permission denied: {0}")]
    PermissionDenied(PathBuf),
    #[error("image decode failed: {0}")]
    ImageDecodeFailed(PathBuf),
    #[error("duplicate screen assignment: {0}")]
    DuplicateScreenAssignment(ScreenId),
    #[error("batch request must include at least one assignment")]
    EmptyBatchRequest,
    #[error("main thread violation")]
    MainThreadViolation,
    #[error("platform API error: {0}")]
    PlatformApiError(String),
    #[error("screen topology changed")]
    ScreenTopologyChanged,
    #[error("ambiguous screen mapping")]
    AmbiguousScreenMapping,
    #[error("read after write mismatch for {screen_id}: expected {expected:?}, actual {actual:?}")]
    ReadAfterWriteMismatch {
        screen_id: ScreenId,
        expected: PathBuf,
        actual: Option<PathBuf>,
    },
    #[error("partial failure: {0:?}")]
    PartialFailure(BatchSetReport),
}

impl WallpaperError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ScreenNotFound(_) => "screen_not_found",
            Self::InvalidImagePath(_) => "invalid_image_path",
            Self::UnsupportedImageFormat(_) => "unsupported_image_format",
            Self::PermissionDenied(_) => "permission_denied",
            Self::ImageDecodeFailed(_) => "image_decode_failed",
            Self::DuplicateScreenAssignment(_) => "duplicate_screen_assignment",
            Self::EmptyBatchRequest => "empty_batch_request",
            Self::MainThreadViolation => "main_thread_violation",
            Self::PlatformApiError(_) => "platform_api_error",
            Self::ScreenTopologyChanged => "screen_topology_changed",
            Self::AmbiguousScreenMapping => "ambiguous_screen_mapping",
            Self::ReadAfterWriteMismatch { .. } => "read_after_write_mismatch",
            Self::PartialFailure(_) => "partial_failure",
        }
    }
}
