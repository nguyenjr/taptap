use crate::settings::{Settings, SoundPreset};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayStateSnapshot {
    pub active: bool,
    pub presentation: bool,
    pub scale_pct: u32,
    pub sound: SoundPreset,
    pub volume: u8,
}

impl Default for OverlayStateSnapshot {
    fn default() -> Self {
        Self {
            active: true,
            presentation: false,
            scale_pct: 100,
            sound: SoundPreset::Wood,
            volume: 100,
        }
    }
}

impl OverlayStateSnapshot {
    pub fn suppresses_clicks(self) -> bool {
        self.active && self.presentation
    }
}

#[derive(Clone, Copy)]
pub enum Command {
    ToggleActive,
    TogglePresentation,
    ScaleUp,
    ScaleDown,
    SetScale(u32),
    SetSound(SoundPreset),
    SetVolume(u8),
    SettingsLoaded(Settings),
    Quit,
}

pub struct OverlayState {
    active: AtomicBool,
    presentation: AtomicBool,
    scale_pct: AtomicU32,
    sound: AtomicU8,
    volume: AtomicU8,
}

impl Default for OverlayState {
    fn default() -> Self {
        Self::new()
    }
}

impl OverlayState {
    pub fn new() -> Self {
        let defaults = OverlayStateSnapshot::default();
        Self {
            active: AtomicBool::new(defaults.active),
            presentation: AtomicBool::new(defaults.presentation),
            scale_pct: AtomicU32::new(defaults.scale_pct),
            sound: AtomicU8::new(defaults.sound.as_u8()),
            volume: AtomicU8::new(defaults.volume),
        }
    }

    pub fn snapshot(&self) -> OverlayStateSnapshot {
        OverlayStateSnapshot {
            active: self.active.load(Ordering::SeqCst),
            presentation: self.presentation.load(Ordering::SeqCst),
            scale_pct: self.scale_pct.load(Ordering::SeqCst),
            sound: SoundPreset::from_u8(self.sound.load(Ordering::SeqCst)),
            volume: self.volume.load(Ordering::SeqCst),
        }
    }

    pub fn suppresses_clicks(&self) -> bool {
        self.snapshot().suppresses_clicks()
    }

    pub fn settings(&self) -> Settings {
        let snapshot = self.snapshot();
        Settings {
            scale_pct: snapshot.scale_pct,
            sound: snapshot.sound,
            volume: snapshot.volume,
        }
    }

    pub fn apply(&self, command: Command) {
        match command {
            Command::ToggleActive => {
                let next = !self.active.load(Ordering::SeqCst);
                self.active.store(next, Ordering::SeqCst);
            }
            Command::TogglePresentation => {
                let next = !self.presentation.load(Ordering::SeqCst);
                self.presentation.store(next, Ordering::SeqCst);
            }
            Command::ScaleUp | Command::ScaleDown => {
                let delta = if matches!(command, Command::ScaleUp) {
                    10
                } else {
                    -10
                };
                let next =
                    (self.scale_pct.load(Ordering::SeqCst) as i32 + delta).clamp(50, 200) as u32;
                self.scale_pct.store(next, Ordering::SeqCst);
            }
            Command::SetScale(pct) => self.scale_pct.store(pct.clamp(50, 200), Ordering::SeqCst),
            Command::SetSound(preset) => self.sound.store(preset.as_u8(), Ordering::SeqCst),
            Command::SetVolume(volume) => {
                self.volume.store(volume.clamp(0, 100), Ordering::SeqCst);
            }
            Command::SettingsLoaded(settings) => {
                self.scale_pct
                    .store(settings.scale_pct.clamp(50, 200), Ordering::SeqCst);
                self.sound.store(settings.sound.as_u8(), Ordering::SeqCst);
                self.volume
                    .store(settings.volume.clamp(0, 100), Ordering::SeqCst);
            }
            Command::Quit => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, OverlayState, OverlayStateSnapshot};
    use crate::settings::{Settings, SoundPreset};

    fn apply(commands: &[Command]) -> OverlayStateSnapshot {
        let state = OverlayState::new();
        for command in commands {
            state.apply(*command);
        }
        state.snapshot()
    }

    #[test]
    fn defaults_are_the_documented_values() {
        assert_eq!(
            OverlayState::new().snapshot(),
            OverlayStateSnapshot::default()
        );
    }

    #[test]
    fn toggles_flip_both_states() {
        let snapshot = apply(&[Command::ToggleActive, Command::TogglePresentation]);
        assert!(!snapshot.active);
        assert!(snapshot.presentation);
    }

    #[test]
    fn clicks_are_suppressed_only_while_active() {
        let presenting = apply(&[Command::TogglePresentation]);
        assert!(presenting.suppresses_clicks());

        let inactive = apply(&[Command::TogglePresentation, Command::ToggleActive]);
        assert!(!inactive.suppresses_clicks());
    }

    #[test]
    fn scale_steps_clamp_and_presets_apply() {
        let commands: Vec<Command> = (0..20).map(|_| Command::ScaleDown).collect();
        assert_eq!(apply(&commands).scale_pct, 50);

        let commands: Vec<Command> = (0..30).map(|_| Command::ScaleUp).collect();
        assert_eq!(apply(&commands).scale_pct, 200);

        assert_eq!(apply(&[Command::SetScale(135)]).scale_pct, 135);
        assert_eq!(apply(&[Command::SetScale(999)]).scale_pct, 200);
    }

    #[test]
    fn volume_clamps_and_sound_selections_apply() {
        assert_eq!(apply(&[Command::SetVolume(250)]).volume, 100);
        assert_eq!(
            apply(&[Command::SetSound(SoundPreset::Bell)]).sound,
            SoundPreset::Bell
        );
    }

    #[test]
    fn settings_loaded_applies_without_changing_active_or_presentation() {
        let snapshot = apply(&[
            Command::TogglePresentation,
            Command::SettingsLoaded(Settings {
                scale_pct: 175,
                sound: SoundPreset::Pop,
                volume: 25,
            }),
        ]);
        assert!(snapshot.active);
        assert!(snapshot.presentation);
        assert_eq!(snapshot.scale_pct, 175);
        assert_eq!(snapshot.sound, SoundPreset::Pop);
        assert_eq!(snapshot.volume, 25);
    }

    #[test]
    fn settings_snapshot_matches_the_persisted_values() {
        let state = OverlayState::new();
        state.apply(Command::SetScale(135));
        state.apply(Command::SetSound(SoundPreset::Click));
        state.apply(Command::SetVolume(0));
        assert_eq!(
            state.settings(),
            Settings {
                scale_pct: 135,
                sound: SoundPreset::Click,
                volume: 0,
            }
        );
    }
}
