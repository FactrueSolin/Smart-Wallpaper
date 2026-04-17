pub mod application;
pub mod backend;
pub mod bridge;
pub mod display_watcher;
pub mod domain;
pub mod error;
pub mod store;

pub use application::WallpaperManager;
pub use backend::WallpaperBackend;
pub use bridge::swift_backend::SwiftAppKitBridgeBackend;
pub use display_watcher::{DisplayChangeEvent, DisplayWatcher, SnapshotDisplayWatcher};
pub use domain::{
    BatchSetReport, DisplayFingerprint, ScreenDescriptor, ScreenFrame, ScreenId,
    ScreenMatchQuality, ScreenOperationError, ScreenSize, WallpaperAssignment, WallpaperOptions,
    WallpaperScaling, WallpaperSetRequest, WallpaperState,
};
pub use error::WallpaperError;
pub use store::{WallpaperSnapshot, WallpaperStateStore};
