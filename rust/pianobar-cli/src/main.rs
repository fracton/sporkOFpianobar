use pianobar_core::{
    download_song_assets, AudioQuality, ClientError, DownloadOptions, PandoraClient, StorageError,
};
use std::env;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::process::ExitCode;

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
    let command = Command::from_args(env::args().skip(1))?;
    let username = env::var(USER_ENV).map_err(|_| CliError::MissingEnv(USER_ENV))?;
    let password = env::var(PASSWORD_ENV).map_err(|_| CliError::MissingEnv(PASSWORD_ENV))?;

    let mut client = PandoraClient::android_default()?;
    client
        .login(username, password)
        .await
        .map_err(|err| CliError::Operation {
            operation: "login",
            source: err,
        })?;
    match command {
        Command::Stations => {
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
        Command::Playlist {
            station_id,
            quality,
        } => {
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
        Command::DownloadFirst {
            station_id,
            quality,
            output_dir,
        } => {
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum Command {
    Stations,
    Playlist {
        station_id: String,
        quality: AudioQuality,
    },
    DownloadFirst {
        station_id: String,
        quality: AudioQuality,
        output_dir: PathBuf,
    },
}

impl Command {
    fn from_args(mut args: impl Iterator<Item = String>) -> Result<Self, CliError> {
        match args.next().as_deref() {
            None | Some("stations") => Ok(Self::Stations),
            Some("playlist") => {
                let station_id = args.next().ok_or(CliError::Usage)?;
                let quality = match args.next().as_deref() {
                    None | Some("high") => AudioQuality::High,
                    Some("medium") => AudioQuality::Medium,
                    Some("low") => AudioQuality::Low,
                    Some(_) => return Err(CliError::Usage),
                };
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
                let output_dir = args
                    .next()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(DEFAULT_DOWNLOAD_DIR));
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

fn parse_quality(value: Option<&str>) -> Result<AudioQuality, CliError> {
    match value {
        None | Some("high") => Ok(AudioQuality::High),
        Some("medium") => Ok(AudioQuality::Medium),
        Some("low") => Ok(AudioQuality::Low),
        Some(_) => Err(CliError::Usage),
    }
}

#[derive(Debug)]
enum CliError {
    Usage,
    MissingEnv(&'static str),
    NoSongs,
    Client(ClientError),
    Storage(StorageError),
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
            Self::MissingEnv(name) => write!(f, "missing required environment variable {name}"),
            Self::NoSongs => write!(f, "playlist did not contain any songs"),
            Self::Client(err) => write!(f, "{err}"),
            Self::Storage(err) => write!(f, "{err}"),
            Self::Operation { operation, source } => write!(f, "{operation} failed: {source}"),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Usage => None,
            Self::MissingEnv(_) => None,
            Self::NoSongs => None,
            Self::Client(err) => Some(err),
            Self::Storage(err) => Some(err),
            Self::Operation { source, .. } => Some(source),
        }
    }
}

impl From<ClientError> for CliError {
    fn from(err: ClientError) -> Self {
        Self::Client(err)
    }
}

impl From<StorageError> for CliError {
    fn from(err: StorageError) -> Self {
        Self::Storage(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_stations_command() {
        assert_eq!(
            Command::from_args([].into_iter()).unwrap(),
            Command::Stations
        );
        assert_eq!(
            Command::from_args(["stations".to_string()].into_iter()).unwrap(),
            Command::Stations
        );
    }

    #[test]
    fn parses_playlist_command() {
        assert_eq!(
            Command::from_args(
                ["playlist", "station-id", "medium"]
                    .map(String::from)
                    .into_iter()
            )
            .unwrap(),
            Command::Playlist {
                station_id: "station-id".to_string(),
                quality: AudioQuality::Medium,
            }
        );
    }

    #[test]
    fn parses_download_first_command() {
        assert_eq!(
            Command::from_args(
                ["download-first", "station-id", "low", "/tmp/out"]
                    .map(String::from)
                    .into_iter()
            )
            .unwrap(),
            Command::DownloadFirst {
                station_id: "station-id".to_string(),
                quality: AudioQuality::Low,
                output_dir: PathBuf::from("/tmp/out"),
            }
        );
    }

    #[test]
    fn rejects_bad_usage() {
        assert!(matches!(
            Command::from_args(["playlist"].map(String::from).into_iter()),
            Err(CliError::Usage)
        ));
        assert!(matches!(
            Command::from_args(["nope"].map(String::from).into_iter()),
            Err(CliError::Usage)
        ));
    }
}
