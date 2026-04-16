use crate::wallpaper_manager::domain::{
    ScreenDescriptor, ScreenId, WallpaperAssignment, WallpaperState,
};
use crate::wallpaper_manager::error::WallpaperError;

pub trait WallpaperBackend {
    fn list_screens(&self) -> Result<Vec<ScreenDescriptor>, WallpaperError>;
    fn get_wallpaper(&self, screen_id: &ScreenId) -> Result<WallpaperState, WallpaperError>;
    fn set_wallpaper(&self, assignment: &WallpaperAssignment) -> Result<(), WallpaperError>;
}
