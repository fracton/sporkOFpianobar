use crate::crypt::{decrypt_hex, CryptError};
use crate::model::{
    Artist, AudioFormat, AudioQuality, Genre, GenreCategory, SearchResult, Song, SongRating,
    Station, StationInfo, StationMode, User, UserSettings,
};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const API_ERROR_OFFSET: i32 = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartnerLogin {
    pub auth_token: String,
    pub partner_id: u32,
    pub time_offset: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserLogin {
    pub user: User,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("Pandora API error {code}: {kind:?}")]
pub struct ApiError {
    pub code: i32,
    pub kind: ApiErrorKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiErrorKind {
    Internal,
    MaintenanceMode,
    UrlParamMissingMethod,
    UrlParamMissingAuthToken,
    UrlParamMissingPartnerId,
    UrlParamMissingUserId,
    SecureProtocolRequired,
    CertificateRequired,
    ParameterTypeMismatch,
    ParameterMissing,
    ParameterValueInvalid,
    ApiVersionNotSupported,
    LicensingRestrictions,
    InsufficientConnectivity,
    ReadOnlyMode,
    InvalidAuthToken,
    InvalidPartnerLogin,
    ListenerNotAuthorized,
    UserNotAuthorized,
    MaxStationsReached,
    StationDoesNotExist,
    ComplimentaryPeriodAlreadyInUse,
    CallNotAllowed,
    DeviceNotFound,
    PartnerNotAuthorized,
    InvalidUsername,
    InvalidPassword,
    UsernameAlreadyExists,
    DeviceAlreadyAssociatedToAccount,
    UpgradeDeviceModelInvalid,
    ExplicitPinIncorrect,
    ExplicitPinMalformed,
    DeviceModelInvalid,
    ZipCodeInvalid,
    BirthYearInvalid,
    BirthYearTooYoung,
    InvalidCountryCode,
    DeviceDisabled,
    DailyTrialLimitReached,
    InvalidSponsor,
    UserAlreadyUsedTrial,
    RateLimit,
    Unknown,
}

impl ApiErrorKind {
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Internal,
            1 => Self::MaintenanceMode,
            2 => Self::UrlParamMissingMethod,
            3 => Self::UrlParamMissingAuthToken,
            4 => Self::UrlParamMissingPartnerId,
            5 => Self::UrlParamMissingUserId,
            6 => Self::SecureProtocolRequired,
            7 => Self::CertificateRequired,
            8 => Self::ParameterTypeMismatch,
            9 => Self::ParameterMissing,
            10 => Self::ParameterValueInvalid,
            11 => Self::ApiVersionNotSupported,
            12 => Self::LicensingRestrictions,
            13 => Self::InsufficientConnectivity,
            1000 => Self::ReadOnlyMode,
            1001 => Self::InvalidAuthToken,
            1002 => Self::InvalidPartnerLogin,
            1003 => Self::ListenerNotAuthorized,
            1004 => Self::UserNotAuthorized,
            1005 => Self::MaxStationsReached,
            1006 => Self::StationDoesNotExist,
            1007 => Self::ComplimentaryPeriodAlreadyInUse,
            1008 => Self::CallNotAllowed,
            1009 => Self::DeviceNotFound,
            1010 => Self::PartnerNotAuthorized,
            1011 => Self::InvalidUsername,
            1012 => Self::InvalidPassword,
            1013 => Self::UsernameAlreadyExists,
            1014 => Self::DeviceAlreadyAssociatedToAccount,
            1015 => Self::UpgradeDeviceModelInvalid,
            1018 => Self::ExplicitPinIncorrect,
            1020 => Self::ExplicitPinMalformed,
            1023 => Self::DeviceModelInvalid,
            1024 => Self::ZipCodeInvalid,
            1025 => Self::BirthYearInvalid,
            1026 => Self::BirthYearTooYoung,
            1027 => Self::InvalidCountryCode,
            1034 => Self::DeviceDisabled,
            1035 => Self::DailyTrialLimitReached,
            1036 => Self::InvalidSponsor,
            1037 => Self::UserAlreadyUsedTrial,
            1039 => Self::RateLimit,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Error)]
pub enum ResponseError {
    #[error("invalid JSON response")]
    Json(#[from] serde_json::Error),
    #[error("response is missing required field `{0}`")]
    MissingField(&'static str),
    #[error("response has an invalid field `{0}`")]
    InvalidField(&'static str),
    #[error("requested audio quality is not available")]
    QualityUnavailable,
    #[error("station mode change did not activate the requested mode")]
    StationModeNotChanged,
    #[error("system clock is before UNIX epoch")]
    InvalidSystemClock,
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error("response decryption failed")]
    Crypt(#[from] CryptError),
}

pub fn parse_partner_login(input: &str, in_key: &[u8]) -> Result<PartnerLogin, ResponseError> {
    let result = envelope(input)?;
    let sync_time = required_str(&result, "syncTime")?;
    let decrypted = decrypt_hex(in_key, sync_time)?;
    let timestamp = std::str::from_utf8(decrypted.get(4..).unwrap_or_default())
        .map_err(|_| ResponseError::InvalidField("syncTime"))?
        .trim_end_matches('\0')
        .parse::<i64>()
        .map_err(|_| ResponseError::InvalidField("syncTime"))?;
    let real_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ResponseError::InvalidSystemClock)?
        .as_secs() as i64;

    Ok(PartnerLogin {
        auth_token: required_str(&result, "partnerAuthToken")?.to_string(),
        partner_id: required_u32(&result, "partnerId")?,
        time_offset: real_time - timestamp,
    })
}

pub fn parse_user_login(input: &str) -> Result<UserLogin, ResponseError> {
    let result = envelope(input)?;
    Ok(UserLogin {
        user: User {
            listener_id: required_str(&result, "userId")?.to_string(),
            auth_token: required_str(&result, "userAuthToken")?.to_string(),
        },
    })
}

pub fn parse_stations(input: &str) -> Result<Vec<Station>, ResponseError> {
    let result = envelope(input)?;
    let Some(stations) = result.get("stations").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut parsed: Vec<Station> = stations.iter().map(parse_station).collect();
    let quick_mix_ids: Vec<&str> = stations
        .iter()
        .find(|station| bool_or(station, "isQuickMix", false))
        .and_then(|station| station.get("quickMixStationIds"))
        .and_then(Value::as_array)
        .map(|ids| ids.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    for station in &mut parsed {
        station.use_quick_mix = quick_mix_ids.iter().any(|id| *id == station.id);
    }

    Ok(parsed)
}

pub fn parse_playlist(input: &str, quality: AudioQuality) -> Result<Vec<Song>, ResponseError> {
    let result = envelope(input)?;
    let Some(items) = result.get("items").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };

    let mut playlist = Vec::new();
    for item in items {
        if item.get("artistName").is_none() {
            continue;
        }

        let mut song = Song {
            artist: string(item, "artistName"),
            album: string(item, "albumName"),
            title: string(item, "songName"),
            track_token: string(item, "trackToken"),
            station_id: string(item, "stationId"),
            cover_art: string(item, "albumArtUrl"),
            detail_url: string(item, "songDetailUrl"),
            file_gain: item.get("trackGain").and_then(Value::as_f64).unwrap_or(0.0),
            length: item
                .get("trackLength")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .try_into()
                .unwrap_or(u32::MAX),
            rating: rating_from_int(item.get("songRating").and_then(Value::as_i64).unwrap_or(0)),
            ..Song::default()
        };

        if let Some(audio) = item
            .get("audioUrlMap")
            .and_then(|map| map.get(quality.response_key()))
        {
            let encoding = required_str(audio, "encoding")?;
            song.audio_format = AudioFormat::from_encoding(encoding);
            song.audio_url = string(audio, "audioUrl");
        } else {
            return Err(ResponseError::QualityUnavailable);
        }

        playlist.push(song);
    }

    Ok(playlist)
}

pub fn parse_search(input: &str) -> Result<SearchResult, ResponseError> {
    let result = envelope(input)?;
    let artists = result
        .get("artists")
        .and_then(Value::as_array)
        .map(|artists| {
            artists
                .iter()
                .map(|artist| Artist {
                    name: string(artist, "artistName"),
                    music_id: string(artist, "musicToken"),
                    seed_id: string(artist, "seedId"),
                    score: artist
                        .get("score")
                        .and_then(Value::as_i64)
                        .and_then(|v| v.try_into().ok()),
                })
                .collect()
        })
        .unwrap_or_default();
    let songs = result
        .get("songs")
        .and_then(Value::as_array)
        .map(|songs| {
            songs
                .iter()
                .map(|item| Song {
                    title: string(item, "songName"),
                    artist: string(item, "artistName"),
                    music_id: string(item, "musicToken"),
                    ..Song::default()
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(SearchResult { artists, songs })
}

pub fn parse_created_station(input: &str) -> Result<Station, ResponseError> {
    let result = envelope(input)?;
    Ok(parse_station(&result))
}

pub fn parse_genre_stations(input: &str) -> Result<Vec<GenreCategory>, ResponseError> {
    let result = envelope(input)?;
    let categories = result
        .get("categories")
        .and_then(Value::as_array)
        .map(|categories| {
            categories
                .iter()
                .map(|category| {
                    let genres = category
                        .get("stations")
                        .and_then(Value::as_array)
                        .map(|stations| {
                            stations
                                .iter()
                                .map(|station| Genre {
                                    name: string(station, "stationName"),
                                    music_id: string(station, "stationToken"),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    GenreCategory {
                        name: string(category, "categoryName"),
                        genres,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(categories)
}

pub fn parse_explain(input: &str) -> Result<Option<String>, ResponseError> {
    let result = envelope(input)?;
    let Some(explanations) = result.get("explanations").and_then(Value::as_array) else {
        return Ok(None);
    };
    let traits: Vec<&str> = explanations
        .iter()
        .filter_map(|explanation| explanation.get("focusTraitName").and_then(Value::as_str))
        .collect();

    if traits.is_empty() {
        return Ok(None);
    }

    let mut out = String::from("We're playing this track because it features ");
    for (idx, name) in traits.iter().enumerate() {
        out.push_str(name);
        if idx < traits.len().saturating_sub(2) {
            out.push_str(", ");
        } else if idx == traits.len().saturating_sub(2) {
            out.push_str(" and ");
        } else {
            out.push('.');
        }
    }

    Ok(Some(out))
}

pub fn parse_get_settings(input: &str) -> Result<UserSettings, ResponseError> {
    let result = envelope(input)?;
    Ok(UserSettings {
        username: string(&result, "username"),
        explicit_content_filter: bool_or(&result, "isExplicitContentFilterEnabled", false),
    })
}

pub fn parse_station_info(input: &str) -> Result<StationInfo, ResponseError> {
    let result = envelope(input)?;
    let mut info = StationInfo {
        song_seeds: Vec::new(),
        artist_seeds: Vec::new(),
        station_seeds: Vec::new(),
        feedback: Vec::new(),
    };

    if let Some(music) = result.get("music") {
        if let Some(songs) = music.get("songs").and_then(Value::as_array) {
            info.song_seeds = songs
                .iter()
                .map(|song| Song {
                    title: string(song, "songName"),
                    artist: string(song, "artistName"),
                    seed_id: string(song, "seedId"),
                    ..Song::default()
                })
                .collect();
        }
        if let Some(artists) = music.get("artists").and_then(Value::as_array) {
            info.artist_seeds = artists
                .iter()
                .map(|artist| Artist {
                    name: string(artist, "artistName"),
                    music_id: string(artist, "musicToken"),
                    seed_id: string(artist, "seedId"),
                    score: None,
                })
                .collect();
        }
        if let Some(stations) = music.get("stations").and_then(Value::as_array) {
            info.station_seeds = stations.iter().map(parse_station).collect();
        }
    }

    if let Some(feedback) = result.get("feedback") {
        for key in ["thumbsUp", "thumbsDown"] {
            let Some(songs) = feedback.get(key).and_then(Value::as_array) else {
                continue;
            };
            info.feedback.extend(songs.iter().map(|song| {
                Song {
                    title: string(song, "songName"),
                    artist: string(song, "artistName"),
                    feedback_id: string(song, "feedbackId"),
                    rating: if bool_or(song, "isPositive", false) {
                        SongRating::Love
                    } else {
                        SongRating::Ban
                    },
                    length: song
                        .get("trackLength")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .try_into()
                        .unwrap_or(u32::MAX),
                    ..Song::default()
                }
            }));
        }
    }

    Ok(info)
}

pub fn parse_station_modes(input: &str) -> Result<Vec<StationMode>, ResponseError> {
    let result = envelope(input)?;
    let active = result
        .get("currentModeId")
        .and_then(Value::as_i64)
        .and_then(|id| i32::try_from(id).ok());
    let modes = result
        .get("availableModes")
        .and_then(Value::as_array)
        .map(|modes| {
            modes
                .iter()
                .filter_map(|mode| {
                    let id = mode
                        .get("modeId")
                        .and_then(Value::as_i64)
                        .and_then(|id| i32::try_from(id).ok())?;
                    Some(StationMode {
                        id,
                        name: string(mode, "modeName"),
                        description: string(mode, "modeDescription"),
                        is_algorithmic: bool_or(mode, "isAlgorithmicMode", false),
                        is_takeover: bool_or(mode, "isTakeoverMode", false),
                        active: active == Some(id),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(modes)
}

pub fn parse_set_station_mode(input: &str, requested_id: u32) -> Result<(), ResponseError> {
    let result = envelope(input)?;
    let active = result
        .get("currentModeId")
        .and_then(Value::as_u64)
        .ok_or(ResponseError::MissingField("currentModeId"))?;
    if active == requested_id as u64 {
        Ok(())
    } else {
        Err(ResponseError::StationModeNotChanged)
    }
}

pub fn parse_empty(input: &str) -> Result<(), ResponseError> {
    ensure_ok(input).map(|_| ())
}

fn envelope(input: &str) -> Result<Value, ResponseError> {
    let root = ensure_ok(input)?;
    root.get("result")
        .cloned()
        .ok_or(ResponseError::MissingField("result"))
}

fn ensure_ok(input: &str) -> Result<Value, ResponseError> {
    let root: Value = serde_json::from_str(input)?;
    match root.get("stat").and_then(Value::as_str) {
        Some("ok") => Ok(root),
        Some(_) => {
            let code = root
                .get("code")
                .and_then(Value::as_i64)
                .ok_or(ResponseError::MissingField("code"))? as i32;
            Err(ApiError {
                code,
                kind: ApiErrorKind::from_code(code),
            }
            .into())
        }
        None => Err(ResponseError::MissingField("stat")),
    }
}

fn parse_station(station: &Value) -> Station {
    Station {
        id: string(station, "stationToken").unwrap_or_default(),
        name: string(station, "stationName"),
        is_creator: !bool_or(station, "isShared", true),
        is_quick_mix: bool_or(station, "isQuickMix", false),
        use_quick_mix: false,
        seed_id: string(station, "seedId"),
    }
}

fn string(value: &Value, key: &'static str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn required_str<'a>(value: &'a Value, key: &'static str) -> Result<&'a str, ResponseError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(ResponseError::MissingField(key))
}

fn required_u32(value: &Value, key: &'static str) -> Result<u32, ResponseError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| value.try_into().ok())
        .ok_or(ResponseError::MissingField(key))
}

fn bool_or(value: &Value, key: &'static str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn rating_from_int(value: i64) -> SongRating {
    match value {
        1 => SongRating::Love,
        _ => SongRating::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypt::encrypt_hex;

    #[test]
    fn parses_partner_login_and_time_offset() {
        let encrypted_sync = encrypt_hex(b"R=U!LH$O2B#", "00001234567890").unwrap();
        let input = format!(
            r#"{{
                "stat": "ok",
                "result": {{
                    "syncTime": "{encrypted_sync}",
                    "partnerAuthToken": "partner-token",
                    "partnerId": 7
                }}
            }}"#
        );

        let login = parse_partner_login(&input, b"R=U!LH$O2B#").unwrap();

        assert_eq!(login.auth_token, "partner-token");
        assert_eq!(login.partner_id, 7);
        assert!(login.time_offset > 0);
    }

    #[test]
    fn parses_station_list_and_quick_mix_flags() {
        let stations = parse_stations(
            r#"{
                "stat": "ok",
                "result": {
                    "stations": [
                        {"stationName": "A", "stationToken": "a", "isShared": false},
                        {"stationName": "Mix", "stationToken": "q", "isQuickMix": true,
                         "quickMixStationIds": ["a"]}
                    ]
                }
            }"#,
        )
        .unwrap();

        assert_eq!(stations.len(), 2);
        assert_eq!(stations[0].name.as_deref(), Some("A"));
        assert!(stations[0].is_creator);
        assert!(stations[0].use_quick_mix);
        assert!(stations[1].is_quick_mix);
        assert!(!stations[1].use_quick_mix);
    }

    #[test]
    fn parses_playlist_for_selected_quality() {
        let songs = parse_playlist(
            r#"{
                "stat": "ok",
                "result": {
                    "items": [{
                        "artistName": "Artist",
                        "albumName": "Album",
                        "songName": "Song",
                        "trackToken": "track",
                        "stationId": "station",
                        "albumArtUrl": "https://img",
                        "songDetailUrl": "https://detail",
                        "trackGain": -1.5,
                        "trackLength": 123,
                        "songRating": 1,
                        "audioUrlMap": {
                            "highQuality": {
                                "encoding": "mp3",
                                "audioUrl": "https://audio"
                            }
                        }
                    }]
                }
            }"#,
            AudioQuality::High,
        )
        .unwrap();

        assert_eq!(songs.len(), 1);
        assert_eq!(songs[0].title.as_deref(), Some("Song"));
        assert_eq!(songs[0].audio_url.as_deref(), Some("https://audio"));
        assert_eq!(songs[0].audio_format, AudioFormat::Mp3);
        assert_eq!(songs[0].rating, SongRating::Love);
    }

    #[test]
    fn unavailable_playlist_quality_is_an_error() {
        let err = parse_playlist(
            r#"{"stat":"ok","result":{"items":[{"artistName":"Artist","audioUrlMap":{}}]}}"#,
            AudioQuality::High,
        )
        .unwrap_err();

        assert!(matches!(err, ResponseError::QualityUnavailable));
    }

    #[test]
    fn parses_search_results() {
        let result = parse_search(
            r#"{
                "stat": "ok",
                "result": {
                    "artists": [{"artistName": "Artist", "musicToken": "artist-token"}],
                    "songs": [{"songName": "Song", "artistName": "Artist", "musicToken": "song-token"}]
                }
            }"#,
        )
        .unwrap();

        assert_eq!(result.artists[0].music_id.as_deref(), Some("artist-token"));
        assert_eq!(result.songs[0].music_id.as_deref(), Some("song-token"));
    }

    #[test]
    fn parses_created_station() {
        let station = parse_created_station(
            r#"{
                "stat": "ok",
                "result": {
                    "stationName": "Created",
                    "stationToken": "created-token",
                    "isShared": false
                }
            }"#,
        )
        .unwrap();

        assert_eq!(station.id, "created-token");
        assert_eq!(station.name.as_deref(), Some("Created"));
        assert!(station.is_creator);
    }

    #[test]
    fn maps_api_errors() {
        let err = parse_empty(r#"{"stat":"fail","code":1001}"#).unwrap_err();

        assert!(matches!(
            err,
            ResponseError::Api(ApiError {
                code: 1001,
                kind: ApiErrorKind::InvalidAuthToken
            })
        ));
    }

    #[test]
    fn accepts_success_without_result_for_empty_responses() {
        parse_empty(r#"{"stat":"ok"}"#).unwrap();
    }

    #[test]
    fn parses_station_modes_and_mode_set_response() {
        let input = r#"{
            "stat": "ok",
            "result": {
                "currentModeId": 2,
                "availableModes": [
                    {"modeId": 1, "modeName": "Normal"},
                    {"modeId": 2, "modeName": "Deep Cuts", "isAlgorithmicMode": true}
                ]
            }
        }"#;
        let modes = parse_station_modes(input).unwrap();

        assert_eq!(modes.len(), 2);
        assert!(!modes[0].active);
        assert!(modes[1].active);
        parse_set_station_mode(input, 2).unwrap();
    }
}
