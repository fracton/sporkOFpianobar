use crate::crypt::{encrypt_hex, CryptError};
use crate::model::{CreateStationKind, Session, SongRating, Station, User};
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub const RPC_HOST: &str = "tuner.pandora.com";
pub const RPC_PATH: &str = "/services/json/?";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub kind: RequestKind,
    pub secure: bool,
    pub url_path: String,
    pub post_data: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestKind {
    PartnerLogin,
    UserLogin,
    GetStations,
    GetPlaylist,
    RateSong,
    AddFeedback,
    RenameStation,
    DeleteStation,
    Search,
    CreateStation,
    AddSeed,
    AddTiredSong,
    SetQuickMix,
    GetGenreStations,
    TransformStation,
    Explain,
    BookmarkSong,
    BookmarkArtist,
    GetStationInfo,
    DeleteFeedback,
    DeleteSeed,
    GetSettings,
    ChangeSettings,
    GetStationModes,
    SetStationMode,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StationRef {
    pub id: String,
}

impl From<&Station> for StationRef {
    fn from(station: &Station) -> Self {
        Self {
            id: station.id.clone(),
        }
    }
}

impl From<&str> for StationRef {
    fn from(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddFeedback {
    pub station_id: String,
    pub track_token: String,
    pub rating: SongRating,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenameStation {
    pub station_id: String,
    pub new_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateStation {
    pub token: String,
    pub kind: CreateStationKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AddSeed {
    pub station_id: String,
    pub music_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeleteSeed {
    pub seed_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChangeSettings {
    pub current_username: String,
    pub current_password: String,
    pub new_username: Option<String>,
    pub new_password: Option<String>,
    pub explicit_content_filter: Option<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SetStationMode {
    pub station_id: String,
    pub mode_id: u32,
}

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("request requires partner authentication")]
    MissingPartnerAuth,
    #[error("request requires user authentication")]
    MissingUserAuth,
    #[error("request requires a positive or negative song rating")]
    InvalidRating,
    #[error("system clock is before UNIX epoch")]
    InvalidSystemClock,
    #[error("request encryption failed")]
    Crypt(#[from] CryptError),
}

#[derive(Clone, Debug)]
pub struct RequestBuilder {
    session: Session,
    out_key: Vec<u8>,
}

impl RequestBuilder {
    pub fn new(session: Session, out_key: impl Into<Vec<u8>>) -> Self {
        Self {
            session,
            out_key: out_key.into(),
        }
    }

    pub fn partner_login(&self) -> Result<Request, RequestError> {
        let body = json!({
            "username": self.session.partner.user,
            "password": self.session.partner.password,
            "deviceModel": self.session.partner.device,
            "version": "5",
            "includeUrls": true,
        });

        self.finish_plain(
            RequestKind::PartnerLogin,
            true,
            "auth.partnerLogin",
            None,
            body,
        )
    }

    pub fn user_login(
        &self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<Request, RequestError> {
        let partner_auth_token = self.partner_auth_token()?;
        let partner_id = self.partner_id()?;
        let timestamp = self.timestamp()?;
        let body = json!({
            "loginType": "user",
            "username": username.as_ref(),
            "password": password.as_ref(),
            "partnerAuthToken": partner_auth_token,
            "syncTime": timestamp,
        });
        let query = format!(
            "auth_token={}&partner_id={}",
            urlencoding::encode(partner_auth_token),
            partner_id
        );

        self.finish_encrypted(
            RequestKind::UserLogin,
            true,
            "auth.userLogin",
            Some(&query),
            body,
        )
    }

    pub fn get_stations(&self) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::GetStations,
            false,
            "user.getStationList",
            json!({ "returnAllStations": true }),
        )
    }

    pub fn get_playlist(&self, station: impl Into<StationRef>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::GetPlaylist,
            true,
            "station.getPlaylist",
            json!({
                "stationToken": station.into().id,
                "includeTrackLength": true,
            }),
        )
    }

    pub fn add_feedback(&self, data: AddFeedback) -> Result<Request, RequestError> {
        if data.rating != SongRating::Love && data.rating != SongRating::Ban {
            return Err(RequestError::InvalidRating);
        }

        self.authed(
            RequestKind::AddFeedback,
            false,
            "station.addFeedback",
            json!({
                "stationToken": data.station_id,
                "trackToken": data.track_token,
                "isPositive": data.rating == SongRating::Love,
            }),
        )
    }

    pub fn rate_song(
        &self,
        station_id: impl Into<String>,
        track_token: impl Into<String>,
        rating: SongRating,
    ) -> Result<Request, RequestError> {
        let mut request = self.add_feedback(AddFeedback {
            station_id: station_id.into(),
            track_token: track_token.into(),
            rating,
        })?;
        request.kind = RequestKind::RateSong;
        Ok(request)
    }

    pub fn rename_station(&self, data: RenameStation) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::RenameStation,
            false,
            "station.renameStation",
            json!({
                "stationToken": data.station_id,
                "stationName": data.new_name,
            }),
        )
    }

    pub fn delete_station(&self, station: impl Into<StationRef>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::DeleteStation,
            false,
            "station.deleteStation",
            json!({ "stationToken": station.into().id }),
        )
    }

    pub fn search(&self, search_text: impl AsRef<str>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::Search,
            false,
            "music.search",
            json!({ "searchText": search_text.as_ref() }),
        )
    }

    pub fn create_station(&self, data: CreateStation) -> Result<Request, RequestError> {
        let body = match data.kind {
            CreateStationKind::MusicToken => json!({ "musicToken": data.token }),
            CreateStationKind::Song => json!({
                "trackToken": data.token,
                "musicType": "song",
            }),
            CreateStationKind::Artist => json!({
                "trackToken": data.token,
                "musicType": "artist",
            }),
        };

        self.authed(
            RequestKind::CreateStation,
            false,
            "station.createStation",
            body,
        )
    }

    pub fn add_seed(&self, data: AddSeed) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::AddSeed,
            false,
            "station.addMusic",
            json!({
                "musicToken": data.music_id,
                "stationToken": data.station_id,
            }),
        )
    }

    pub fn add_tired_song(&self, track_token: impl AsRef<str>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::AddTiredSong,
            false,
            "user.sleepSong",
            json!({ "trackToken": track_token.as_ref() }),
        )
    }

    pub fn set_quick_mix<'a>(
        &self,
        stations: impl IntoIterator<Item = &'a Station>,
    ) -> Result<Request, RequestError> {
        let quick_mix_station_ids: Vec<&str> = stations
            .into_iter()
            .filter(|station| station.use_quick_mix && !station.is_quick_mix)
            .map(|station| station.id.as_str())
            .collect();

        self.authed(
            RequestKind::SetQuickMix,
            false,
            "user.setQuickMix",
            json!({ "quickMixStationIds": quick_mix_station_ids }),
        )
    }

    pub fn get_genre_stations(&self) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::GetGenreStations,
            false,
            "station.getGenreStations",
            json!({}),
        )
    }

    pub fn transform_station(
        &self,
        station: impl Into<StationRef>,
    ) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::TransformStation,
            false,
            "station.transformSharedStation",
            json!({ "stationToken": station.into().id }),
        )
    }

    pub fn explain(&self, track_token: impl AsRef<str>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::Explain,
            false,
            "track.explainTrack",
            json!({ "trackToken": track_token.as_ref() }),
        )
    }

    pub fn bookmark_song(&self, track_token: impl AsRef<str>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::BookmarkSong,
            false,
            "bookmark.addSongBookmark",
            json!({ "trackToken": track_token.as_ref() }),
        )
    }

    pub fn bookmark_artist(&self, track_token: impl AsRef<str>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::BookmarkArtist,
            false,
            "bookmark.addArtistBookmark",
            json!({ "trackToken": track_token.as_ref() }),
        )
    }

    pub fn get_station_info(
        &self,
        station: impl Into<StationRef>,
    ) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::GetStationInfo,
            false,
            "station.getStation",
            json!({
                "stationToken": station.into().id,
                "includeExtendedAttributes": true,
                "includeExtraParams": true,
            }),
        )
    }

    pub fn delete_feedback(&self, feedback_id: impl AsRef<str>) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::DeleteFeedback,
            false,
            "station.deleteFeedback",
            json!({ "feedbackId": feedback_id.as_ref() }),
        )
    }

    pub fn delete_seed(&self, data: DeleteSeed) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::DeleteSeed,
            false,
            "station.deleteMusic",
            json!({ "seedId": data.seed_id }),
        )
    }

    pub fn get_settings(&self) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::GetSettings,
            false,
            "user.getSettings",
            json!({}),
        )
    }

    pub fn change_settings(&self, data: ChangeSettings) -> Result<Request, RequestError> {
        let mut body = Map::new();
        body.insert("userInitiatedChange".to_string(), json!(true));
        body.insert("currentUsername".to_string(), json!(data.current_username));
        body.insert("currentPassword".to_string(), json!(data.current_password));

        if let Some(enabled) = data.explicit_content_filter {
            body.insert("isExplicitContentFilterEnabled".to_string(), json!(enabled));
        }
        if let Some(new_username) = data.new_username {
            body.insert("newUsername".to_string(), json!(new_username));
        }
        if let Some(new_password) = data.new_password {
            body.insert("newPassword".to_string(), json!(new_password));
        }

        self.authed(
            RequestKind::ChangeSettings,
            true,
            "user.changeSettings",
            Value::Object(body),
        )
    }

    pub fn get_station_modes(
        &self,
        station: impl Into<StationRef>,
    ) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::GetStationModes,
            true,
            "interactiveradio.v1.getAvailableModesSimple",
            json!({ "stationId": station.into().id }),
        )
    }

    pub fn set_station_mode(&self, data: SetStationMode) -> Result<Request, RequestError> {
        self.authed(
            RequestKind::SetStationMode,
            true,
            "interactiveradio.v1.setAndGetAvailableModes",
            json!({
                "stationId": data.station_id,
                "modeId": data.mode_id,
            }),
        )
    }

    fn authed(
        &self,
        kind: RequestKind,
        secure: bool,
        method: &str,
        mut body: Value,
    ) -> Result<Request, RequestError> {
        let User {
            listener_id,
            auth_token,
        } = self.user()?;
        let partner_id = self.partner_id()?;

        body["userAuthToken"] = json!(auth_token);
        body["syncTime"] = json!(self.timestamp()?);

        let query = format!(
            "auth_token={}&partner_id={}&user_id={}",
            urlencoding::encode(auth_token),
            partner_id,
            listener_id
        );

        self.finish_encrypted(kind, secure, method, Some(&query), body)
    }

    fn finish_plain(
        &self,
        kind: RequestKind,
        secure: bool,
        method: &str,
        query: Option<&str>,
        body: Value,
    ) -> Result<Request, RequestError> {
        Ok(Request {
            kind,
            secure,
            url_path: url_path(method, query),
            post_data: body.to_string(),
        })
    }

    fn finish_encrypted(
        &self,
        kind: RequestKind,
        secure: bool,
        method: &str,
        query: Option<&str>,
        body: Value,
    ) -> Result<Request, RequestError> {
        Ok(Request {
            kind,
            secure,
            url_path: url_path(method, query),
            post_data: encrypt_hex(&self.out_key, &body.to_string())?,
        })
    }

    fn user(&self) -> Result<&User, RequestError> {
        self.session
            .user
            .as_ref()
            .ok_or(RequestError::MissingUserAuth)
    }

    fn partner_auth_token(&self) -> Result<&str, RequestError> {
        self.session
            .partner
            .auth_token
            .as_deref()
            .ok_or(RequestError::MissingPartnerAuth)
    }

    fn partner_id(&self) -> Result<u32, RequestError> {
        self.session
            .partner
            .id
            .ok_or(RequestError::MissingPartnerAuth)
    }

    fn timestamp(&self) -> Result<i64, RequestError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| RequestError::InvalidSystemClock)?;
        Ok(now.as_secs() as i64 - self.session.time_offset)
    }
}

