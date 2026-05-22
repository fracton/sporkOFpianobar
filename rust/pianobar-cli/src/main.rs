use pianobar_core::{
    download_song_assets, AudioQuality, ClientError, ConfigError, DownloadOptions, PandoraClient,
    PianobarConfig, Session, StorageError,
};
use std::env;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::process::{Command as ProcessCommand, ExitCode};

const CONFIG_ENV: &str = "PIANOBAR_CONFIG";
const USER_ENV: &str = "PIANOBAR_USERNAME";
const PASSWORD_ENV: &str = "PIANOBAR_PASSWORD";
const DEFAULT_DOWNLOAD_DIR: &str = "pianobar-rs-downloads";

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pianobar-rs: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), CliError> {
    let command = CliCommand::from_args(env::args().skip(1))?;
    let config = load_config()?;
    let username = env::var(USER_ENV)
        .ok()
        .or_else(|| config.username.clone())
        .ok_or(CliError::MissingCredential("username"))?;
    let password = resolve_password(&config)?;

    let mut client = PandoraClient::with_config(
        Session::new(config.partner()),
        config.decrypt_password.as_bytes().to_vec(),
        config.encrypt_password.as_bytes().to_vec(),
        config.http_config(),
    )?;
    client
        .login(username, password)
        .await
        .map_err(|err| CliError::Operation {
            operation: "login",
            source: err,
        })?;
    match command {
        CliCommand::Stations => {
            let stations = client
                .get_stations()
                .await
                .map_err(|err| CliError::Operation {
                    operation: "get stations",
                    source: err,
                })?;

            for station in stations {
                println!(
                    "{}\t{}{}",
                    station.id,
                    station.name.unwrap_or_else(|| "(unnamed)".to_string()),
                    if station.is_quick_mix {
                        "\tquickmix"
                    } else {
                        ""
                    }
                );
            }
        }
        CliCommand::Playlist {
            station_id,
            quality,
        } => {
            let quality = quality.unwrap_or(config.audio_quality);
            let songs = client
                .get_playlist(station_id.as_str(), quality)
                .await
                .map_err(|err| CliError::Operation {
                    operation: "get playlist",
                    source: err,
                })?;

            for song in songs {
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    song.track_token.as_deref().unwrap_or(""),
                    song.artist.as_deref().unwrap_or(""),
                    song.title.as_deref().unwrap_or(""),
                    song.album.as_deref().unwrap_or(""),
                    song.audio_url.as_deref().unwrap_or(""),
                    song.cover_art.as_deref().unwrap_or("")
                );
            }
        }
        CliCommand::DownloadFirst {
            station_id,
            quality,
            output_dir,
        } => {
            let quality = quality.unwrap_or(config.audio_quality);
            let output_dir = output_dir
                .or(config.rec.clone())
                .unwrap_or_else(|| PathBuf::from(DEFAULT_DOWNLOAD_DIR));
            let songs = client
                .get_playlist(station_id.as_str(), quality)
                .await
                .map_err(|err| CliError::Operation {
                    operation: "get playlist",
                    source: err,
                })?;
            let song = songs.into_iter().next().ok_or(CliError::NoSongs)?;
            let saved = download_song_assets(&song, &DownloadOptions::new(output_dir)).await?;

            println!("audio\t{}", saved.audio.display());
            if let Some(cover) = saved.cover {
                println!("cover\t{}", cover.display());
            }
        }
    }

    Ok(())
}

fn load_config() -> Result<PianobarConfig, CliError> {
    if let Ok(path) = env::var(CONFIG_ENV) {
        return Ok(PianobarConfig::read_path(path)?);
    }

    let Some(home) = env::var_os("HOME").map(PathBuf::from) else {
        return Ok(PianobarConfig::default());
    };
    let xdg = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".config"));
    let config_path = xdg.join("pianobar").join("config");
    if config_path.exists() {
        Ok(PianobarConfig::read_path(config_path)?)
    } else if PathBuf::from("config").exists() {
        Ok(PianobarConfig::read_path("config")?)
    } else {
        Ok(PianobarConfig::default())
    }
}

fn resolve_password(config: &PianobarConfig) -> Result<String, CliError> {
    if let Ok(password) = env::var(PASSWORD_ENV) {
        return Ok(password);
    }
    if let Some(password) = &config.password {
        return Ok(password.clone());
    }
    if let Some(password) = run_password_command(config.password_command.as_deref()) {
        return password;
    }
    Err(CliError::MissingCredential("password"))
}

fn run_password_command(command: Option<&str>) -> Option<Result<String, CliError>> {
    let command = command?;
    Some(
        ProcessCommand::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .map_err(CliError::Io)
            .and_then(|output| {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout)
                        .trim_end_matches(['\r', '\n'])
                        .to_string())
                } else {
                    Err(CliError::PasswordCommandFailed)
                }
            }),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CliCommand {
    Stations,
    Playlist {
        station_id: String,
        quality: Option<AudioQuality>,
    },
    DownloadFirst {
        station_id: String,
        quality: Option<AudioQuality>,
        output_dir: Option<PathBuf>,
    },
}

