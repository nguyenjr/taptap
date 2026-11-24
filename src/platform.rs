#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hotkey {
    ToggleActive,
    TogglePresentation,
    ScaleUp,
    ScaleDown,
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAction {
    ToggleActive,
    TogglePresentation,
    ToggleMute,
    ScaleUp,
    ScaleDown,
    SetScale(u32),
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Start,
    Hotkey(Hotkey),
    Menu(MenuAction),
    RenderFailed(String),
    StartupFailed(String),
    MessageLoopFailed,
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    InstallMouseHook,
    RemoveMouseHook,
    StartRenderThread,
    StopRenderThread,
    HideSystemCursors,
    RestoreSystemCursors,
    ActiveChanged(bool),
    PresentationChanged(bool),
    MuteChanged(bool),
    ScaleChanged(u32),
    ShowOverlay,
    HideOverlay,
    PostQuit,
    ReportError(String),
}

pub trait Platform {
    fn next_event(&mut self) -> Option<Event>;
    fn emit(&mut self, effect: Effect);
}

#[cfg(test)]
#[derive(Default)]
pub struct FakePlatform {
    pub events: std::collections::VecDeque<Event>,
    pub effects: Vec<Effect>,
}

#[cfg(test)]
impl FakePlatform {
    pub fn new(events: Vec<Event>) -> Self {
        Self {
            events: events.into(),
            effects: Vec::new(),
        }
    }
}

#[cfg(test)]
impl Platform for FakePlatform {
    fn next_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    fn emit(&mut self, effect: Effect) {
        self.effects.push(effect);
    }
}
