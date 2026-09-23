use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub library_root: Option<String>,
    pub keep_removed_managed_files: bool,
    pub sync_on_startup: bool,
    pub sync_interval_minutes: Option<i64>,
    pub acquisition_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            library_root: None,
            keep_removed_managed_files: true,
            sync_on_startup: false,
            sync_interval_minutes: None,
            acquisition_enabled: false,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self
            .sync_interval_minutes
            .is_some_and(|minutes| minutes <= 0)
        {
            return Err("sync interval must be greater than zero");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_are_safe() {
        let settings = AppSettings::default();

        assert!(settings.keep_removed_managed_files);
        assert!(!settings.sync_on_startup);
        assert!(!settings.acquisition_enabled);
        assert_eq!(settings.sync_interval_minutes, None);
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn sync_interval_must_be_positive() {
        let settings = AppSettings {
            sync_interval_minutes: Some(0),
            ..AppSettings::default()
        };

        assert_eq!(
            settings.validate(),
            Err("sync interval must be greater than zero")
        );
    }
}
