use chrono::prelude::*;
use rss::extension::ExtensionMap;
use rss::Channel;
use serde::de::Error;
use serde_json::Value;
use std::str::FromStr;

pub async fn synchronous_download(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .use_rustls_tls() // Make sure that we get TLS.
        .build()?;
    String::from_utf8(
        client
            .request(reqwest::Method::GET, url)
            .header(
                reqwest::header::USER_AGENT,
                "SEC Insights - Item502 and Item105 Twitter Bots hawkinwh@ucmail.uc.edu",
            )
            .send()
            .await?
            .bytes()
            .await?
            .to_vec(),
    )
    .map_err(Into::into)
}

pub fn json_parse(value: String) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str::<Value>(value.as_str())?)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filing {
    pub items: String,
    pub form: String,
    pub time: DateTime<FixedOffset>,
}

impl Filing {
    fn new(items: String, form: String, time: DateTime<FixedOffset>) -> Self {
        Filing { items, form, time }
    }
}

pub fn extract_filings_metadata(json: Value) -> Result<Vec<Filing>, Box<dyn std::error::Error>> {
    let filings = json
        .as_object()
        .and_then(|obj| obj.get("filings"))
        .ok_or(serde_json::Error::custom("Could not find filings."))?;
    let recent = filings
        .as_object()
        .and_then(|obj| obj.get("recent"))
        .ok_or(serde_json::Error::custom("Could not find recent"))?;
    let forms = recent
        .as_object()
        .and_then(|obj| obj.get("form"))
        .and_then(|obj| obj.as_array())
        .ok_or(serde_json::Error::custom("Could not find forms."))?;
    let items = recent
        .as_object()
        .and_then(|obj| obj.get("items"))
        .and_then(|obj| obj.as_array())
        .ok_or(serde_json::Error::custom("Could not find items."))?;
    let times: Vec<DateTime<FixedOffset>> = recent
        .as_object()
        .and_then(|obj| obj.get("acceptanceDateTime"))
        .and_then(|obj| obj.as_array())
        .map(|obj| {
            obj.iter()
                .map_while(|raw| {
                    raw.as_str()
                        .and_then(|raw_str| DateTime::parse_from_rfc3339(raw_str).ok())
                })
                .collect()
        })
        .ok_or(serde_json::Error::custom(
            "Could not find acceptance date/times.",
        ))?;
    if forms.len() != items.len() || items.len() != times.len() {
        return Err(Box::new(serde_json::Error::custom(
            "Corrupt filings JSON: Length of arrays do not match",
        )));
    }

    let mut result: Vec<Filing> = Vec::new();

    for index in 0..forms.len() {
        result.push(Filing::new(
            items[index].to_string(),
            forms[index].to_string(),
            times[index],
        ));
    }
    Ok(result)
}

#[test]
fn test_parse_recent_form_slim() {
    let file = std::fs::File::open("test_data/slim.json").unwrap();
    let raw_value: Result<Value, serde_json::Error> = serde_json::from_reader(file);
    assert!(raw_value.is_ok());
    let extract_result = extract_filings_metadata(raw_value.unwrap());
    assert!(extract_result.is_ok());
    assert!(extract_result.unwrap().len() == 3);
}

#[test]
fn test_parse_recent_form_uneven() {
    let file = std::fs::File::open("test_data/uneven.json").unwrap();
    let raw_value: Result<Value, serde_json::Error> = serde_json::from_reader(file);
    assert!(raw_value.is_ok());
    let extract_result = extract_filings_metadata(raw_value.unwrap());
    assert!(
        extract_result.is_err_and(|err| err.to_string().contains("Length of arrays do not match"))
    );
}

#[test]
fn test_parse_recent_form_filter_by_date() {
    let now: DateTime<Utc> = Utc.with_ymd_and_hms(2024, 4, 23, 0, 0, 0).unwrap();
    let file = std::fs::File::open("test_data/slim.json").unwrap();
    let raw_value: Result<Value, serde_json::Error> = serde_json::from_reader(file);
    assert!(raw_value.is_ok());

    let extract_result = extract_filings_metadata(raw_value.unwrap());
    let filtered_result = extract_result
        .unwrap()
        .into_iter()
        .filter(|filing| filing.time > now);
    assert!(filtered_result.collect::<Vec<Filing>>().len() == 1);
}

pub fn parse_rss(atom_string: String) -> Result<Channel, Box<dyn std::error::Error>> {
    Channel::from_str(atom_string.as_str()).map_err(Into::into)
}

pub fn cik_from_extensions(extensions: &ExtensionMap) -> Option<String> {
    let e = extensions.get("edgar").unwrap();
    let f = e.get("xbrlFiling").unwrap();
    for item in f {
        match item.children().get("cikNumber") {
            Some(cik_number) => return Some(cik_number[0].value()?.to_string()),
            _ => {
                continue;
            }
        }
    }
    None
}

pub fn acceptance_datetime_from_extensions(extensions: &ExtensionMap) -> Option<DateTime<FixedOffset>> {
    extensions
        .get("edgar")
        .and_then(|edgar| edgar.get("xbrlFiling"))
        .and_then(|accepted_dates| accepted_dates[0].children.get("acceptanceDatetime"))
        .and_then(|accepted_date| accepted_date[0].value())
        .and_then(|accepted_date| NaiveDateTime::parse_from_str(accepted_date, "%Y%m%d%H%M%S").ok())
        .map(|naive_accepted_datetime| naive_accepted_datetime.and_utc().fixed_offset())
}

