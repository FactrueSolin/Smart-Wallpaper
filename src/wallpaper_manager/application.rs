use std::collections::HashSet;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};

use crate::wallpaper_manager::backend::WallpaperBackend;
use crate::wallpaper_manager::domain::{
    BatchSetReport, ScreenId, ScreenOperationError, WallpaperAssignment, WallpaperOptions,
    WallpaperSetRequest, WallpaperState,
};
use crate::wallpaper_manager::error::WallpaperError;
use crate::wallpaper_manager::store::WallpaperStateStore;

pub struct WallpaperManager<B: WallpaperBackend> {
    backend: B,
    store: WallpaperStateStore,
}

impl<B: WallpaperBackend> WallpaperManager<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            store: WallpaperStateStore::new(),
        }
    }

    pub fn store(&self) -> &WallpaperStateStore {
        &self.store
    }

    pub fn refresh(&mut self) -> Result<Vec<WallpaperState>, WallpaperError> {
        let screens = self.backend.list_screens()?;
        let states = self.read_states(&screens)?;

        self.store.replace(screens, states.clone());
        Ok(states)
    }

    pub fn apply(
        &mut self,
        request: WallpaperSetRequest,
    ) -> Result<BatchSetReport, WallpaperError> {
        match request {
            WallpaperSetRequest::ApplyToScreen(assignment) => {
                let screen_id = assignment.screen_id.clone();
                self.set_for_screen(assignment)?;
                Ok(BatchSetReport {
                    succeeded: vec![screen_id],
                    failed: Vec::new(),
                })
            }
            WallpaperSetRequest::ApplyToAllScreens {
                image_path,
                options,
            } => self.set_for_all(image_path, options),
            WallpaperSetRequest::ApplyPerScreen(assignments) => self.set_batch(assignments),
        }
    }

    pub fn set_for_screen(
        &mut self,
        assignment: WallpaperAssignment,
    ) -> Result<(), WallpaperError> {
        self.validate_assignment(&assignment)?;
        match self.backend.set_wallpaper(&assignment) {
            Ok(()) => self.confirm_assignment(&assignment),
            Err(error) => {
                self.reload_screen_state(&assignment.screen_id);
                Err(error)
            }
        }
    }

    pub fn set_for_all(
        &mut self,
        image_path: PathBuf,
        options: WallpaperOptions,
    ) -> Result<BatchSetReport, WallpaperError> {
        let screens = self.backend.list_screens()?;
        let assignments = screens
            .into_iter()
            .map(|screen| WallpaperAssignment {
                screen_id: screen.screen_id,
                image_path: image_path.clone(),
                options: options.clone(),
            })
            .collect();
        self.set_batch(assignments)
    }

    pub fn set_batch(
        &mut self,
        assignments: Vec<WallpaperAssignment>,
    ) -> Result<BatchSetReport, WallpaperError> {
        self.validate_batch(&assignments)?;

        let mut report = BatchSetReport::default();

        for assignment in assignments {
            match self.set_for_screen(assignment.clone()) {
                Ok(()) => report.succeeded.push(assignment.screen_id),
                Err(error) => report.failed.push(ScreenOperationError {
                    screen_id: assignment.screen_id,
                    image_path: assignment.image_path,
                    error_code: error.code().to_string(),
                    message: error.to_string(),
                }),
            }
        }

        if report.is_success() {
            Ok(report)
        } else {
            let _ = self.refresh();
            Err(WallpaperError::PartialFailure(report))
        }
    }

    fn validate_assignment(&self, assignment: &WallpaperAssignment) -> Result<(), WallpaperError> {
        self.ensure_screen_exists(&assignment.screen_id)?;
        validate_image_path(&assignment.image_path)
    }

    fn validate_batch(&self, assignments: &[WallpaperAssignment]) -> Result<(), WallpaperError> {
        if assignments.is_empty() {
            return Err(WallpaperError::EmptyBatchRequest);
        }

        let mut screen_ids = HashSet::with_capacity(assignments.len());
        for assignment in assignments {
            if !screen_ids.insert(assignment.screen_id.clone()) {
                return Err(WallpaperError::DuplicateScreenAssignment(
                    assignment.screen_id.clone(),
                ));
            }
        }

        Ok(())
    }

    fn ensure_screen_exists(&self, screen_id: &ScreenId) -> Result<(), WallpaperError> {
        let has_screen = self
            .backend
            .list_screens()?
            .into_iter()
            .any(|screen| screen.screen_id == *screen_id);

        if has_screen {
            Ok(())
        } else {
            Err(WallpaperError::ScreenNotFound(screen_id.clone()))
        }
    }

    fn read_states(
        &self,
        screens: &[crate::wallpaper_manager::domain::ScreenDescriptor],
    ) -> Result<Vec<WallpaperState>, WallpaperError> {
        let mut states = Vec::with_capacity(screens.len());

        for screen in screens {
            states.push(self.backend.get_wallpaper(&screen.screen_id)?);
        }

        Ok(states)
    }

    fn confirm_assignment(
        &mut self,
        assignment: &WallpaperAssignment,
    ) -> Result<(), WallpaperError> {
        let latest_state = self.backend.get_wallpaper(&assignment.screen_id)?;
        let actual = latest_state.image_path.clone();

        if actual.as_ref() == Some(&assignment.image_path) {
            self.store.upsert_state(latest_state);
            Ok(())
        } else {
            self.store.upsert_state(latest_state);
            Err(WallpaperError::ReadAfterWriteMismatch {
                screen_id: assignment.screen_id.clone(),
                expected: assignment.image_path.clone(),
                actual,
            })
        }
    }

    fn reload_screen_state(&mut self, screen_id: &ScreenId) {
        if let Ok(state) = self.backend.get_wallpaper(screen_id) {
            self.store.upsert_state(state);
        }
    }
}

