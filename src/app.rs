use crate::overlay_state::{Command, OverlayState};
use crate::platform::{Effect, Event, Hotkey, MenuAction, Platform};
use std::sync::Arc;

pub struct App {
    state: Arc<OverlayState>,
    quit: bool,
}

impl App {
    pub fn new(state: Arc<OverlayState>) -> Self {
        Self { state, quit: false }
    }

    #[cfg(test)]
    pub fn state(&self) -> crate::overlay_state::OverlayStateSnapshot {
        self.state.snapshot()
    }

    #[cfg(test)]
    pub fn is_quit(&self) -> bool {
        self.quit
    }

    pub fn run(&mut self, platform: &mut dyn Platform) {
        while let Some(event) = platform.next_event() {
            self.handle(event, platform);
            if self.quit {
                return;
            }
        }
        self.handle(Event::Shutdown, platform);
    }

    pub fn handle(&mut self, event: Event, platform: &mut dyn Platform) {
        match event {
            Event::Start => {
                platform.emit(Effect::LoadSettings);
                platform.emit(Effect::RegisterHotkeys);
                platform.emit(Effect::InstallMouseHook);
                platform.emit(Effect::HideSystemCursors);
                platform.emit(Effect::StartRenderThread);
            }
            Event::Hotkey(hotkey) => {
                let command = match hotkey {
                    Hotkey::ToggleActive => Command::ToggleActive,
                    Hotkey::TogglePresentation => Command::TogglePresentation,
                    Hotkey::ScaleUp => Command::ScaleUp,
                    Hotkey::ScaleDown => Command::ScaleDown,
                    Hotkey::Quit => Command::Quit,
                };
                self.command(command, platform);
            }
            Event::Menu(action) => {
                let command = match action {
                    MenuAction::ToggleActive => Command::ToggleActive,
                    MenuAction::TogglePresentation => Command::TogglePresentation,
                    MenuAction::ScaleUp => Command::ScaleUp,
                    MenuAction::ScaleDown => Command::ScaleDown,
                    MenuAction::SetScale(pct) => Command::SetScale(pct),
                    MenuAction::SetSound(preset) => Command::SetSound(preset),
                    MenuAction::SetVolume(volume) => Command::SetVolume(volume),
                    MenuAction::Quit => Command::Quit,
                };
                self.command(command, platform);
            }
            Event::MouseButton(_) => {}
            Event::SettingsLoaded(settings) => {
                self.state.apply(Command::SettingsLoaded(settings));
                platform.emit(Effect::StateChanged);
            }
            Event::SettingsLoadFailed(message) | Event::SettingsSaveFailed(message) => {
                platform.emit(Effect::ReportError(message));
            }
            Event::StartupFailed(message) => platform.emit(Effect::ReportError(message)),
            Event::RenderFailed(message) => {
                platform.emit(Effect::ReportError(message));
                self.shutdown(platform, true);
            }
            Event::MessageLoopFailed => {
                platform.emit(Effect::ReportError("GetMessage failed".to_string()));
                self.shutdown(platform, true);
            }
            Event::Shutdown => self.shutdown(platform, false),
        }
    }

    fn command(&mut self, command: Command, platform: &mut dyn Platform) {
        if matches!(command, Command::Quit) {
            self.shutdown(platform, true);
            return;
        }

        let persists = matches!(
            command,
            Command::ScaleUp
                | Command::ScaleDown
                | Command::SetScale(_)
                | Command::SetSound(_)
                | Command::SetVolume(_)
        );
        self.state.apply(command);
        platform.emit(Effect::StateChanged);
        if persists {
            platform.emit(Effect::SaveSettings(self.state.settings()));
        }
    }

    fn shutdown(&mut self, platform: &mut dyn Platform, post_quit: bool) {
        if post_quit {
            platform.emit(Effect::PostQuit);
        }
        platform.emit(Effect::StopRenderThread);
        platform.emit(Effect::RemoveMouseHook);
        platform.emit(Effect::RestoreSystemCursors);
        self.quit = true;
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::overlay_state::OverlayState;
    use crate::platform::{Effect, Event, FakePlatform, Hotkey, MenuAction, MouseButtonEvent};
    use crate::settings::{Settings, SoundPreset};
    use std::sync::Arc;

    fn run(events: Vec<Event>) -> (App, Vec<Effect>) {
        let mut platform = FakePlatform::new(events);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.run(&mut platform);
        (app, platform.effects)
    }

    #[test]
    fn start_installs_hook_and_starts_render_thread() {
        let (_, effects) = run(vec![Event::Start]);
        assert_eq!(
            effects,
            vec![
                Effect::LoadSettings,
                Effect::RegisterHotkeys,
                Effect::InstallMouseHook,
                Effect::HideSystemCursors,
                Effect::StartRenderThread,
                Effect::StopRenderThread,
                Effect::RemoveMouseHook,
                Effect::RestoreSystemCursors,
            ]
        );
    }

    #[test]
    fn start_loads_settings_before_startup_effects() {
        let (_, effects) = run(vec![Event::Start]);
        assert_eq!(effects.first(), Some(&Effect::LoadSettings));
    }

    #[test]
    fn settings_loaded_applies_without_saving() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(
            Event::SettingsLoaded(Settings {
                scale_pct: 175,
                sound: SoundPreset::Bell,
                volume: 25,
            }),
            &mut platform,
        );
        assert_eq!(app.state().scale_pct, 175);
        assert_eq!(app.state().sound, SoundPreset::Bell);
        assert_eq!(app.state().volume, 25);
        assert_eq!(platform.effects, vec![Effect::StateChanged]);
    }

