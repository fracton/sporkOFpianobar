use pianobar_core::{
    download_song_assets, AudioQuality, ClientError, ConfigError, DownloadOptions, PandoraClient,
    PianobarConfig, Session, Station, StorageError,
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
        CliCommand::Login => {
            let user = client
                .session()
                .user
                .as_ref()
                .ok_or(CliError::Unauthenticated)?;
            println!("listener\t{}", user.listener_id);
        }
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
        CliCommand::Playlist { station, quality } => {
            let quality = quality.unwrap_or(config.audio_quality);
            let station_id = resolve_station_id(&client, &config, station.as_deref()).await?;
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
            station,
            quality,
            output_dir,
        } => {
            let quality = quality.unwrap_or(config.audio_quality);
            let output_dir = output_dir
                .or(config.rec.clone())
                .unwrap_or_else(|| PathBuf::from(DEFAULT_DOWNLOAD_DIR));
            let station_id = resolve_station_id(&client, &config, station.as_deref()).await?;
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

async fn resolve_station_id(
    client: &PandoraClient,
    config: &PianobarConfig,
    requested: Option<&str>,
) -> Result<String, CliError> {
    let selector = requested
        .map(str::to_string)
        .or_else(|| config.autostart_station.clone())
        .ok_or(CliError::MissingStation)?;
    let stations = client
        .get_stations()
        .await
        .map_err(|err| CliError::Operation {
            operation: "get stations",
            source: err,
        })?;

    find_station(&stations, &selector)
        .map(|station| station.id.clone())
        .ok_or(CliError::UnknownStation(selector))
}

fn find_station<'a>(stations: &'a [Station], selector: &str) -> Option<&'a Station> {
    stations
        .iter()
        .find(|station| station.id == selector)
        .or_else(|| {
            stations
                .iter()
                .find(|station| station.name.as_deref() == Some(selector))
        })
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
    Login,
    Stations,
    Playlist {
        station: Option<String>,
        quality: Option<AudioQuality>,
    },
    DownloadFirst {
        station: Option<String>,
        quality: Option<AudioQuality>,
        output_dir: Option<PathBuf>,
    },
}

impl CliCommand {
    fn from_args(mut args: impl Iterator<Item = String>) -> Result<Self, CliError> {
        match args.next().as_deref() {
            None | Some("stations") => Ok(Self::Stations),
            Some("login") => {
                if args.next().is_some() {
                    return Err(CliError::Usage);
                }
                Ok(Self::Login)
            }
            Some("playlist") => {
                let (station, quality) = parse_station_and_quality(args)?;
                Ok(Self::Playlist { station, quality })
            }
            Some("download-first") => {
                let (station, quality, output_dir) = parse_download_args(args)?;
                Ok(Self::DownloadFirst {
                    station,
                    quality,
                    output_dir,
                })
            }
            Some(_) => Err(CliError::Usage),
        }
    }
}

fn parse_station_and_quality(
    args: impl Iterator<Item = String>,
) -> Result<(Option<String>, Option<AudioQuality>), CliError> {
    let values: Vec<String> = args.collect();
    match values.as_slice() {
        [] => Ok((None, None)),
        [one] if is_quality(one) => Ok((None, parse_quality(Some(one))?)),
        [station] => Ok((Some(station.clone()), None)),
        [station, quality] => Ok((Some(station.clone()), parse_quality(Some(quality))?)),
        _ => Err(CliError::Usage),
    }
}

fn parse_download_args(
    args: impl Iterator<Item = String>,
) -> Result<(Option<String>, Option<AudioQuality>, Option<PathBuf>), CliError> {
    let values: Vec<String> = args.collect();
    match values.as_slice() {
        [] => Ok((None, None, None)),
        [one] if is_quality(one) => Ok((None, parse_quality(Some(one))?, None)),
        [station] => Ok((Some(station.clone()), None, None)),
        [one, output_dir] if is_quality(one) => Ok((
            None,
            parse_quality(Some(one))?,
            Some(PathBuf::from(output_dir)),
        )),
        [station, quality] => Ok((Some(station.clone()), parse_quality(Some(quality))?, None)),
        [station, quality, output_dir] => Ok((
            Some(station.clone()),
            parse_quality(Some(quality))?,
            Some(PathBuf::from(output_dir)),
        )),
        _ => Err(CliError::Usage),
    }
}

