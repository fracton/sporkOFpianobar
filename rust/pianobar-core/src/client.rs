use crate::model::{
    AudioQuality, GenreCategory, Partner, SearchResult, Session, Song, Station, StationInfo,
    StationMode, UserSettings,
};
use crate::request::{
    AddFeedback, AddSeed, ChangeSettings, CreateStation, DeleteSeed, RenameStation, Request,
    RequestBuilder, RequestError, SetStationMode, StationRef, RPC_HOST,
};
use crate::response::{
    parse_created_station, parse_empty, parse_explain, parse_genre_stations, parse_get_settings,
    parse_partner_login, parse_playlist, parse_search, parse_set_station_mode, parse_station_info,
    parse_station_modes, parse_stations, parse_user_login, ResponseError,
};
use reqwest::StatusCode;
use thiserror::Error;

const USER_AGENT: &str = "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_10_3) \
AppleWebKit/537.36 (KHTML, like Gecko) Chrome/44.0.2403.89 Safari/537.36";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpConfig {
    pub rpc_host: String,
    pub rpc_tls_port: u16,
    pub rpc_plain_port: u16,
    pub max_retry: u32,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            rpc_host: RPC_HOST.to_string(),
            rpc_tls_port: 443,
            rpc_plain_port: 80,
            max_retry: 5,
        }
    }
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error(transparent)]
    Request(#[from] RequestError),
    #[error(transparent)]
    Response(#[from] ResponseError),
    #[error("network request failed")]
    Network(#[from] reqwest::Error),
    #[error("Pandora HTTP endpoint returned {0}")]
    HttpStatus(StatusCode),
}

#[derive(Clone)]
pub struct PandoraClient {
    http: reqwest::Client,
    config: HttpConfig,
    session: Session,
    in_key: Vec<u8>,
    out_key: Vec<u8>,
}

impl PandoraClient {
    pub fn new(
        session: Session,
        in_key: impl Into<Vec<u8>>,
        out_key: impl Into<Vec<u8>>,
    ) -> Result<Self, ClientError> {
        Self::with_config(session, in_key, out_key, HttpConfig::default())
    }

    pub fn with_config(
        session: Session,
        in_key: impl Into<Vec<u8>>,
        out_key: impl Into<Vec<u8>>,
        config: HttpConfig,
    ) -> Result<Self, ClientError> {
        let http = reqwest::Client::builder().user_agent(USER_AGENT).build()?;
        Ok(Self {
            http,
            config,
            session,
            in_key: in_key.into(),
            out_key: out_key.into(),
        })
    }

    pub fn android_default() -> Result<Self, ClientError> {
        Self::new(
            Session::new(Partner::android_default()),
            b"R=U!LH$O2B#".to_vec(),
            b"6#26FRL$ZWD".to_vec(),
        )
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn into_session(self) -> Session {
        self.session
    }

    pub async fn login(
        &mut self,
        username: impl AsRef<str>,
        password: impl AsRef<str>,
    ) -> Result<(), ClientError> {
        let partner_request = self.builder().partner_login()?;
        let partner_response = self.execute(&partner_request).await?;
        let partner_login = parse_partner_login(&partner_response, &self.in_key)?;

        self.session.partner.auth_token = Some(partner_login.auth_token);
        self.session.partner.id = Some(partner_login.partner_id);
        self.session.time_offset = partner_login.time_offset;

        let user_request = self.builder().user_login(username, password)?;
        let user_response = self.execute(&user_request).await?;
        let user_login = parse_user_login(&user_response)?;
        self.session.user = Some(user_login.user);

        Ok(())
    }

    pub async fn get_stations(&self) -> Result<Vec<Station>, ClientError> {
        let request = self.builder().get_stations()?;
        let response = self.execute(&request).await?;
        Ok(parse_stations(&response)?)
    }

    pub async fn get_playlist(
        &self,
        station: impl Into<StationRef>,
        quality: AudioQuality,
    ) -> Result<Vec<Song>, ClientError> {
        let request = self.builder().get_playlist(station)?;
        let response = self.execute(&request).await?;
        Ok(parse_playlist(&response, quality)?)
    }

    pub async fn add_feedback(&self, data: AddFeedback) -> Result<(), ClientError> {
        let request = self.builder().add_feedback(data)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn rename_station(&self, data: RenameStation) -> Result<(), ClientError> {
        let request = self.builder().rename_station(data)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn delete_station(&self, station: impl Into<StationRef>) -> Result<(), ClientError> {
        let request = self.builder().delete_station(station)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn search(&self, search_text: impl AsRef<str>) -> Result<SearchResult, ClientError> {
        let request = self.builder().search(search_text)?;
        let response = self.execute(&request).await?;
        Ok(parse_search(&response)?)
    }

    pub async fn add_seed(&self, data: AddSeed) -> Result<(), ClientError> {
        let request = self.builder().add_seed(data)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn create_station(&self, data: CreateStation) -> Result<Station, ClientError> {
        let request = self.builder().create_station(data)?;
        let response = self.execute(&request).await?;
        Ok(parse_created_station(&response)?)
    }

    pub async fn add_tired_song(&self, track_token: impl AsRef<str>) -> Result<(), ClientError> {
        let request = self.builder().add_tired_song(track_token)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn set_quick_mix<'a>(
        &self,
        stations: impl IntoIterator<Item = &'a Station>,
    ) -> Result<(), ClientError> {
        let request = self.builder().set_quick_mix(stations)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn get_genre_stations(&self) -> Result<Vec<GenreCategory>, ClientError> {
        let request = self.builder().get_genre_stations()?;
        let response = self.execute(&request).await?;
        Ok(parse_genre_stations(&response)?)
    }

    pub async fn transform_station(
        &self,
        station: impl Into<StationRef>,
    ) -> Result<(), ClientError> {
        let request = self.builder().transform_station(station)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn explain(
        &self,
        track_token: impl AsRef<str>,
    ) -> Result<Option<String>, ClientError> {
        let request = self.builder().explain(track_token)?;
        let response = self.execute(&request).await?;
        Ok(parse_explain(&response)?)
    }

    pub async fn bookmark_song(&self, track_token: impl AsRef<str>) -> Result<(), ClientError> {
        let request = self.builder().bookmark_song(track_token)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn bookmark_artist(&self, track_token: impl AsRef<str>) -> Result<(), ClientError> {
        let request = self.builder().bookmark_artist(track_token)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn get_station_info(
        &self,
        station: impl Into<StationRef>,
    ) -> Result<StationInfo, ClientError> {
        let request = self.builder().get_station_info(station)?;
        let response = self.execute(&request).await?;
        Ok(parse_station_info(&response)?)
    }

    pub async fn delete_feedback(&self, feedback_id: impl AsRef<str>) -> Result<(), ClientError> {
        let request = self.builder().delete_feedback(feedback_id)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn delete_seed(&self, data: DeleteSeed) -> Result<(), ClientError> {
        let request = self.builder().delete_seed(data)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn get_settings(&self) -> Result<UserSettings, ClientError> {
        let request = self.builder().get_settings()?;
        let response = self.execute(&request).await?;
        Ok(parse_get_settings(&response)?)
    }

    pub async fn change_settings(&self, data: ChangeSettings) -> Result<(), ClientError> {
        let request = self.builder().change_settings(data)?;
        let response = self.execute(&request).await?;
        Ok(parse_empty(&response)?)
    }

    pub async fn get_station_modes(
        &self,
        station: impl Into<StationRef>,
    ) -> Result<Vec<StationMode>, ClientError> {
        let request = self.builder().get_station_modes(station)?;
        let response = self.execute(&request).await?;
        Ok(parse_station_modes(&response)?)
    }

    pub async fn set_station_mode(&self, data: SetStationMode) -> Result<(), ClientError> {
        let requested_id = data.mode_id;
        let request = self.builder().set_station_mode(data)?;
        let response = self.execute(&request).await?;
        Ok(parse_set_station_mode(&response, requested_id)?)
    }

    async fn execute(&self, request: &Request) -> Result<String, ClientError> {
        let url = self.url_for(request);
        let max_retry = self.config.max_retry.max(1);
        let mut attempts = 0;

        loop {
            attempts += 1;
            let result = self
                .http
                .post(&url)
                .header(reqwest::header::CONTENT_TYPE, "text/plain")
                .body(request.post_data.clone())
                .send()
                .await;

            match result {
                Ok(response) => {
                    let status = response.status();
                    if !status.is_success() {
                        if attempts < max_retry && is_temporary_status(status) {
                            continue;
                        }
                        return Err(ClientError::HttpStatus(status));
                    }
                    return Ok(response.text().await?);
                }
                Err(err) => {
                    if attempts < max_retry && is_temporary_error(&err) {
                        continue;
                    }
                    return Err(err.into());
                }
            }
        }
    }

    fn builder(&self) -> RequestBuilder {
        RequestBuilder::new(self.session.clone(), self.out_key.clone())
    }

    fn url_for(&self, request: &Request) -> String {
        let (scheme, port) = if request.secure {
            ("https", self.config.rpc_tls_port)
        } else {
            ("http", self.config.rpc_plain_port)
        };
        format!(
            "{scheme}://{}:{port}{}",
            self.config.rpc_host, request.url_path
        )
    }
}

fn is_temporary_status(status: StatusCode) -> bool {
    status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT
}

fn is_temporary_error(err: &reqwest::Error) -> bool {
    err.is_timeout() || err.is_connect() || err.is_request()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::User;

    fn authed_client() -> PandoraClient {
        let partner = Partner::authenticated(
            "android",
            "partner-password",
            "android-generic",
            "partner-token",
            42,
        );
        let user = User {
            listener_id: "listener".to_string(),
            auth_token: "user-token".to_string(),
        };
        PandoraClient::with_config(
            Session::authenticated(partner, user, 0),
            b"R=U!LH$O2B#".to_vec(),
            b"6#26FRL$ZWD".to_vec(),
            HttpConfig {
                rpc_host: "example.invalid".to_string(),
                rpc_tls_port: 8443,
                rpc_plain_port: 8080,
                max_retry: 3,
            },
        )
        .unwrap()
    }

    #[test]
    fn builds_secure_and_plain_urls() {
        let client = authed_client();
        let stations = client.builder().get_stations().unwrap();
        let playlist = client.builder().get_playlist("station").unwrap();

        assert!(client
            .url_for(&stations)
            .starts_with("http://example.invalid:8080/services/json/?"));
        assert!(client
            .url_for(&playlist)
            .starts_with("https://example.invalid:8443/services/json/?"));
    }

    #[test]
    fn default_client_starts_without_user_auth() {
        let client = PandoraClient::android_default().unwrap();

        assert_eq!(client.session().partner.user, "android");
        assert!(client.session().user.is_none());
    }
}
