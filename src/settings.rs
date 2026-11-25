use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub scale_pct: u32,
    pub muted: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scale_pct: 100,
            muted: false,
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

fn parse_muted(value: &str, settings: &mut Settings) {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => settings.muted = true,
        "false" => settings.muted = false,
        _ => {}
    }
}

fn format_muted(settings: &Settings) -> String {
    settings.muted.to_string()
}

const FIELDS: &[Field] = &[
    Field {
        key: "scale_pct",
        parse: parse_scale_pct,
        format: format_scale_pct,
    },
    Field {
        key: "muted",
        parse: parse_muted,
        format: format_muted,
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
    use super::Settings;
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
            muted: true,
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
            muted: false,
        }
        .save_to(&file)
        .unwrap();
        let settings = Settings {
            scale_pct: 135,
            muted: true,
        };
        settings.save_to(&file).unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), settings);
    }

    #[test]
    fn invalid_values_fall_back_to_defaults() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "scale_pct=abc\nmuted=maybe\n").unwrap();
        assert_eq!(Settings::load_from(&file).unwrap(), Settings::default());
    }

    #[test]
    fn scale_is_clamped_on_load() {
        let dir = TempDir::new();
        let file = dir.file();
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "scale_pct=999\nmuted=true\n").unwrap();
        assert_eq!(
            Settings::load_from(&file).unwrap(),
            Settings {
                scale_pct: 200,
                muted: true,
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
        fs::write(
            &file,
            "\n# a comment\nunknown=1\nscale_pct=135\n muted = TRUE \n",
        )
        .unwrap();
        assert_eq!(
            Settings::load_from(&file).unwrap(),
            Settings {
                scale_pct: 135,
                muted: true,
            }
        );
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
