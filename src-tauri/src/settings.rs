use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SETTINGS_SCHEMA_VERSION: u32 = 3;

fn default_schema_version() -> u32 {
    SETTINGS_SCHEMA_VERSION
}

fn default_true() -> bool {
    true
}

fn default_foreground_interval() -> u64 {
    1_000
}

fn default_background_interval() -> u64 {
    5_000
}

fn default_cleaner_poll_interval() -> u64 {
    5_000
}

fn default_standby_threshold() -> u64 {
    1_073_741_824
}

fn default_free_zero_threshold() -> u64 {
    1_073_741_824
}

fn default_cleaner_cooldown() -> u64 {
    600_000
}

fn default_cleaner_minimum_interval() -> u64 {
    300_000
}

fn default_maximum_purges_per_hour() -> u32 {
    2
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    #[serde(default = "default_true")]
    pub performance_hud_enabled: bool,
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default = "default_true")]
    pub guardian_enabled: bool,
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    #[serde(default = "default_foreground_interval")]
    pub foreground_interval_ms: u64,
    #[serde(default = "default_background_interval")]
    pub background_interval_ms: u64,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default)]
    pub compact_density: bool,
    #[serde(default)]
    pub automatic_cleaning_enabled: bool,
    #[serde(default)]
    pub cleaner_paused: bool,
    #[serde(default = "default_cleaner_poll_interval")]
    pub cleaner_poll_interval_ms: u64,
    #[serde(default = "default_standby_threshold")]
    pub standby_threshold_bytes: u64,
    #[serde(default = "default_free_zero_threshold")]
    pub free_zero_threshold_bytes: u64,
    #[serde(default = "default_cleaner_cooldown")]
    pub cleaner_cooldown_ms: u64,
    #[serde(default = "default_cleaner_minimum_interval")]
    pub cleaner_minimum_interval_ms: u64,
    #[serde(default = "default_maximum_purges_per_hour")]
    pub maximum_purges_per_hour: u32,
    #[serde(default)]
    pub cleaner_excluded_applications: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            performance_hud_enabled: true,
            schema_version: SETTINGS_SCHEMA_VERSION,
            guardian_enabled: true,
            close_to_tray: true,
            foreground_interval_ms: default_foreground_interval(),
            background_interval_ms: default_background_interval(),
            reduced_motion: false,
            compact_density: false,
            automatic_cleaning_enabled: false,
            cleaner_paused: false,
            cleaner_poll_interval_ms: default_cleaner_poll_interval(),
            standby_threshold_bytes: default_standby_threshold(),
            free_zero_threshold_bytes: default_free_zero_threshold(),
            cleaner_cooldown_ms: default_cleaner_cooldown(),
            cleaner_minimum_interval_ms: default_cleaner_minimum_interval(),
            maximum_purges_per_hour: default_maximum_purges_per_hour(),
            cleaner_excluded_applications: Vec::new(),
        }
    }
}

