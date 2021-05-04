use std::{env, error::Error};

use goauth::{auth::JwtClaims, credentials::Credentials, fetcher::TokenFetcher, scopes::Scope};
use serde_json::{json, Value};
use smpl_jwt::Jwt;
use time::Duration;

use reqwest::Client;

const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const TTS_URL: &str = "https://texttospeech.googleapis.com/v1/text:synthesize";

pub struct GApiClient {
    client: Client,
    fetcher: TokenFetcher,
}

impl GApiClient {
    pub fn new(client: Client) -> Self {
        let cred_file = env::var("GAPI_CREDS_FILE").expect("GAPI_CREDS_FILE not specified");
        let iss = env::var("GAPI_SERVICE_ACCOUNT_EMAIL")
            .expect("GAPI_SERVICE_ACCOUNT_EMAIL not specified");

        let credentials =
            Credentials::from_file(&cred_file).expect("failed to parse GCloud credentials");
        let claims = JwtClaims::new(
            iss,
            &Scope::CloudPlatform,
            String::from(TOKEN_URL),
            None,
            None,
        );
        let jwt = Jwt::new(claims, credentials.rsa_key().unwrap(), None);

        let fetcher =
            TokenFetcher::with_client(client.clone(), jwt, credentials, Duration::new(1, 0));

        Self { fetcher, client }
    }

    pub async fn generate_text(&self, text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        log::debug!("start fetching token");
        let token = self.fetcher.fetch_token().await?;
        log::debug!("start token fetched");

        let response: Value = self
            .client
            .post(TTS_URL)
            .bearer_auth(token.access_token())
            .json(&json!({
                "audioConfig": {
                  "audioEncoding": "MP3",
                  "pitch": -0.2,
                  "speakingRate": 1,
                },
                "input": {
                  "text": text,
                },
                "voice": {
                  "languageCode": "en-GB",
                  "name": "en-GB-Wavenet-D",
                },
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        Ok(base64::decode(
            response["audioContent"].as_str().unwrap_or_default(),
        )?)
    }
}