    #[test]
    fn scale_changes_save_a_full_snapshot() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(Event::Hotkey(Hotkey::ScaleUp), &mut platform);
        assert!(platform.effects.contains(&Effect::SaveSettings(Settings {
            scale_pct: 110,
            ..Default::default()
        })));
        assert!(platform.effects.contains(&Effect::StateChanged));
    }

    #[test]
    fn sound_and_volume_selections_change_state_and_save() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(
            Event::Menu(MenuAction::SetSound(SoundPreset::Pop)),
            &mut platform,
        );
        app.handle(Event::Menu(MenuAction::SetVolume(250)), &mut platform);
        assert_eq!(app.state().sound, SoundPreset::Pop);
        assert_eq!(app.state().volume, 100);
        assert_eq!(
            platform
                .effects
                .iter()
                .filter(|effect| **effect == Effect::StateChanged)
                .count(),
            2
        );
        assert!(platform.effects.contains(&Effect::SaveSettings(Settings {
            sound: SoundPreset::Pop,
            volume: 100,
            ..Default::default()
        })));
    }

    #[test]
    fn settings_failures_are_reported_without_shutdown() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(
            Event::SettingsLoadFailed("bad file".to_string()),
            &mut platform,
        );
        assert!(!app.is_quit());
        assert_eq!(
            platform.effects,
            vec![Effect::ReportError("bad file".to_string())]
        );

        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(
            Event::SettingsSaveFailed("save failed".to_string()),
            &mut platform,
        );
        assert!(!app.is_quit());
        assert_eq!(
            platform.effects,
            vec![Effect::ReportError("save failed".to_string())]
        );
    }

    #[test]
    fn hotkey_and_menu_toggle_active_identically() {
        let (by_hotkey, hotkey_effects) = run(vec![Event::Hotkey(Hotkey::ToggleActive)]);
        let (by_menu, menu_effects) = run(vec![Event::Menu(MenuAction::ToggleActive)]);
        assert_eq!(by_hotkey.state(), by_menu.state());
        assert_eq!(hotkey_effects, menu_effects);
    }

    #[test]
    fn scale_steps_clamp_and_presets_apply() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        for _ in 0..20 {
            app.handle(Event::Hotkey(Hotkey::ScaleDown), &mut platform);
        }
        assert_eq!(app.state().scale_pct, 50);
        for _ in 0..30 {
            app.handle(Event::Hotkey(Hotkey::ScaleUp), &mut platform);
        }
        assert_eq!(app.state().scale_pct, 200);
        app.handle(Event::Menu(MenuAction::SetScale(135)), &mut platform);
        assert_eq!(app.state().scale_pct, 135);
    }

    #[test]
    fn presentation_toggles_from_both_sources() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(Event::Hotkey(Hotkey::TogglePresentation), &mut platform);
        assert!(app.state().presentation);
        app.handle(Event::Menu(MenuAction::TogglePresentation), &mut platform);
        assert!(!app.state().presentation);
    }

    #[test]
    fn quit_posts_quit_and_cleans_up() {
        let (app, effects) = run(vec![Event::Menu(MenuAction::Quit), Event::Start]);
        assert!(app.is_quit());
        assert!(effects.contains(&Effect::PostQuit));
        assert!(effects.contains(&Effect::StopRenderThread));
        assert!(effects.contains(&Effect::RemoveMouseHook));
        assert!(effects.contains(&Effect::RestoreSystemCursors));
        assert!(!effects.contains(&Effect::StartRenderThread));
    }

    #[test]
    fn render_failure_shuts_down_cleanly() {
        let (app, effects) = run(vec![Event::RenderFailed("DIB failed".to_string())]);
        assert!(app.is_quit());
        assert!(effects.contains(&Effect::ReportError("DIB failed".to_string())));
        assert!(effects.contains(&Effect::PostQuit));
        assert!(effects.contains(&Effect::RestoreSystemCursors));
    }

    #[test]
    fn shutdown_restores_cursors_exactly_once() {
        let (_, effects) = run(vec![]);
        assert_eq!(
            effects
                .iter()
                .filter(|effect| **effect == Effect::RestoreSystemCursors)
                .count(),
            1
        );
    }

    #[test]
    fn presentation_suppresses_clicks_only_while_active() {
        let (app, _) = run(vec![
            Event::Menu(MenuAction::TogglePresentation),
            Event::MouseButton(MouseButtonEvent::LeftDown),
        ]);
        assert!(app.state().suppresses_clicks());

        let (app, _) = run(vec![
            Event::Menu(MenuAction::TogglePresentation),
            Event::Hotkey(Hotkey::ToggleActive),
            Event::MouseButton(MouseButtonEvent::Right),
        ]);
        assert!(!app.state().suppresses_clicks());
    }

    #[test]
    fn startup_failure_is_reported_through_the_seam() {
        let message = "Failed to register hotkey: Active (Ctrl+Shift+P)".to_string();
        let (_, effects) = run(vec![Event::StartupFailed(message.clone())]);
        assert!(effects.contains(&Effect::ReportError(message)));
    }

    #[test]
    fn quit_while_presenting_still_cleans_up() {
        let (app, effects) = run(vec![
            Event::Menu(MenuAction::TogglePresentation),
            Event::Hotkey(Hotkey::Quit),
        ]);
        assert!(app.is_quit());
        assert!(app.state().suppresses_clicks());
        assert!(effects.contains(&Effect::PostQuit));
        assert!(effects.contains(&Effect::StopRenderThread));
        assert!(effects.contains(&Effect::RemoveMouseHook));
        assert!(effects.contains(&Effect::RestoreSystemCursors));
    }
}
