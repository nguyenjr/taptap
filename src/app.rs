use crate::overlay_state::{Command, OverlayState};
use crate::platform::{Effect, Event, Hotkey, MenuAction, NotifyKind, Platform};
use std::collections::HashSet;
use std::sync::Arc;

pub struct App {
    state: Arc<OverlayState>,
    quit: bool,
    seen_failures: HashSet<String>,
}

impl App {
    pub fn new(state: Arc<OverlayState>) -> Self {
        Self {
            state,
            quit: false,
            seen_failures: HashSet::new(),
        }
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
                    MenuAction::SetHand(style) => Command::SetHand(style),
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
                self.report_failure(message, platform);
            }
            Event::StartupFailed(messages) => self.report_failures(messages, platform),
            Event::RenderFailed(message) => {
                self.report_failure(message, platform);
                self.shutdown(platform, true);
            }
            Event::MessageLoopFailed => {
                self.report_failure("The message loop failed.".to_string(), platform);
                self.shutdown(platform, true);
            }
            Event::Shutdown => self.shutdown(platform, false),
        }
    }

    fn report_failure(&mut self, text: String, platform: &mut dyn Platform) {
        self.report_failures(vec![text], platform);
    }

    fn report_failures(&mut self, messages: Vec<String>, platform: &mut dyn Platform) {
        let lines: Vec<String> = messages
            .into_iter()
            .filter(|text| note_if_new(NotifyKind::Failure, text, &mut self.seen_failures))
            .collect();
        if !lines.is_empty() {
            platform.emit(Effect::Notify {
                kind: NotifyKind::Failure,
                text: lines.join("\n"),
            });
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
                | Command::SetHand(_)
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

fn note_if_new(kind: NotifyKind, text: &str, seen: &mut HashSet<String>) -> bool {
    match kind {
        NotifyKind::Info => true,
        NotifyKind::Failure => seen.insert(text.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::{note_if_new, App};
    use crate::hand_art::HandStyle;
    use crate::overlay_state::OverlayState;
    use crate::platform::{
        Effect, Event, FakePlatform, Hotkey, MenuAction, MouseButtonEvent, NotifyKind,
    };
    use crate::settings::{Settings, SoundPreset};
    use std::collections::HashSet;
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
                hand_style: HandStyle::GINGERBREAD,
                sound: SoundPreset::Bell,
                volume: 25,
            }),
            &mut platform,
        );
        assert_eq!(app.state().scale_pct, 175);
        assert_eq!(app.state().hand_style, HandStyle::GINGERBREAD);
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
    fn hand_style_selection_changes_state_and_saves() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(
            Event::Menu(MenuAction::SetHand(HandStyle::GINGERBREAD)),
            &mut platform,
        );
        assert_eq!(app.state().hand_style, HandStyle::GINGERBREAD);
        assert!(platform.effects.contains(&Effect::StateChanged));
        assert!(platform.effects.contains(&Effect::SaveSettings(Settings {
            hand_style: HandStyle::GINGERBREAD,
            ..Default::default()
        })));
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
            vec![Effect::Notify {
                kind: NotifyKind::Failure,
                text: "bad file".to_string()
            }]
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
            vec![Effect::Notify {
                kind: NotifyKind::Failure,
                text: "save failed".to_string()
            }]
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
        assert!(effects.contains(&Effect::Notify {
            kind: NotifyKind::Failure,
            text: "DIB failed".to_string()
        }));
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
        let (_, effects) = run(vec![Event::StartupFailed(vec![message.clone()])]);
        assert!(effects.contains(&Effect::Notify {
            kind: NotifyKind::Failure,
            text: message
        }));
    }

    #[test]
    fn startup_failures_are_aggregated_into_one_message() {
        let (_, effects) = run(vec![Event::StartupFailed(vec![
            "First failure.".to_string(),
            "Second failure.".to_string(),
        ])]);
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, Effect::Notify { .. }))
                .count(),
            1
        );
        assert!(effects.contains(&Effect::Notify {
            kind: NotifyKind::Failure,
            text: "First failure.\nSecond failure.".to_string()
        }));
    }

    #[test]
    fn identical_failures_are_reported_once_per_session() {
        let mut platform = FakePlatform::new(vec![]);
        let mut app = App::new(Arc::new(OverlayState::new()));
        app.handle(
            Event::SettingsSaveFailed("Save failed.".to_string()),
            &mut platform,
        );
        app.handle(
            Event::SettingsLoadFailed("Save failed.".to_string()),
            &mut platform,
        );
        app.handle(
            Event::SettingsSaveFailed("A different failure.".to_string()),
            &mut platform,
        );
        assert_eq!(
            platform.effects,
            vec![
                Effect::Notify {
                    kind: NotifyKind::Failure,
                    text: "Save failed.".to_string()
                },
                Effect::Notify {
                    kind: NotifyKind::Failure,
                    text: "A different failure.".to_string()
                },
            ]
        );
    }

    #[test]
    fn informational_messages_are_not_deduplicated() {
        let mut seen = HashSet::new();
        assert!(note_if_new(NotifyKind::Info, "Notice.", &mut seen));
        assert!(note_if_new(NotifyKind::Info, "Notice.", &mut seen));
        assert!(note_if_new(NotifyKind::Failure, "Failure.", &mut seen));
        assert!(!note_if_new(NotifyKind::Failure, "Failure.", &mut seen));
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