impl CliCommand {
    fn from_args(mut args: impl Iterator<Item = String>) -> Result<Self, CliError> {
        match args.next().as_deref() {
            None | Some("stations") => Ok(Self::Stations),
            Some("playlist") => {
                let station_id = args.next().ok_or(CliError::Usage)?;
                let quality = parse_quality(args.next().as_deref())?;
                if args.next().is_some() {
                    return Err(CliError::Usage);
                }
                Ok(Self::Playlist {
                    station_id,
                    quality,
                })
            }
            Some("download-first") => {
                let station_id = args.next().ok_or(CliError::Usage)?;
                let quality = parse_quality(args.next().as_deref())?;
                let output_dir = args.next().map(PathBuf::from);
                if args.next().is_some() {
                    return Err(CliError::Usage);
                }
                Ok(Self::DownloadFirst {
                    station_id,
                    quality,
                    output_dir,
                })
            }
            Some(_) => Err(CliError::Usage),
        }
    }
}

fn parse_quality(value: Option<&str>) -> Result<Option<AudioQuality>, CliError> {
    match value {
        None => Ok(None),
        Some("high") => Ok(Some(AudioQuality::High)),
        Some("medium") => Ok(Some(AudioQuality::Medium)),
        Some("low") => Ok(Some(AudioQuality::Low)),
        Some(_) => Err(CliError::Usage),
    }
}

#[derive(Debug)]
enum CliError {
    Usage,
    MissingCredential(&'static str),
    PasswordCommandFailed,
    Config(ConfigError),
    NoSongs,
    Client(ClientError),
    Storage(StorageError),
    Io(std::io::Error),
    Operation {
        operation: &'static str,
        source: ClientError,
    },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => write!(
                f,
                "usage: pianobar-rs [stations|playlist <station-id> [low|medium|high]|download-first <station-id> [low|medium|high] [output-dir]]"
            ),
            Self::MissingCredential(name) => write!(
                f,
                "missing {name}; set it in config or with PIANOBAR_{}",
                name.to_ascii_uppercase()
            ),
            Self::PasswordCommandFailed => write!(f, "password_command exited unsuccessfully"),
            Self::Config(err) => write!(f, "{err}"),
            Self::NoSongs => write!(f, "playlist did not contain any songs"),
            Self::Client(err) => write!(f, "{err}"),
            Self::Storage(err) => write!(f, "{err}"),
            Self::Io(err) => write!(f, "{err}"),
            Self::Operation { operation, source } => write!(f, "{operation} failed: {source}"),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage => None,
            Self::MissingCredential(_) => None,
            Self::PasswordCommandFailed => None,
            Self::Config(err) => Some(err),
            Self::NoSongs => None,
            Self::Client(err) => Some(err),
            Self::Storage(err) => Some(err),
            Self::Io(err) => Some(err),
            Self::Operation { source, .. } => Some(source),
        }
    }
}

impl From<ClientError> for CliError {
    fn from(err: ClientError) -> Self {
        Self::Client(err)
    }
}

impl From<ConfigError> for CliError {
    fn from(err: ConfigError) -> Self {
        Self::Config(err)
    }
}

impl From<StorageError> for CliError {
    fn from(err: StorageError) -> Self {
        Self::Storage(err)
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_stations_command() {
        assert_eq!(
            CliCommand::from_args([].into_iter()).unwrap(),
            CliCommand::Stations
        );
        assert_eq!(
            CliCommand::from_args(["stations".to_string()].into_iter()).unwrap(),
            CliCommand::Stations
        );
    }

    #[test]
    fn parses_playlist_command() {
        assert_eq!(
            CliCommand::from_args(
                ["playlist", "station-id", "medium"]
                    .map(String::from)
                    .into_iter()
            )
            .unwrap(),
            CliCommand::Playlist {
                station_id: "station-id".to_string(),
                quality: Some(AudioQuality::Medium),
            }
        );
    }

    #[test]
    fn parses_playlist_default_quality() {
        assert_eq!(
            CliCommand::from_args(["playlist", "station-id"].map(String::from).into_iter())
                .unwrap(),
            CliCommand::Playlist {
                station_id: "station-id".to_string(),
                quality: None,
            }
        );
    }

    #[test]
    fn parses_download_first_command() {
        assert_eq!(
            CliCommand::from_args(
                ["download-first", "station-id", "low", "/tmp/out"]
                    .map(String::from)
                    .into_iter()
            )
            .unwrap(),
            CliCommand::DownloadFirst {
                station_id: "station-id".to_string(),
                quality: Some(AudioQuality::Low),
                output_dir: Some(PathBuf::from("/tmp/out")),
            }
        );
    }

    #[test]
    fn rejects_bad_usage() {
        assert!(matches!(
            CliCommand::from_args(["playlist"].map(String::from).into_iter()),
            Err(CliError::Usage)
        ));
        assert!(matches!(
            CliCommand::from_args(["nope"].map(String::from).into_iter()),
            Err(CliError::Usage)
        ));
    }
}
