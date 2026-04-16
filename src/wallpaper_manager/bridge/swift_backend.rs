use crate::wallpaper_manager::backend::WallpaperBackend;
use crate::wallpaper_manager::domain::{
    ScreenDescriptor, ScreenId, WallpaperAssignment, WallpaperState,
};
use crate::wallpaper_manager::error::WallpaperError;

#[derive(Debug, Default, Clone, Copy)]
pub struct SwiftAppKitBridgeBackend;

impl SwiftAppKitBridgeBackend {
    pub fn new() -> Self {
        Self
    }

    fn unavailable() -> WallpaperError {
        WallpaperError::PlatformApiError(
            "Swift AppKit bridge is not wired into this crate yet".to_string(),
        )
    }
}

impl WallpaperBackend for SwiftAppKitBridgeBackend {
    fn list_screens(&self) -> Result<Vec<ScreenDescriptor>, WallpaperError> {
        Err(Self::unavailable())
    }

    fn get_wallpaper(&self, _screen_id: &ScreenId) -> Result<WallpaperState, WallpaperError> {
        Err(Self::unavailable())
    }

    fn set_wallpaper(&self, _assignment: &WallpaperAssignment) -> Result<(), WallpaperError> {
        Err(Self::unavailable())
    }
}