fn url_path(method: &str, query: Option<&str>) -> String {
    match query {
        Some(query) => format!("{RPC_PATH}method={method}&{query}"),
        None => format!("{RPC_PATH}method={method}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Partner, Session};

    fn builder() -> RequestBuilder {
        let partner = Partner::authenticated(
            "android",
            "partner-password",
            "android-generic",
            "partner token /+",
            42,
        );
        let user = User {
            listener_id: "listener-id".to_string(),
            auth_token: "user token /+".to_string(),
        };
        RequestBuilder::new(
            Session::authenticated(partner, user, 0),
            b"6#26FRL$ZWD".to_vec(),
        )
    }

    #[test]
    fn partner_login_is_plaintext_and_secure() {
        let request = RequestBuilder::new(
            Session::new(Partner::android_default()),
            b"6#26FRL$ZWD".to_vec(),
        )
        .partner_login()
        .unwrap();

        assert_eq!(request.kind, RequestKind::PartnerLogin);
        assert!(request.secure);
        assert_eq!(request.url_path, "/services/json/?method=auth.partnerLogin");
        assert!(request.post_data.contains(r#""username":"android""#));
        assert!(request
            .post_data
            .contains(r#""deviceModel":"android-generic""#));
        assert!(request.post_data.contains(r#""includeUrls":true"#));
    }

    #[test]
    fn user_login_uses_partner_auth_query() {
        let request = builder().user_login("listener", "secret").unwrap();

        assert_eq!(request.kind, RequestKind::UserLogin);
        assert!(request.secure);
        assert_eq!(
            request.url_path,
            "/services/json/?method=auth.userLogin&auth_token=partner%20token%20%2F%2B&partner_id=42"
        );
        assert!(!request.post_data.contains("listener"));
    }

    #[test]
    fn authed_request_adds_user_query() {
        let request = builder().get_stations().unwrap();

        assert_eq!(
            request.url_path,
            "/services/json/?method=user.getStationList&auth_token=user%20token%20%2F%2B&partner_id=42&user_id=listener-id"
        );
        assert!(!request.secure);
    }

    #[test]
    fn quick_mix_filters_quick_mix_station_itself() {
        let stations = vec![
            Station {
                id: "a".to_string(),
                name: None,
                is_creator: false,
                is_quick_mix: false,
                use_quick_mix: true,
                seed_id: None,
            },
            Station {
                id: "b".to_string(),
                name: None,
                is_creator: false,
                is_quick_mix: true,
                use_quick_mix: true,
                seed_id: None,
            },
        ];

        let request = builder().set_quick_mix(&stations).unwrap();

        assert_eq!(request.kind, RequestKind::SetQuickMix);
        assert!(request.post_data.len() > 16);
    }

    #[test]
    fn invalid_rating_is_rejected() {
        let err = builder()
            .add_feedback(AddFeedback {
                station_id: "station".to_string(),
                track_token: "track".to_string(),
                rating: SongRating::Tired,
            })
            .unwrap_err();

        assert!(matches!(err, RequestError::InvalidRating));
    }
}
