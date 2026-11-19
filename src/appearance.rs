use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process,
};

use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default)]
pub struct AppearanceSettings {
    pub background: Background,
    pub corners: Corners,
}

impl AppearanceSettings {
    pub fn load() -> Self {
        settings_directory()
            .as_deref()
            .map(Self::load_from)
            .unwrap_or_default()
    }

    pub fn save(self) -> io::Result<()> {
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

    fn save_to(self, directory: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec_pretty(&self)?;
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
        };

        assert_eq!(AppearanceSettings::load_from(directory.path()), defaults);
        fs::write(directory.path().join("settings.json"), "invalid JSON")?;
        assert_eq!(AppearanceSettings::load_from(directory.path()), defaults);
        Ok(())
    }

    #[test]
    fn saved_choices_are_loaded_and_can_be_overwritten() -> io::Result<()> {
        let temp = tempfile::tempdir()?;
        let directory = temp.path().join("wi-notch");
        let settings = AppearanceSettings {
            background: Background::Charcoal,
            corners: Corners::Square,
        };

        settings.save_to(&directory)?;
        assert_eq!(AppearanceSettings::load_from(&directory), settings);

        let defaults = AppearanceSettings::default();
        defaults.save_to(&directory)?;
        assert_eq!(AppearanceSettings::load_from(&directory), defaults);
        Ok(())
    }
}