impl AppSettings {
    pub fn validated(mut self) -> Self {
        self.schema_version = SETTINGS_SCHEMA_VERSION;
        self.foreground_interval_ms = self.foreground_interval_ms.clamp(500, 10_000);
        self.background_interval_ms = self.background_interval_ms.clamp(1_000, 30_000);
        if self.background_interval_ms < self.foreground_interval_ms {
            self.background_interval_ms = self.foreground_interval_ms;
        }
        self.cleaner_poll_interval_ms = self.cleaner_poll_interval_ms.clamp(1_000, 60_000);
        self.standby_threshold_bytes = self
            .standby_threshold_bytes
            .clamp(64 * 1024 * 1024, 1024_u64.pow(4));
        self.free_zero_threshold_bytes = self
            .free_zero_threshold_bytes
            .clamp(64 * 1024 * 1024, 1024_u64.pow(4));
        self.cleaner_cooldown_ms = self.cleaner_cooldown_ms.clamp(60_000, 86_400_000);
        self.cleaner_minimum_interval_ms = self
            .cleaner_minimum_interval_ms
            .clamp(60_000, self.cleaner_cooldown_ms);
        self.maximum_purges_per_hour = self.maximum_purges_per_hour.clamp(1, 12);
        self.cleaner_excluded_applications = self
            .cleaner_excluded_applications
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter(|value| !value.is_empty() && value.len() <= 260)
            .collect();
        self.cleaner_excluded_applications.sort();
        self.cleaner_excluded_applications.dedup();
        self
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsEnvelope {
    pub settings: AppSettings,
    pub warning: Option<String>,
    pub path: String,
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> SettingsEnvelope {
        let path_text = self.path.display().to_string();
        let raw = match fs::read_to_string(&self.path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return SettingsEnvelope {
                    settings: AppSettings::default(),
                    warning: None,
                    path: path_text,
                };
            }
            Err(error) => {
                return SettingsEnvelope {
                    settings: AppSettings::default(),
                    warning: Some(format!(
                        "Settings could not be read; safe defaults are active: {error}"
                    )),
                    path: path_text,
                };
            }
        };

        match serde_json::from_str::<AppSettings>(&raw) {
            Ok(settings) if settings.schema_version <= SETTINGS_SCHEMA_VERSION => {
                SettingsEnvelope {
                    settings: settings.validated(),
                    warning: None,
                    path: path_text,
                }
            }
            Ok(settings) => SettingsEnvelope {
                settings: AppSettings::default(),
                warning: Some(format!(
                    "Settings schema {} is newer than supported schema {}; safe defaults are active and the file was preserved.",
                    settings.schema_version, SETTINGS_SCHEMA_VERSION
                )),
                path: path_text,
            },
            Err(error) => SettingsEnvelope {
                settings: AppSettings::default(),
                warning: Some(format!(
                    "Settings are invalid; safe defaults are active and the file was preserved: {error}"
                )),
                path: path_text,
            },
        }
    }

    pub fn save(&self, settings: &AppSettings) -> Result<AppSettings, String> {
        let settings = settings.clone().validated();
        let parent = self
            .path
            .parent()
            .ok_or_else(|| "Settings path has no parent directory.".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create settings directory: {error}"))?;

        let temp_path = self.path.with_extension("json.new");
        let payload = serde_json::to_vec_pretty(&settings)
            .map_err(|error| format!("Could not serialize settings: {error}"))?;
        let mut temp = File::create(&temp_path)
            .map_err(|error| format!("Could not create temporary settings file: {error}"))?;
        temp.write_all(&payload)
            .map_err(|error| format!("Could not write temporary settings file: {error}"))?;
        temp.sync_all()
            .map_err(|error| format!("Could not flush temporary settings file: {error}"))?;
        drop(temp);

        atomic_replace(&temp_path, &self.path).map_err(|error| {
            format!(
                "Could not atomically replace settings file (temporary copy retained at {}): {error}",
                temp_path.display()
            )
        })?;
        Ok(settings)
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> windows::core::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    use windows::core::PCWSTR;

    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(source_wide.as_ptr()),
            PCWSTR(destination_wide.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_settings_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("memflux-settings-test-{nonce}"))
            .join(name)
    }

    #[test]
    fn validation_clamps_intervals_and_preserves_relationship() {
        let settings = AppSettings {
            foreground_interval_ms: 50,
            background_interval_ms: 25,
            ..Default::default()
        }
        .validated();
        assert_eq!(settings.foreground_interval_ms, 500);
        assert_eq!(settings.background_interval_ms, 1_000);
        assert_eq!(settings.cleaner_poll_interval_ms, 5_000);
        assert!(!settings.automatic_cleaning_enabled);
    }

    #[test]
    fn cleaner_settings_are_conservative_and_normalized() {
        let settings = AppSettings {
            automatic_cleaning_enabled: true,
            cleaner_poll_interval_ms: 1,
            cleaner_cooldown_ms: 1,
            cleaner_minimum_interval_ms: 999_999_999,
            maximum_purges_per_hour: 99,
            cleaner_excluded_applications: vec![
                " OBS64.EXE ".into(),
                "obs64.exe".into(),
                String::new(),
            ],
            ..Default::default()
        }
        .validated();
        assert_eq!(settings.cleaner_poll_interval_ms, 1_000);
        assert_eq!(settings.cleaner_cooldown_ms, 60_000);
        assert_eq!(settings.cleaner_minimum_interval_ms, 60_000);
        assert_eq!(settings.maximum_purges_per_hour, 12);
        assert_eq!(settings.cleaner_excluded_applications, ["obs64.exe"]);
    }

    #[test]
    fn settings_round_trip_through_atomic_file() {
        let path = temp_settings_path("settings.json");
        let store = SettingsStore::new(path.clone());
        let expected = AppSettings {
            performance_hud_enabled: false,
            reduced_motion: true,
            background_interval_ms: 7_000,
            ..Default::default()
        };
        store.save(&expected).expect("save settings");
        let loaded = store.load();
        assert_eq!(loaded.settings, expected);
        assert!(loaded.warning.is_none());
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(path.parent().expect("test directory"));
    }

    #[test]
    fn malformed_file_is_preserved_and_defaults_are_used() {
        let path = temp_settings_path("settings.json");
        fs::create_dir_all(path.parent().expect("test directory")).expect("create directory");
        fs::write(&path, "{ definitely not json").expect("write malformed file");
        let store = SettingsStore::new(path.clone());
        let loaded = store.load();
        assert_eq!(loaded.settings, AppSettings::default());
        assert!(loaded.warning.is_some());
        assert_eq!(
            fs::read_to_string(&path).expect("read original"),
            "{ definitely not json"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir(path.parent().expect("test directory"));
    }
}
