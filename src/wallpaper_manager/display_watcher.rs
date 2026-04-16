use crate::wallpaper_manager::domain::ScreenId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayChangeEvent {
    ScreenAdded(ScreenId),
    ScreenRemoved(ScreenId),
    ScreenReidentified { from: ScreenId, to: ScreenId },
    ScreenAmbiguous(ScreenId),
}

pub trait DisplayWatcher {
    fn poll_changes(&mut self) -> Vec<DisplayChangeEvent>;
}
