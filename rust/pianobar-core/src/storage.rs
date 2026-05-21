use crate::model::Song;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DownloadOptions {
    pub output_dir: PathBuf,
    pub include_cover: bool,
    pub overwrite: bool,
}

impl DownloadOptions {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            include_cover: true,
            overwrite: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedAssets {
    pub audio: PathBuf,
    pub cover: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("selected song is missing {0} URL")]
    MissingSongUrl(&'static str),
    #[error("download request failed")]
    Http(#[from] reqwest::Error),
    #[error("file operation failed")]
    Io(#[from] std::io::Error),
}

pub async fn download_song_assets(
    song: &Song,
    options: &DownloadOptions,
) -> Result<SavedAssets, StorageError> {
    let audio_url = song
        .audio_url
        .as_deref()
        .ok_or(StorageError::MissingSongUrl("audio"))?;
    fs::create_dir_all(&options.output_dir)?;

    let base_name = song_file_stem(song);
    let audio_ext = extension_from_url(audio_url).unwrap_or("audio");
    let audio_path = output_path(
        &options.output_dir,
        &base_name,
        audio_ext,
        options.overwrite,
    );
    download_url_to_file(audio_url, &audio_path).await?;

    let cover = if options.include_cover {
        if let Some(cover_url) = song.cover_art.as_deref().filter(|url| !url.is_empty()) {
            let cover_path = output_path(
                &options.output_dir,
                &format!("{base_name}-cover"),
                "jpg",
                options.overwrite,
            );
            download_url_to_file(cover_url, &cover_path).await?;
            Some(cover_path)
        } else {
            None
        }
    } else {
        None
    };

    Ok(SavedAssets {
        audio: audio_path,
        cover,
    })
}

async fn download_url_to_file(url: &str, path: &Path) -> Result<(), StorageError> {
    let response = reqwest::get(url).await?.error_for_status()?;
    let bytes = response.bytes().await?;
    write_atomic(path, &bytes)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let tmp_path = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("download")
    ));

    fs::write(&tmp_path, bytes)?;
    fs::rename(&tmp_path, path)?;

    Ok(())
}

pub fn song_file_stem(song: &Song) -> String {
    let artist = song.artist.as_deref().unwrap_or("unknown artist");
    let title = song.title.as_deref().unwrap_or("unknown title");
    sanitize_filename(&format!("{artist} - {title}"))
}

pub fn sanitize_filename(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.' | '(' | ')') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches([' ', '.']).trim();

    if trimmed.is_empty() {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn extension_from_url(url: &str) -> Option<&str> {
    let path = url.split('?').next().unwrap_or(url);
    let ext = path.rsplit('/').next()?.rsplit_once('.')?.1;
    (!ext.is_empty() && ext.len() <= 8 && ext.chars().all(|ch| ch.is_ascii_alphanumeric()))
        .then_some(ext)
}

pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }

    for idx in 1.. {
        let candidate = dir.join(format!("{stem}-{idx}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }

    unreachable!("unbounded iterator always returns a candidate")
}

fn output_path(dir: &Path, stem: &str, ext: &str, overwrite: bool) -> PathBuf {
    if overwrite {
        dir.join(format!("{stem}.{ext}"))
    } else {
        unique_path(dir, stem, ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_file_names() {
        assert_eq!(
            sanitize_filename("AC/DC: Thunderstruck?"),
            "AC_DC_ Thunderstruck_"
        );
        assert_eq!(sanitize_filename("..."), "download");
    }

    #[test]
    fn extracts_url_extension() {
        assert_eq!(
            extension_from_url("https://example.test/path/song.m4a?token=abc"),
            Some("m4a")
        );
        assert_eq!(extension_from_url("https://example.test/path/song"), None);
    }

    #[test]
    fn builds_song_file_stem() {
        let song = Song {
            artist: Some("Artist/Name".to_string()),
            title: Some("Title?".to_string()),
            ..Song::default()
        };

        assert_eq!(song_file_stem(&song), "Artist_Name - Title_");
    }
}
