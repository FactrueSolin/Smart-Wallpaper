use std::collections::{HashMap, HashSet};

use crate::wallpaper_manager::domain::{
    DisplayFingerprint, ScreenDescriptor, ScreenId, ScreenMatchQuality,
};

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

#[derive(Debug, Clone, Default)]
pub struct SnapshotDisplayWatcher {
    previous: Vec<ScreenDescriptor>,
    pending: Vec<DisplayChangeEvent>,
}

impl SnapshotDisplayWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_snapshot(&mut self, current: Vec<ScreenDescriptor>) {
        self.pending = if self.previous.is_empty() {
            Vec::new()
        } else {
            diff_screens(&self.previous, &current)
        };
        self.previous = current;
    }
}

impl DisplayWatcher for SnapshotDisplayWatcher {
    fn poll_changes(&mut self) -> Vec<DisplayChangeEvent> {
        std::mem::take(&mut self.pending)
    }
}

fn diff_screens(
    previous: &[ScreenDescriptor],
    current: &[ScreenDescriptor],
) -> Vec<DisplayChangeEvent> {
    let previous_by_id: HashMap<&ScreenId, &ScreenDescriptor> = previous
        .iter()
        .map(|screen| (&screen.screen_id, screen))
        .collect();
    let current_by_id: HashMap<&ScreenId, &ScreenDescriptor> = current
        .iter()
        .map(|screen| (&screen.screen_id, screen))
        .collect();

    let mut events = Vec::new();
    let mut reidentified_current = HashSet::new();

    for screen in current {
        if screen.match_quality == ScreenMatchQuality::Ambiguous {
            events.push(DisplayChangeEvent::ScreenAmbiguous(
                screen.screen_id.clone(),
            ));
        }
    }

    for screen in current {
        if previous_by_id.contains_key(&screen.screen_id) {
            continue;
        }

        if let Some(previous_screen) = previous
            .iter()
            .find(|candidate| fingerprints_match(&candidate.fingerprint, &screen.fingerprint))
        {
            events.push(DisplayChangeEvent::ScreenReidentified {
                from: previous_screen.screen_id.clone(),
                to: screen.screen_id.clone(),
            });
            reidentified_current.insert(screen.screen_id.clone());
        } else {
            events.push(DisplayChangeEvent::ScreenAdded(screen.screen_id.clone()));
        }
    }

    for screen in previous {
        if current_by_id.contains_key(&screen.screen_id) {
            continue;
        }

        let was_reidentified = current.iter().any(|candidate| {
            reidentified_current.contains(&candidate.screen_id)
                && fingerprints_match(&candidate.fingerprint, &screen.fingerprint)
        });

        if !was_reidentified {
            events.push(DisplayChangeEvent::ScreenRemoved(screen.screen_id.clone()));
        }
    }

    events
}

fn fingerprints_match(left: &DisplayFingerprint, right: &DisplayFingerprint) -> bool {
    left == right
}

#[cfg(test)]
mod tests {
    use super::{DisplayChangeEvent, DisplayWatcher, SnapshotDisplayWatcher};
    use crate::wallpaper_manager::domain::{
        DisplayFingerprint, ScreenDescriptor, ScreenFrame, ScreenId, ScreenMatchQuality, ScreenSize,
    };

    #[test]
    fn watcher_emits_reidentify_and_ambiguous_events() {
        let mut watcher = SnapshotDisplayWatcher::new();
        watcher.push_snapshot(vec![screen(
            "old-main",
            ScreenMatchQuality::Exact,
            Some(100),
        )]);
        assert!(watcher.poll_changes().is_empty());

        watcher.push_snapshot(vec![
            screen("new-main", ScreenMatchQuality::Exact, Some(100)),
            screen("unclear", ScreenMatchQuality::Ambiguous, Some(200)),
        ]);

        let events = watcher.poll_changes();

        assert!(events.contains(&DisplayChangeEvent::ScreenReidentified {
            from: ScreenId::new("old-main"),
            to: ScreenId::new("new-main"),
        }));
        assert!(
            events.contains(&DisplayChangeEvent::ScreenAmbiguous(ScreenId::new(
                "unclear"
            )))
        );
    }

    fn screen(
        id: &str,
        quality: ScreenMatchQuality,
        serial_number: Option<u32>,
    ) -> ScreenDescriptor {
        ScreenDescriptor {
            screen_id: ScreenId::new(id),
            localized_name: id.to_string(),
            is_builtin: id.contains("main"),
            is_primary: id.contains("main"),
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
            serial_number,
            fingerprint: DisplayFingerprint {
                vendor_id: Some(1),
                model_id: Some(2),
                serial_number,
                is_builtin: id.contains("main"),
            },
            match_quality: quality,
        }
    }
}
