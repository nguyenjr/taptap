use crate::platform::{Effect, Event, Hotkey, MenuAction, Platform};
use crate::settings::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppState {
    pub active: bool,
    pub presentation: bool,
    pub muted: bool,
    pub scale_pct: u32,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            active: true,
            presentation: false,
            muted: false,
            scale_pct: 100,
        }
    }
}

#[derive(Clone, Copy)]
enum Command {
    ToggleActive,
    TogglePresentation,
    ToggleMute,
    ScaleUp,
    ScaleDown,
    SetScale(u32),
    Quit,
}

pub struct App {
    state: AppState,
    quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            state: AppState::default(),
            quit: false,
        }
    }

    #[cfg(test)]
    pub fn state(&self) -> AppState {
        self.state
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
                    MenuAction::ToggleMute => Command::ToggleMute,
                    MenuAction::ScaleUp => Command::ScaleUp,
                    MenuAction::ScaleDown => Command::ScaleDown,
                    MenuAction::SetScale(pct) => Command::SetScale(pct),
                    MenuAction::Quit => Command::Quit,
                };
                self.command(command, platform);
            }
            Event::MouseButton(_) => {
                platform.emit(Effect::SuppressClick(self.suppression()));
            }
            Event::SettingsLoaded(settings) => {
                self.state.scale_pct = settings.scale_pct;
                self.state.muted = settings.muted;
                platform.emit(Effect::ScaleChanged(settings.scale_pct));
                platform.emit(Effect::MuteChanged(settings.muted));
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

    fn suppression(&self) -> bool {
        self.state.active && self.state.presentation
    }

    fn command(&mut self, command: Command, platform: &mut dyn Platform) {
        match command {
            Command::ToggleActive => {
                self.state.active = !self.state.active;
                platform.emit(Effect::ActiveChanged(self.state.active));
                platform.emit(Effect::SuppressClick(self.suppression()));
                if self.state.active {
                    platform.emit(Effect::HideSystemCursors);
                    platform.emit(Effect::ShowOverlay);
                } else {
                    platform.emit(Effect::RestoreSystemCursors);
                    platform.emit(Effect::HideOverlay);
                }
            }
            Command::TogglePresentation => {
                self.state.presentation = !self.state.presentation;
                platform.emit(Effect::PresentationChanged(self.state.presentation));
                platform.emit(Effect::SuppressClick(self.suppression()));
            }
            Command::ToggleMute => {
                self.state.muted = !self.state.muted;
                platform.emit(Effect::MuteChanged(self.state.muted));
                platform.emit(Effect::SaveSettings(self.settings()));
            }
            Command::ScaleUp | Command::ScaleDown => {
                let delta = match command {
                    Command::ScaleUp => 10,
                    _ => -10,
                };
                let next = (self.state.scale_pct as i32 + delta).clamp(50, 200) as u32;
                self.state.scale_pct = next;
                platform.emit(Effect::ScaleChanged(next));
                platform.emit(Effect::SaveSettings(self.settings()));
            }
            Command::SetScale(pct) => {
                self.state.scale_pct = pct;
                platform.emit(Effect::ScaleChanged(pct));
                platform.emit(Effect::SaveSettings(self.settings()));
            }
            Command::Quit => self.shutdown(platform, true),
        }
    }

    fn settings(&self) -> Settings {
        Settings {
            scale_pct: self.state.scale_pct,
            muted: self.state.muted,
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
    use crate::platform::{Effect, Event, FakePlatform, Hotkey, MenuAction, MouseButtonEvent};
    use crate::settings::Settings;

    fn run(events: Vec<Event>) -> (App, Vec<Effect>) {
        let mut platform = FakePlatform::new(events);
        let mut app = App::new();
        app.run(&mut platform);
        (app, platform.effects)
    }

    fn last_suppression(effects: &[Effect]) -> Option<bool> {
        effects.iter().rev().find_map(|effect| match effect {
            Effect::SuppressClick(suppress) => Some(*suppress),
            _ => None,
        })
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
    fn settings_loaded_applies_through_existing_effects_without_saving() {
        let mut app = App::new();
        let mut platform = FakePlatform::new(vec![]);
        app.handle(
            Event::SettingsLoaded(Settings {
                scale_pct: 175,
                muted: true,
            }),
            &mut platform,
        );
        assert_eq!(app.state().scale_pct, 175);
        assert!(app.state().muted);
        assert_eq!(
            platform.effects,
            vec![Effect::ScaleChanged(175), Effect::MuteChanged(true)]
        );
    }

    #[test]
    fn scale_and_mute_changes_save_a_full_snapshot() {
        let mut app = App::new();
        let mut platform = FakePlatform::new(vec![]);
        app.handle(Event::Hotkey(Hotkey::ScaleUp), &mut platform);
        app.handle(Event::Menu(MenuAction::ToggleMute), &mut platform);
        assert!(platform.effects.contains(&Effect::SaveSettings(Settings {
            scale_pct: 110,
            muted: false,
        })));
        assert!(platform.effects.contains(&Effect::SaveSettings(Settings {
            scale_pct: 110,
            muted: true,
        })));
    }

    #[test]
    fn settings_failures_are_reported_without_shutdown() {
        let mut app = App::new();
        let mut platform = FakePlatform::new(vec![]);
        app.handle(
            Event::SettingsLoadFailed("bad file".to_string()),
            &mut platform,
        );
        assert!(!app.is_quit());
        assert_eq!(
            platform.effects,
            vec![Effect::ReportError("bad file".to_string())]
        );

        let mut app = App::new();
        let mut platform = FakePlatform::new(vec![]);
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
        let by_hotkey = run(vec![Event::Hotkey(Hotkey::ToggleActive)]).1;
        let by_menu = run(vec![Event::Menu(MenuAction::ToggleActive)]).1;
        assert_eq!(by_hotkey, by_menu);
    }

    #[test]
    fn scale_steps_clamp_and_presets_apply() {
        let mut app = App::new();
        let mut platform = FakePlatform::new(vec![]);
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
    fn presentation_and_mute_toggle_from_both_sources() {
        let mut app = App::new();
        let mut platform = FakePlatform::new(vec![]);
        app.handle(Event::Hotkey(Hotkey::TogglePresentation), &mut platform);
        app.handle(Event::Menu(MenuAction::ToggleMute), &mut platform);
        assert!(app.state().presentation);
        assert!(app.state().muted);
        app.handle(Event::Menu(MenuAction::TogglePresentation), &mut platform);
        app.handle(Event::Menu(MenuAction::ToggleMute), &mut platform);
        assert!(!app.state().presentation);
        assert!(!app.state().muted);
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
        let (_, effects) = run(vec![
            Event::Menu(MenuAction::TogglePresentation),
            Event::MouseButton(MouseButtonEvent::LeftDown),
        ]);
        assert_eq!(last_suppression(&effects), Some(true));

        let (_, effects) = run(vec![
            Event::Menu(MenuAction::TogglePresentation),
            Event::Hotkey(Hotkey::ToggleActive),
            Event::MouseButton(MouseButtonEvent::Right),
        ]);
        assert_eq!(last_suppression(&effects), Some(false));
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
        assert_eq!(last_suppression(&effects), Some(true));
        assert!(effects.contains(&Effect::PostQuit));
        assert!(effects.contains(&Effect::StopRenderThread));
        assert!(effects.contains(&Effect::RemoveMouseHook));
        assert!(effects.contains(&Effect::RestoreSystemCursors));
    }
}
