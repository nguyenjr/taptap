use crate::hand_art::HandStyle;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundPreset {
    Wood,
    Click,
    Bell,
    Pop,
    JingleBell,
}

impl SoundPreset {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Wood => 0,
            Self::Click => 1,
            Self::Bell => 2,
            Self::Pop => 3,
            Self::JingleBell => 4,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Click,
            2 => Self::Bell,
            3 => Self::Pop,
            4 => Self::JingleBell,
            _ => Self::Wood,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub scale_pct: u32,
    pub hand_style: HandStyle,
    pub sound: SoundPreset,
    pub volume: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scale_pct: 100,
            hand_style: HandStyle::default(),
            sound: SoundPreset::Wood,
            volume: 100,
        }
    }
}

struct Field {
    key: &'static str,
    parse: fn(&str, &mut Settings),
    format: fn(&Settings) -> String,
}

fn parse_scale_pct(value: &str, settings: &mut Settings) {
    if let Ok(pct) = value.trim().parse::<u32>() {
        settings.scale_pct = pct.clamp(50, 200);
    }
}

fn format_scale_pct(settings: &Settings) -> String {
    settings.scale_pct.to_string()
}

fn parse_hand_style(value: &str, settings: &mut Settings) {
    if let Some(style) = HandStyle::from_id(value.trim()) {
        settings.hand_style = style;
    }
}

fn format_hand_style(settings: &Settings) -> String {
    settings.hand_style.binding().id.to_string()
}

fn parse_sound(value: &str, settings: &mut Settings) {
    settings.sound = match value.trim().to_ascii_lowercase().as_str() {
        "click" => SoundPreset::Click,
        "bell" => SoundPreset::Bell,
        "pop" => SoundPreset::Pop,
        "jingle" => SoundPreset::JingleBell,
        "wood" => SoundPreset::Wood,
        _ => return,
    };
}

fn format_sound(settings: &Settings) -> String {
    match settings.sound {
        SoundPreset::Wood => "wood",
        SoundPreset::Click => "click",
        SoundPreset::Bell => "bell",
        SoundPreset::Pop => "pop",
        SoundPreset::JingleBell => "jingle",
    }
    .to_string()
}

fn parse_volume(value: &str, settings: &mut Settings) {
    if let Ok(volume) = value.trim().parse::<u8>() {
        settings.volume = volume.clamp(0, 100);
    }
}

fn format_volume(settings: &Settings) -> String {
    settings.volume.to_string()
}

const FIELDS: &[Field] = &[
    Field {
        key: "scale_pct",
        parse: parse_scale_pct,
        format: format_scale_pct,
    },
    Field {
        key: "hand_style",
        parse: parse_hand_style,
        format: format_hand_style,
    },
    Field {
        key: "sound",
        parse: parse_sound,
        format: format_sound,
    },
    Field {
        key: "volume",
        parse: parse_volume,
        format: format_volume,
    },
];

impl Settings {
    pub fn load() -> Result<Self, String> {
        Self::load_from(&Self::path())
    }

    fn load_from(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(Self::parse(&text)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!(
                "Failed to read settings from {}: {error}",
                path.display()
            )),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }

