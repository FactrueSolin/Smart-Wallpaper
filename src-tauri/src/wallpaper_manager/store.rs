use std::collections::HashMap;

use crate::wallpaper_manager::domain::{ScreenDescriptor, ScreenId, WallpaperState};

#[derive(Debug, Clone, Default)]
pub struct WallpaperSnapshot {
    pub screens: Vec<ScreenDescriptor>,
    pub wallpapers: Vec<WallpaperState>,
}

#[derive(Debug, Clone, Default)]
pub struct WallpaperStateStore {
    snapshot: WallpaperSnapshot,
    states_by_screen: HashMap<ScreenId, WallpaperState>,
    screens_by_id: HashMap<ScreenId, ScreenDescriptor>,
}

impl WallpaperStateStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> &WallpaperSnapshot {
        &self.snapshot
    }

    pub fn replace(&mut self, screens: Vec<ScreenDescriptor>, wallpapers: Vec<WallpaperState>) {
        self.states_by_screen = wallpapers
            .iter()
            .cloned()
            .map(|state| (state.screen_id.clone(), state))
            .collect();
        self.screens_by_id = screens
            .iter()
            .cloned()
            .map(|screen| (screen.screen_id.clone(), screen))
            .collect();
        self.snapshot = WallpaperSnapshot {
            screens,
            wallpapers,
        };
    }

    pub fn replace_states(&mut self, wallpapers: Vec<WallpaperState>) {
        self.states_by_screen = wallpapers
            .iter()
            .cloned()
            .map(|state| (state.screen_id.clone(), state))
            .collect();
        self.snapshot.wallpapers = wallpapers;
    }

    pub fn upsert_state(&mut self, state: WallpaperState) {
        let screen_id = state.screen_id.clone();
        self.states_by_screen
            .insert(screen_id.clone(), state.clone());

        match self
            .snapshot
            .wallpapers
            .iter_mut()
            .find(|existing| existing.screen_id == screen_id)
        {
            Some(existing) => *existing = state,
            None => self.snapshot.wallpapers.push(state),
        }
    }

    pub fn get_state(&self, screen_id: &ScreenId) -> Option<&WallpaperState> {
        self.states_by_screen.get(screen_id)
    }

    pub fn get_screen(&self, screen_id: &ScreenId) -> Option<&ScreenDescriptor> {
        self.screens_by_id.get(screen_id)
    }

    pub fn screens(&self) -> &[ScreenDescriptor] {
        &self.snapshot.screens
    }

    pub fn contains_screen(&self, screen_id: &ScreenId) -> bool {
        self.screens_by_id.contains_key(screen_id)
    }
}