fn validate_image_path(path: &Path) -> Result<(), WallpaperError> {
    let metadata = fs::symlink_metadata(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::PermissionDenied => {
            WallpaperError::PermissionDenied(path.to_path_buf())
        }
        _ => WallpaperError::InvalidImagePath(path.to_path_buf()),
    })?;

    if metadata.file_type().is_symlink() && fs::metadata(path).is_err() {
        return Err(WallpaperError::InvalidImagePath(path.to_path_buf()));
    }

    let metadata = fs::metadata(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::PermissionDenied => {
            WallpaperError::PermissionDenied(path.to_path_buf())
        }
        _ => WallpaperError::InvalidImagePath(path.to_path_buf()),
    })?;

    if !metadata.is_file() {
        return Err(WallpaperError::InvalidImagePath(path.to_path_buf()));
    }

    File::open(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::PermissionDenied => {
            WallpaperError::PermissionDenied(path.to_path_buf())
        }
        _ => WallpaperError::InvalidImagePath(path.to_path_buf()),
    })?;

    if !is_supported_image(path) {
        return Err(WallpaperError::UnsupportedImageFormat(path.to_path_buf()));
    }

    Ok(())
}

fn is_supported_image(path: &Path) -> bool {
    const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "tiff", "bmp"];

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|supported| ext.eq_ignore_ascii_case(supported))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    use super::WallpaperManager;
    use crate::wallpaper_manager::backend::WallpaperBackend;
    use crate::wallpaper_manager::domain::{
        DisplayFingerprint, ScreenDescriptor, ScreenFrame, ScreenId, ScreenMatchQuality,
        ScreenSize, WallpaperAssignment, WallpaperOptions, WallpaperScaling, WallpaperState,
    };
    use crate::wallpaper_manager::error::WallpaperError;

    struct MockWallpaperBackend {
        screens: Vec<ScreenDescriptor>,
        states: RefCell<HashMap<ScreenId, WallpaperState>>,
        fail_on_set: RefCell<Vec<ScreenId>>,
        mismatch_on_read_after_write: RefCell<Vec<ScreenId>>,
    }

    impl MockWallpaperBackend {
        fn new(screens: Vec<ScreenDescriptor>, states: Vec<WallpaperState>) -> Self {
            Self {
                screens,
                states: RefCell::new(
                    states
                        .into_iter()
                        .map(|state| (state.screen_id.clone(), state))
                        .collect(),
                ),
                fail_on_set: RefCell::new(Vec::new()),
                mismatch_on_read_after_write: RefCell::new(Vec::new()),
            }
        }

        fn fail_for(&self, screen_id: ScreenId) {
            self.fail_on_set.borrow_mut().push(screen_id);
        }

        fn mismatch_after_set_for(&self, screen_id: ScreenId) {
            self.mismatch_on_read_after_write
                .borrow_mut()
                .push(screen_id);
        }
    }

    impl WallpaperBackend for MockWallpaperBackend {
        fn list_screens(&self) -> Result<Vec<ScreenDescriptor>, WallpaperError> {
            Ok(self.screens.clone())
        }

        fn get_wallpaper(&self, screen_id: &ScreenId) -> Result<WallpaperState, WallpaperError> {
            self.states
                .borrow()
                .get(screen_id)
                .cloned()
                .ok_or_else(|| WallpaperError::ScreenNotFound(screen_id.clone()))
        }

        fn set_wallpaper(&self, assignment: &WallpaperAssignment) -> Result<(), WallpaperError> {
            if self.fail_on_set.borrow().contains(&assignment.screen_id) {
                return Err(WallpaperError::PlatformApiError(
                    "simulated set failure".to_string(),
                ));
            }

            let target_path = if self
                .mismatch_on_read_after_write
                .borrow()
                .contains(&assignment.screen_id)
            {
                PathBuf::from("different.png")
            } else {
                assignment.image_path.clone()
            };

            self.states.borrow_mut().insert(
                assignment.screen_id.clone(),
                WallpaperState {
                    screen_id: assignment.screen_id.clone(),
                    image_path: Some(target_path),
                    options: assignment.options.clone(),
                },
            );

            Ok(())
        }
    }

    #[test]
    fn refresh_updates_snapshot() {
        let screens = vec![screen("main"), screen("external")];
        let states = vec![state("main", Some("main.jpg")), state("external", None)];
        let backend = MockWallpaperBackend::new(screens, states);
        let mut manager = WallpaperManager::new(backend);

        let refreshed = manager.refresh().expect("refresh should succeed");

        assert_eq!(refreshed.len(), 2);
        assert_eq!(manager.store().snapshot().screens.len(), 2);
        assert_eq!(
            manager
                .store()
                .get_state(&ScreenId::new("main"))
                .and_then(|state| state.image_path.clone()),
            Some(PathBuf::from("main.jpg"))
        );
    }

    #[test]
    fn set_for_screen_updates_cached_state() {
        let backend = MockWallpaperBackend::new(vec![screen("main")], vec![state("main", None)]);
        let mut manager = WallpaperManager::new(backend);
        let image_path = temp_image("single-screen.jpg");

        manager
            .set_for_screen(WallpaperAssignment {
                screen_id: ScreenId::new("main"),
                image_path: image_path.clone(),
                options: WallpaperOptions::default(),
            })
            .expect("set should succeed");

        assert_eq!(
            manager
                .store()
                .get_state(&ScreenId::new("main"))
                .and_then(|state| state.image_path.clone()),
            Some(image_path)
        );
    }

    #[test]
    fn set_for_screen_rejects_unknown_screen() {
        let backend = MockWallpaperBackend::new(vec![screen("main")], vec![state("main", None)]);
        let mut manager = WallpaperManager::new(backend);
        let image_path = temp_image("missing-screen.png");

        let error = manager
            .set_for_screen(WallpaperAssignment {
                screen_id: ScreenId::new("external"),
                image_path,
                options: WallpaperOptions::default(),
            })
            .expect_err("unknown screen should fail");

        assert!(matches!(error, WallpaperError::ScreenNotFound(_)));
    }

    #[test]
    fn set_batch_collects_partial_failures() {
        let backend = MockWallpaperBackend::new(
            vec![screen("main"), screen("external")],
            vec![state("main", None), state("external", None)],
        );
        backend.fail_for(ScreenId::new("external"));
        let mut manager = WallpaperManager::new(backend);
        let first = temp_image("batch-main.png");
        let second = temp_image("batch-external.png");

        let error = manager
            .set_batch(vec![
                WallpaperAssignment {
                    screen_id: ScreenId::new("main"),
                    image_path: first.clone(),
                    options: WallpaperOptions::default(),
                },
                WallpaperAssignment {
                    screen_id: ScreenId::new("external"),
                    image_path: second,
                    options: WallpaperOptions::default(),
                },
            ])
            .expect_err("one failing screen should return partial failure");

        match error {
            WallpaperError::PartialFailure(report) => {
                assert_eq!(report.succeeded, vec![ScreenId::new("main")]);
                assert_eq!(report.failed.len(), 1);
                assert_eq!(report.failed[0].screen_id, ScreenId::new("external"));
            }
            other => panic!("unexpected error: {other}"),
        }

        assert_eq!(
            manager
                .store()
                .get_state(&ScreenId::new("main"))
                .and_then(|state| state.image_path.clone()),
            Some(first)
        );
    }

    #[test]
    fn set_batch_rejects_empty_requests() {
        let backend = MockWallpaperBackend::new(vec![screen("main")], vec![state("main", None)]);
        let mut manager = WallpaperManager::new(backend);

        let error = manager
            .set_batch(Vec::new())
            .expect_err("empty batch should be rejected");

        assert!(matches!(error, WallpaperError::EmptyBatchRequest));
    }

    #[test]
    fn set_batch_rejects_duplicate_screen_assignments() {
        let backend = MockWallpaperBackend::new(vec![screen("main")], vec![state("main", None)]);
        let mut manager = WallpaperManager::new(backend);
        let first = temp_image("duplicate-1.png");
        let second = temp_image("duplicate-2.png");

        let error = manager
            .set_batch(vec![
                WallpaperAssignment {
                    screen_id: ScreenId::new("main"),
                    image_path: first,
                    options: WallpaperOptions::default(),
                },
                WallpaperAssignment {
                    screen_id: ScreenId::new("main"),
                    image_path: second,
                    options: WallpaperOptions::default(),
                },
            ])
            .expect_err("duplicate screen ids should fail");

        assert!(matches!(
            error,
            WallpaperError::DuplicateScreenAssignment(screen_id) if screen_id == ScreenId::new("main")
        ));
    }

    #[test]
    fn set_for_screen_rejects_read_after_write_mismatch() {
        let backend = MockWallpaperBackend::new(vec![screen("main")], vec![state("main", None)]);
        backend.mismatch_after_set_for(ScreenId::new("main"));
        let mut manager = WallpaperManager::new(backend);
        let image_path = temp_image("mismatch.png");

        let error = manager
            .set_for_screen(WallpaperAssignment {
                screen_id: ScreenId::new("main"),
                image_path: image_path.clone(),
                options: WallpaperOptions::default(),
            })
            .expect_err("mismatch should fail");

        assert!(matches!(
            error,
            WallpaperError::ReadAfterWriteMismatch { screen_id, expected, actual }
                if screen_id == ScreenId::new("main")
                    && expected == image_path
                    && actual == Some(PathBuf::from("different.png"))
        ));
        assert_eq!(
            manager
                .store()
                .get_state(&ScreenId::new("main"))
                .and_then(|state| state.image_path.clone()),
            Some(PathBuf::from("different.png"))
        );
    }

    #[test]
    fn set_for_screen_rejects_unsupported_extensions() {
        let backend = MockWallpaperBackend::new(vec![screen("main")], vec![state("main", None)]);
        let mut manager = WallpaperManager::new(backend);
        let invalid_path = temp_file("not-image.txt");

        let error = manager
            .set_for_screen(WallpaperAssignment {
                screen_id: ScreenId::new("main"),
                image_path: invalid_path,
                options: WallpaperOptions::default(),
            })
            .expect_err("unsupported image extension should fail");

        assert!(matches!(error, WallpaperError::UnsupportedImageFormat(_)));
    }

    fn screen(id: &str) -> ScreenDescriptor {
        ScreenDescriptor {
            screen_id: ScreenId::new(id),
            localized_name: id.to_string(),
            is_builtin: id == "main",
            is_primary: id == "main",
            frame: ScreenFrame {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
            },
            native_size: ScreenSize {
                width: 1920,
                height: 1080,
            },
            vendor_id: Some(1),
            model_id: Some(2),
            serial_number: Some(3),
            fingerprint: DisplayFingerprint {
                vendor_id: Some(1),
                model_id: Some(2),
                serial_number: Some(3),
                is_builtin: id == "main",
            },
            match_quality: ScreenMatchQuality::Exact,
        }
    }

    fn state(id: &str, image_path: Option<&str>) -> WallpaperState {
        WallpaperState {
            screen_id: ScreenId::new(id),
            image_path: image_path.map(PathBuf::from),
            options: WallpaperOptions {
                scaling: WallpaperScaling::Fill,
                allow_clipping: true,
            },
        }
    }

    fn temp_image(name: &str) -> PathBuf {
        let path = env::temp_dir().join(name);
        fs::write(&path, b"test-image").expect("temp image should be written");
        path
    }

    fn temp_file(name: &str) -> PathBuf {
        let path = env::temp_dir().join(name);
        fs::write(&path, b"test-file").expect("temp file should be written");
        path
    }
}
