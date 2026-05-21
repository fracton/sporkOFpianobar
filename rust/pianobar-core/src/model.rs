#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Partner {
    pub user: String,
    pub password: String,
    pub device: String,
    pub auth_token: Option<String>,
    pub id: Option<u32>,
}

impl Partner {
    pub fn android_default() -> Self {
        Self {
            user: "android".to_string(),
            password: "AC7IBG09A3DTSYM4R41UJWL07VLN8JI7".to_string(),
            device: "android-generic".to_string(),
            auth_token: None,
            id: None,
        }
    }

    pub fn authenticated(
        user: impl Into<String>,
        password: impl Into<String>,
        device: impl Into<String>,
        auth_token: impl Into<String>,
        id: u32,
    ) -> Self {
        Self {
            user: user.into(),
            password: password.into(),
            device: device.into(),
            auth_token: Some(auth_token.into()),
            id: Some(id),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct User {
    pub listener_id: String,
    pub auth_token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    pub partner: Partner,
    pub user: Option<User>,
    pub time_offset: i64,
}

impl Session {
    pub fn new(partner: Partner) -> Self {
        Self {
            partner,
            user: None,
            time_offset: 0,
        }
    }

    pub fn authenticated(partner: Partner, user: User, time_offset: i64) -> Self {
        Self {
            partner,
            user: Some(user),
            time_offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioQuality {
    Low,
    Medium,
    High,
}

impl AudioQuality {
    pub(crate) fn response_key(self) -> &'static str {
        match self {
            Self::Low => "lowQuality",
            Self::Medium => "mediumQuality",
            Self::High => "highQuality",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioFormat {
    Unknown,
    AacPlus,
    Mp3,
}

impl AudioFormat {
    pub(crate) fn from_encoding(encoding: &str) -> Self {
        match encoding {
            "aacplus" => Self::AacPlus,
            "mp3" => Self::Mp3,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SongRating {
    None,
    Love,
    Ban,
    Tired,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Station {
    pub id: String,
    pub name: Option<String>,
    pub is_creator: bool,
    pub is_quick_mix: bool,
    pub use_quick_mix: bool,
    pub seed_id: Option<String>,
}

impl Station {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: None,
            is_creator: false,
            is_quick_mix: false,
            use_quick_mix: false,
            seed_id: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateStationKind {
    MusicToken,
    Song,
    Artist,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub artist: Option<String>,
    pub station_id: Option<String>,
    pub album: Option<String>,
    pub audio_url: Option<String>,
    pub cover_art: Option<String>,
    pub music_id: Option<String>,
    pub title: Option<String>,
    pub seed_id: Option<String>,
    pub feedback_id: Option<String>,
    pub detail_url: Option<String>,
    pub track_token: Option<String>,
    pub file_gain: f64,
    pub length: u32,
    pub rating: SongRating,
    pub audio_format: AudioFormat,
}

impl Default for Song {
    fn default() -> Self {
        Self {
            artist: None,
            station_id: None,
            album: None,
            audio_url: None,
            cover_art: None,
            music_id: None,
            title: None,
            seed_id: None,
            feedback_id: None,
            detail_url: None,
            track_token: None,
            file_gain: 0.0,
            length: 0,
            rating: SongRating::None,
            audio_format: AudioFormat::Unknown,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Artist {
    pub name: Option<String>,
    pub music_id: Option<String>,
    pub seed_id: Option<String>,
    pub score: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub artists: Vec<Artist>,
    pub songs: Vec<Song>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Genre {
    pub name: Option<String>,
    pub music_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenreCategory {
    pub name: Option<String>,
    pub genres: Vec<Genre>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StationInfo {
    pub song_seeds: Vec<Song>,
    pub artist_seeds: Vec<Artist>,
    pub station_seeds: Vec<Station>,
    pub feedback: Vec<Song>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserSettings {
    pub username: Option<String>,
    pub explicit_content_filter: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StationMode {
    pub id: i32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_algorithmic: bool,
    pub is_takeover: bool,
    pub active: bool,
}
