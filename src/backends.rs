use regex::Regex;
/*
use chrono::{prelude::*};
use regex::Regex;
use reqwest::Method;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::error::Error as StdError;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufReader, Read};
use std::net::IpAddr;
use std::time::SystemTime;
use twitter_v2::data::Tweet;
use twitter_v2::id::IntoNumericId;
use twitter_v2::{authorization::Oauth1aToken, ApiResult};
use url::Url;

fn parse_alert_regular_expression(
    s: String,
) -> Result<Regex, Box<dyn std::error::Error + Send + Sync + 'static>> {
    s.parse().or(Err(String::into(format!(
        "Could not parse alert to a valid regular expression."
    ))))
}

fn bearer_from_config(
    request: &reqwest::Request,
    config: &TwitterConfig,
) -> Result<reqwest::header::HeaderValue, Box<dyn std::error::Error>> {
    let token = oauth1::Token::from_parts(
        config.consumer_key.clone(),
        config.consumer_secret.clone(),
        config.access_token.clone(),
        config.access_secret.clone(),
    );
    let method = request.method().as_str();
    let url = {
        let mut url = request.url().clone();
        url.set_query(None);
        url.set_fragment(None);
        url
    };
    let request = request.url().query_pairs().collect::<BTreeSet<_>>();
    oauth1::authorize(method, url, &request, &token, oauth1::HmacSha1)
        .parse()
        .map_err(|_| Item105Errors::InvalidAuthorizationHeader.into())
}

async fn update_bio(
    message: &str,
    configuration: &TwitterConfig,
) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .use_rustls_tls() // Make sure that we get TLS.
        .build()?;

    let mut request = client
        .request(
            Method::POST,
            Url::parse("https://api.twitter.com/1.1/account/update_profile.json")?,
        )
        .query(&[("description", message)])
        .build()?;
    let authorization_header = bearer_from_config(&request, configuration)?;
    request
        .headers_mut()
        .insert(reqwest::header::AUTHORIZATION, authorization_header);

    client.execute(request).await.or_else(|e| Err(e.into()))
}

#[derive(Deserialize, Debug, Clone)]
struct TwitterConfig {
    pub consumer_key: String,
    pub consumer_secret: String,
    pub access_token: String,
    pub access_secret: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PrometheusConfig {
    ip: IpAddr,
    port: u16,
}

#[derive(Deserialize, Debug, Clone)]
struct Item105Config {
    pub bio: Option<String>,
    pub alert: String,
    pub twitter: TwitterConfig,
    pub prometheus: Option<PrometheusConfig>,
}

impl Item105Config {
    pub fn config_from_file(file: &mut File) -> Result<Self, Item105Errors> {
        let mut reader = BufReader::new(file);
        let mut file_contents: String = Default::default();
        let _ = reader.read_to_string(&mut file_contents);

        TryInto::try_into(file_contents)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item105Errors {
    ParseError,
    InvalidAuthorizationHeader,
}

impl Display for Item105Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError => {
                write!(f, "There was a parse error.")
            }
            Self::InvalidAuthorizationHeader => {
                write!(f, "Invalid authorization header.")
            }
        }
    }
}

impl StdError for Item105Errors {}

impl TryFrom<String> for Item105Config {
    type Error = Item105Errors;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        serde_json::from_str(raw.as_str())
            .or(Err(Item105Errors::ParseError))
            .and_then(|config: Self| {
                parse_alert_regular_expression(config.alert.clone())
                    .or(Err(Item105Errors::ParseError))
                    .and(Ok(config))
            })
    }
}

#[test]
fn test_Item105Config_config_from_file() {
    let mut file = std::fs::File::open("configs/test.json").unwrap();
    if let Ok(config) = Item105Config::config_from_file(&mut file) {
        assert!(config.twitter.access_secret == "access_secret");
        assert!(config.twitter.access_token == "access_token");
        assert!(config.twitter.consumer_key == "consumer_key");
        assert!(config.twitter.consumer_secret == "consumer_secret");
        assert!(config.alert == "anything");
        assert!(config.bio.unwrap() == "my bio");
        return;
    }
    assert!(true == false);
}

#[test]
fn test_Item105Config_config_from_file_bad_alert() {
    let mut file = std::fs::File::open("configs/test_bad_alert.json").unwrap();
    assert!(
        Item105Config::config_from_file(&mut file).is_err_and(|e| e == Item105Errors::ParseError)
    );
}

#[test]
fn test_Item105Config_config_from_file_no_bio() {
    let mut file = std::fs::File::open("configs/test_no_bio.json").unwrap();
    if let Ok(config) = Item105Config::config_from_file(&mut file) {
        assert!(config.bio.is_none());
        return;
    }
    assert!(false)
}

#[test]
fn test_Item105Config_config_good_prometheus() {
    let mut file = std::fs::File::open("configs/test_good_prometheus.json").unwrap();
    if let Ok(config) = Item105Config::config_from_file(&mut file) {
        if let Some(prometheus) = config.prometheus {
            assert!(prometheus.ip.is_ipv4());
            return;
        }
    }
    assert!(false)
}

#[test]
fn test_Item105Config_config_bad_prometheus_ip() {
    let mut file = std::fs::File::open("configs/test_bad_prometheus.json").unwrap();
    let parse_result = Item105Config::config_from_file(&mut file);
    assert!(parse_result.is_err());
}

async fn tweet(
    message: &str,
    configuration: &TwitterConfig,
    reply_id: Option<impl IntoNumericId>,
) -> ApiResult<Oauth1aToken, Tweet, ()> {
    let token = Oauth1aToken::new(
        configuration.consumer_key.clone(),
        configuration.consumer_secret.clone(),
        configuration.access_token.clone(),
        configuration.access_secret.clone(),
    );
    let api = twitter_v2::TwitterApi::new(token);
    let mut tweet_builder = api.post_tweet();

    if let Some(reply_id) = reply_id {
        tweet_builder.in_reply_to_tweet_id(reply_id);
    }
    tweet_builder.text(message.to_string());
    tweet_builder.send().await
}

#[tokio::test]
async fn test_update_bio() {
    let mut file = std::fs::File::open("502_config.json").unwrap();
    if let Ok(config) = Item105Config::config_from_file(&mut file) {
        let result = update_bio(
            "This is a test of the update_bio_local method.",
            &config.twitter,
        )
        .await;
        assert!(result.is_ok());
        return;
    }
    assert!(false == true)
}

async fn tweet_hello(configuration: &TwitterConfig) -> ApiResult<Oauth1aToken, Tweet, ()> {
    let system_time = SystemTime::now();
    let datetime: DateTime<Utc> = system_time.into();
    let hello_content = format!(
        "Test, test, test: This bot says, 'Hello,' at ... {}",
        datetime.format("%d/%m/%Y %T")
    );
    tweet(hello_content.as_str(), configuration, None::<u64>).await
}

#[tokio::test]
async fn test_tweet() {
    let mut file = std::fs::File::open("502_config.json").unwrap();
    if let Ok(config) = Item105Config::config_from_file(&mut file) {
        let result = tweet_hello(&config.twitter).await;
        if let Ok(result) = result {
            assert!(true)
        } else {
            let error = result.err();
            println!("error: {:?}", error);
            assert!(false);
        }
        return;
    }
    assert!(false == true)
}
*/
use serde::Deserialize;
use serde_json::Value;
use std::fmt::Display;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Debug)]
pub struct Item105Error {
    pub msg: String,
}