fn is_quality(value: &str) -> bool {
    matches!(value, "high" | "medium" | "low")
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
    MissingStation,
    UnknownStation(String),
    Unauthenticated,
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
                "usage: pianobar-rs [login|stations|playlist [station-id-or-name] [low|medium|high]|download-first [station-id-or-name] [low|medium|high] [output-dir]]"
            ),
            Self::MissingCredential(name) => write!(
                f,
                "missing {name}; set it in config or with PIANOBAR_{}",
                name.to_ascii_uppercase()
            ),
            Self::MissingStation => write!(
                f,
                "missing station; pass a station id/name or set autostart_station in config"
            ),
            Self::UnknownStation(station) => write!(f, "station not found: {station}"),
            Self::Unauthenticated => write!(f, "login succeeded without a user session"),
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
            Self::MissingStation => None,
            Self::UnknownStation(_) => None,
            Self::Unauthenticated => None,
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
    fn parses_login_command() {
        assert_eq!(
            CliCommand::from_args(["login"].map(String::from).into_iter()).unwrap(),
            CliCommand::Login
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
                station: Some("station-id".to_string()),
                quality: Some(AudioQuality::Medium),
            }
        );
    }

    #[test]
    fn parses_playlist_with_default_station_or_default_quality() {
        assert_eq!(
            CliCommand::from_args(["playlist", "station-id"].map(String::from).into_iter())
                .unwrap(),
            CliCommand::Playlist {
                station: Some("station-id".to_string()),
                quality: None,
            }
        );
        assert_eq!(
            CliCommand::from_args(["playlist", "high"].map(String::from).into_iter()).unwrap(),
            CliCommand::Playlist {
                station: None,
                quality: Some(AudioQuality::High),
            }
        );
        assert_eq!(
            CliCommand::from_args(["playlist"].map(String::from).into_iter()).unwrap(),
            CliCommand::Playlist {
                station: None,
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
                station: Some("station-id".to_string()),
                quality: Some(AudioQuality::Low),
                output_dir: Some(PathBuf::from("/tmp/out")),
            }
        );
        assert_eq!(
            CliCommand::from_args(
                ["download-first", "medium", "/tmp/out"]
                    .map(String::from)
                    .into_iter()
            )
            .unwrap(),
            CliCommand::DownloadFirst {
                station: None,
                quality: Some(AudioQuality::Medium),
                output_dir: Some(PathBuf::from("/tmp/out")),
            }
        );
    }

    #[test]
    fn rejects_bad_usage() {
        assert!(matches!(
            CliCommand::from_args(["login", "extra"].map(String::from).into_iter()),
            Err(CliError::Usage)
        ));
        assert!(matches!(
            CliCommand::from_args(["nope"].map(String::from).into_iter()),
            Err(CliError::Usage)
        ));
    }

    #[test]
    fn finds_station_by_id_or_name() {
        let stations = vec![
            Station {
                id: "station-1".to_string(),
                name: Some("Morning".to_string()),
                is_creator: true,
                is_quick_mix: false,
                use_quick_mix: false,
                seed_id: None,
            },
            Station {
                id: "station-2".to_string(),
                name: Some("Evening".to_string()),
                is_creator: false,
                is_quick_mix: false,
                use_quick_mix: false,
                seed_id: None,
            },
        ];

        assert_eq!(
            find_station(&stations, "station-1").unwrap().id,
            "station-1"
        );
        assert_eq!(find_station(&stations, "Evening").unwrap().id, "station-2");
        assert!(find_station(&stations, "missing").is_none());
    }
}
