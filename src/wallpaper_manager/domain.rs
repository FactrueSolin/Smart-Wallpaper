use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScreenId(String);

impl ScreenId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ScreenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for ScreenId {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for ScreenId {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallpaperScaling {
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperOptions {
    pub scaling: WallpaperScaling,
    pub allow_clipping: bool,
}

impl Default for WallpaperOptions {
    fn default() -> Self {
        Self {
            scaling: WallpaperScaling::Fill,
            allow_clipping: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperAssignment {
    pub screen_id: ScreenId,
    pub image_path: PathBuf,
    pub options: WallpaperOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperState {
    pub screen_id: ScreenId,
    pub image_path: Option<PathBuf>,
    pub options: WallpaperOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenFrame {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayFingerprint {
    pub vendor_id: Option<u32>,
    pub model_id: Option<u32>,
    pub serial_number: Option<u32>,
    pub is_builtin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenMatchQuality {
    Exact,
    Fuzzy,
    Ambiguous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenDescriptor {
    pub screen_id: ScreenId,
    pub localized_name: String,
    pub is_builtin: bool,
    pub is_primary: bool,
    pub frame: ScreenFrame,
    pub native_size: ScreenSize,
    pub vendor_id: Option<u32>,
    pub model_id: Option<u32>,
    pub serial_number: Option<u32>,
    pub fingerprint: DisplayFingerprint,
    pub match_quality: ScreenMatchQuality,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallpaperSetRequest {
    ApplyToScreen(WallpaperAssignment),
    ApplyToAllScreens {
        image_path: PathBuf,
        options: WallpaperOptions,
    },
    ApplyPerScreen(Vec<WallpaperAssignment>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenOperationError {
    pub screen_id: ScreenId,
    pub image_path: PathBuf,
    pub error_code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BatchSetReport {
    pub succeeded: Vec<ScreenId>,
    pub failed: Vec<ScreenOperationError>,
}

impl BatchSetReport {
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }
}