impl Display for Item105Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
    }
}

pub type Item105Result<T> = core::result::Result<T, Item105Error>;

pub trait Backend {
    async fn post(&self, msg: String) -> Item105Result<()>;
    async fn login(&mut self) -> Item105Result<()>;
    async fn status(&self, msg: String) -> Item105Result<()>;
}

pub trait BackendConfig {
    fn name(&self) -> String;
    fn configure(&self, config: String) -> Item105Result<Box<impl Backend>>;
}

#[derive(Deserialize, Debug, Clone)]
pub struct Item105Config {
    pub backend: String,
    pub alert: String,
    pub config: Value,
}

impl Item105Config {
    pub fn config_from_file(file: &mut File) -> Result<Self, Item105Error> {
        let mut reader = BufReader::new(file);
        let mut file_contents: String = Default::default();
        let _ = reader.read_to_string(&mut file_contents);

        TryInto::try_into(file_contents)
    }
}

fn parse_alert_regular_expression(
    s: String,
) -> Result<Regex, Box<dyn std::error::Error + Send + Sync + 'static>> {
    s.parse().or(Err(String::into(
        "Could not parse alert to a valid regular expression.".into(),
    )))
}

impl TryFrom<String> for Item105Config {
    type Error = Item105Error;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        serde_json::from_str(raw.as_str())
            .or(Err(Item105Error {
                msg: "Parse error".into(),
            }))
            .and_then(|config: Self| {
                parse_alert_regular_expression(config.alert.clone())
                    .or(Err(Item105Error {
                        msg: "Could not parse alert regular expression".into(),
                    }))
                    .and(Ok(config))
            })
    }
}
