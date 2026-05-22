use crate::client::HttpConfig;
use crate::model::{AudioQuality, Partner};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PianobarConfig {
    pub username: Option<String>,
    pub password: Option<String>,
    pub password_command: Option<String>,
    pub rec: Option<PathBuf>,
    pub audio_quality: AudioQuality,
    pub autostart_station: Option<String>,
    pub rpc_host: String,
    pub rpc_tls_port: u16,
    pub partner_user: String,
    pub partner_password: String,
    pub device: String,
    pub encrypt_password: String,
    pub decrypt_password: String,
    pub max_retry: u32,
}

impl Default for PianobarConfig {
    fn default() -> Self {
        let partner = Partner::android_default();
        Self {
            username: None,
            password: None,
            password_command: None,
            rec: None,
            audio_quality: AudioQuality::High,
            autostart_station: None,
            rpc_host: "tuner.pandora.com".to_string(),
            rpc_tls_port: 443,
            partner_user: partner.user,
            partner_password: partner.password,
            device: partner.device,
            encrypt_password: "6#26FRL$ZWD".to_string(),
            decrypt_password: "R=U!LH$O2B#".to_string(),
            max_retry: 5,
        }
    }
}

impl PianobarConfig {
    pub fn read_path(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn parse(content: &str) -> Result<Self, ConfigError> {
        let mut config = Self::default();
        for line in content.lines() {
            let Some((key, value)) = parse_line(line) else {
                continue;
            };

            match key {
                "user" => config.username = Some(value.to_string()),
                "password" => config.password = Some(value.to_string()),
                "password_command" => config.password_command = Some(value.to_string()),
                "rec" => config.rec = Some(expand_tilde(value)),
                "audio_quality" => {
                    config.audio_quality = match value {
                        "low" => AudioQuality::Low,
                        "medium" => AudioQuality::Medium,
                        "high" => AudioQuality::High,
                        _ => return Err(ConfigError::InvalidValue("audio_quality".to_string())),
                    };
                }
                "autostart_station" => config.autostart_station = Some(value.to_string()),
                "rpc_host" => config.rpc_host = value.to_string(),
                "rpc_tls_port" => {
                    config.rpc_tls_port = value
                        .parse()
                        .map_err(|_| ConfigError::InvalidValue("rpc_tls_port".to_string()))?;
                }
                "partner_user" => config.partner_user = value.to_string(),
                "partner_password" => config.partner_password = value.to_string(),
                "device" => config.device = value.to_string(),
                "encrypt_password" => config.encrypt_password = value.to_string(),
                "decrypt_password" => config.decrypt_password = value.to_string(),
                "max_retry" => {
                    config.max_retry = value
                        .parse()
                        .map_err(|_| ConfigError::InvalidValue("max_retry".to_string()))?;
                }
                _ => {}
            }
        }
        Ok(config)
    }

    pub fn partner(&self) -> Partner {
        Partner {
            user: self.partner_user.clone(),
            password: self.partner_password.clone(),
            device: self.device.clone(),
            auth_token: None,
            id: None,
        }
    }

    pub fn http_config(&self) -> HttpConfig {
        HttpConfig {
            rpc_host: self.rpc_host.clone(),
            rpc_tls_port: self.rpc_tls_port,
            rpc_plain_port: 80,
            max_retry: self.max_retry,
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file")]
    Io(#[from] std::io::Error),
    #[error("invalid value for config key `{0}`")]
    InvalidValue(String),
}

fn parse_line(line: &str) -> Option<(&str, &str)> {
    let trimmed_start = line.trim_start();
    if trimmed_start.is_empty() || trimmed_start.starts_with('#') {
        return None;
    }

    let (key, value) = trimmed_start.split_once('=')?;
    let key = key.trim_end();
    let value = value.strip_prefix(char::is_whitespace).unwrap_or(value);
    Some((key, value.trim_end_matches(['\r', '\n'])))
}

fn expand_tilde(value: &str) -> PathBuf {
    if value == "~" {
        home_dir().unwrap_or_else(|| PathBuf::from(value))
    } else if let Some(rest) = value.strip_prefix("~/") {
        home_dir()
            .map(|home| home.join(rest))
            .unwrap_or_else(|| PathBuf::from(value))
    } else {
        PathBuf::from(value)
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_keys_and_ignores_comments() {
        let config = PianobarConfig::parse(
            r#"
            #user = ignored
            user = user@example.test
            password = secret
            rec=~/Music/Pandora
            audio_quality = medium
            autostart_station = station-token
            rpc_host = internal-tuner.pandora.com
            rpc_tls_port = 444
            partner_user = pandora one
            partner_password = partner-secret
            device = D01
            encrypt_password = enc
            decrypt_password = dec
            max_retry = 9
            unknown_key = ignored
            "#,
        )
        .unwrap();

        assert_eq!(config.username.as_deref(), Some("user@example.test"));
        assert_eq!(config.password.as_deref(), Some("secret"));
        assert_eq!(config.audio_quality, AudioQuality::Medium);
        assert_eq!(config.autostart_station.as_deref(), Some("station-token"));
        assert_eq!(config.rpc_host, "internal-tuner.pandora.com");
        assert_eq!(config.rpc_tls_port, 444);
        assert_eq!(config.partner_user, "pandora one");
        assert_eq!(config.partner_password, "partner-secret");
        assert_eq!(config.device, "D01");
        assert_eq!(config.encrypt_password, "enc");
        assert_eq!(config.decrypt_password, "dec");
        assert_eq!(config.max_retry, 9);
    }

    #[test]
    fn preserves_one_leading_value_space_like_c_parser() {
        let config = PianobarConfig::parse("password =  leading-space\n").unwrap();

        assert_eq!(config.password.as_deref(), Some(" leading-space"));
    }

    #[test]
    fn rejects_invalid_quality() {
        assert!(matches!(
            PianobarConfig::parse("audio_quality = huge\n"),
            Err(ConfigError::InvalidValue(key)) if key == "audio_quality"
        ));
    }
}