    fn save_to(&self, path: &Path) -> Result<(), String> {
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        if let Err(error) = fs::create_dir_all(directory) {
            return Err(format!(
                "Failed to create settings directory {}: {error}",
                directory.display()
            ));
        }

        let temporary = path.with_extension("txt.tmp");
        if let Err(error) = fs::write(&temporary, self.serialize()) {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "Failed to write settings to {}: {error}",
                temporary.display()
            ));
        }
        if let Err(error) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(&temporary);
            return Err(format!(
                "Failed to replace settings at {}: {error}",
                path.display()
            ));
        }
        Ok(())
    }

    fn path() -> PathBuf {
        match std::env::var_os("APPDATA") {
            Some(appdata) if !appdata.is_empty() => {
                PathBuf::from(appdata).join("taptap").join("settings.txt")
            }
            _ => PathBuf::from("settings.txt"),
        }
    }

    fn parse(text: &str) -> Self {
        let mut settings = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            if let Some(field) = FIELDS.iter().find(|field| field.key == key) {
                (field.parse)(value, &mut settings);
            }
        }
        settings
    }

    fn serialize(&self) -> String {
        let mut text = String::new();
        for field in FIELDS {
            text.push_str(field.key);
            text.push('=');
            text.push_str(&(field.format)(self));
            text.push('\n');
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::{Settings, SoundPreset};
    use crate::hand_art::HandStyle;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT_DIR: AtomicU32 = AtomicU32::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "taptap-settings-test-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::SeqCst)
            ));
            Self { path }
        }

        fn file(&self) -> PathBuf {
            self.path.join("nested").join("settings.txt")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn missing_file_loads_defaults() {
        let dir = TempDir::new();
        assert_eq!(
            Settings::load_from(&dir.file()).unwrap(),
            Settings::default()
        );
    }

    #[test]
    fn save_creates_parent_directory_and_round_trips() {
        let dir = TempDir::new();
        let file = dir.file();
        let settings = Settings {
            scale_pct: 175,
            hand_style: HandStyle(1),
            sound: SoundPreset::Bell,
            volume: 25,
        };
        settings.save_to(&file).unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), settings);
        assert!(!file.with_extension("txt.tmp").exists());
    }

    #[test]
    fn save_overwrites_existing_file() {
        let dir = TempDir::new();
        let file = dir.file();
        Settings {
            scale_pct: 75,
            ..Default::default()
        }
        .save_to(&file)
        .unwrap();
        let settings = Settings {
            scale_pct: 135,
            sound: SoundPreset::Pop,
            volume: 0,
            ..Default::default()
        };
        settings.save_to(&file).unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), settings);
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "scale_pct=abc\nvolume=maybe\n").unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), Settings::default());
    }

    #[test]
    fn scale_is_clamped_on_load() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "scale_pct=999\n").unwrap();
        assert_eq!(
            Settings::load_from(&file).unwrap(),
            Settings {
                scale_pct: 200,
                ..Default::default()
            }
        );
        fs::write(&file, "scale_pct=1\n").unwrap();
        assert_eq!(Settings::load_from(&file).unwrap().scale_pct, 50);
    }

    #[test]
    fn unknown_keys_blank_lines_and_comments_are_ignored() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "\n# a comment\nunknown=1\nscale_pct=135\n").unwrap();
        assert_eq!(
            Settings::load_from(&file).unwrap(),
            Settings {
                scale_pct: 135,
                ..Default::default()
            }
        );
    }

    #[test]
    fn sound_and_volume_round_trip_and_clamp() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "sound=Bell\nvolume=250\n").unwrap();
        assert_eq!(
            Settings::load_from(&file).unwrap(),
            Settings {
                sound: SoundPreset::Bell,
                volume: 100,
                ..Default::default()
            }
        );
        fs::write(&file, "sound=unknown\nvolume=abc\n").unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), Settings::default());
        assert_eq!(SoundPreset::from_u8(3), SoundPreset::Pop);
        assert_eq!(SoundPreset::Wood.as_u8(), 0);
    }

    #[test]
    fn hand_style_and_jingle_sound_round_trip() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "hand_style=gingerbread\nsound=jingle\n").unwrap();
        let settings = Settings::load_from(&file).unwrap();
        assert_eq!(settings.hand_style, HandStyle(1));
        assert_eq!(settings.sound, SoundPreset::JingleBell);
        settings.save_to(&file).unwrap();
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "scale_pct=100\nhand_style=gingerbread\nsound=jingle\nvolume=100\n"
        );
        fs::write(&file, "hand_style=unknown\n").unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), Settings::default());
        assert_eq!(SoundPreset::from_u8(4), SoundPreset::JingleBell);
    }

    #[test]
    fn unreadable_file_is_reported() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(&file).unwrap();
        let error = Settings::load_from(&file).unwrap_err();
        assert!(error.starts_with("Failed to read settings from "));
    }
}
