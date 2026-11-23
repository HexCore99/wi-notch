use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process,
};

use serde::{Deserialize, Serialize};

use crate::prayer::DHAKA_ID;

pub const MIN_IDLE_COLLAPSE_DELAY_SECS: u32 = 5;
pub const MAX_IDLE_COLLAPSE_DELAY_SECS: u32 = 300;
pub const DEFAULT_POMODORO_LABEL: &str = "Study (DSA)";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Background {
    #[default]
    Black,
    Charcoal,
}

impl Background {
    pub fn color(self) -> u32 {
        match self {
            Self::Black => 0x000000,
            Self::Charcoal => 0x202020,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Corners {
    #[default]
    Rounded,
    Square,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub background: Background,
    pub corners: Corners,
    pub top_gap: u32,
    pub offset: u32,
    pub monitor: Option<String>,
    pub auto_minimal: bool,
    pub always_minimal: bool,
    pub idle_collapse_delay_secs: u32,
    pub prayer_city_id: u32,
    pub pomodoro_labels: Vec<String>,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            background: Background::Black,
            corners: Corners::Rounded,
            top_gap: 0,
            offset: 5,
            monitor: None,
            auto_minimal: false,
            always_minimal: false,
            idle_collapse_delay_secs: 20,
            prayer_city_id: DHAKA_ID,
            pomodoro_labels: vec![DEFAULT_POMODORO_LABEL.to_owned()],
        }
    }
}

impl AppearanceSettings {
    pub fn idle_collapse_seconds(&self) -> u32 {
        self.idle_collapse_delay_secs
            .clamp(MIN_IDLE_COLLAPSE_DELAY_SECS, MAX_IDLE_COLLAPSE_DELAY_SECS)
    }

    pub fn load() -> Self {
        settings_directory()
            .as_deref()
            .map(Self::load_from)
            .unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let directory =
            settings_directory().ok_or_else(|| io::Error::other("LOCALAPPDATA is unavailable"))?;
        self.save_to(&directory)
    }

    fn load_from(directory: &Path) -> Self {
        fs::read(directory.join("settings.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save_to(&self, directory: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self)?;
        fs::create_dir_all(directory)?;
        let temp = directory.join(format!("settings.{}.tmp", process::id()));
        let result = fs::write(&temp, bytes)
            .and_then(|()| fs::rename(&temp, directory.join("settings.json")));
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
}

fn settings_directory() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .map(|path| path.join("wi-notch"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_malformed_files_use_defaults() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        let defaults = AppearanceSettings {
            background: Background::Black,
            corners: Corners::Rounded,
            top_gap: 0,
            offset: 5,
            monitor: None,
            auto_minimal: false,
            always_minimal: false,
            idle_collapse_delay_secs: 20,
            prayer_city_id: DHAKA_ID,
            pomodoro_labels: vec![DEFAULT_POMODORO_LABEL.to_owned()],
        };

        assert_eq!(AppearanceSettings::load_from(directory.path()), defaults);
        fs::write(directory.path().join("settings.json"), "invalid JSON")?;
        assert_eq!(AppearanceSettings::load_from(directory.path()), defaults);
        Ok(())
    }

    #[test]
    fn older_settings_get_the_new_defaults() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        fs::write(
            directory.path().join("settings.json"),
            r#"{"background":"charcoal","corners":"square"}"#,
        )?;

        let settings = AppearanceSettings::load_from(directory.path());
        assert_eq!(settings.background, Background::Charcoal);
        assert_eq!(settings.corners, Corners::Square);
        assert_eq!(settings.top_gap, 0);
        assert_eq!(settings.offset, 5);
        assert_eq!(settings.monitor, None);
        assert!(!settings.auto_minimal);
        assert!(!settings.always_minimal);
        assert_eq!(settings.idle_collapse_seconds(), 20);
        assert_eq!(settings.prayer_city_id, DHAKA_ID);
        assert_eq!(settings.pomodoro_labels, [DEFAULT_POMODORO_LABEL]);
        Ok(())
    }

    #[test]
    fn explicitly_saved_gap_survives_new_defaults() -> io::Result<()> {
        let directory = tempfile::tempdir()?;
        fs::write(directory.path().join("settings.json"), r#"{"top_gap":100}"#)?;
        assert_eq!(AppearanceSettings::load_from(directory.path()).top_gap, 100);
        Ok(())
    }

    #[test]
    fn saved_choices_are_loaded_and_can_be_overwritten() -> io::Result<()> {
        let temp = tempfile::tempdir()?;
        let directory = temp.path().join("wi-notch");
        let settings = AppearanceSettings {
            background: Background::Charcoal,
            corners: Corners::Square,
            top_gap: 125,
            offset: 15,
            monitor: Some("DISPLAY2".into()),
            auto_minimal: true,
            always_minimal: true,
            idle_collapse_delay_secs: 45,
            prayer_city_id: 5_128_581,
            pomodoro_labels: vec![DEFAULT_POMODORO_LABEL.to_owned(), "Work".to_owned()],
        };

        settings.save_to(&directory)?;
        assert_eq!(AppearanceSettings::load_from(&directory), settings);

        let defaults = AppearanceSettings::default();
        defaults.save_to(&directory)?;
        assert_eq!(AppearanceSettings::load_from(&directory), defaults);
        Ok(())
    }
}
