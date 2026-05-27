use atrium_api::types::string::Datetime;
use atrium_api::types::string::RecordKey;
use bsky_sdk::record::Record;
use bsky_sdk::rich_text::RichText;
use bsky_sdk::BskyAgent;
use serde::Deserialize;
use serde::Serialize;

use crate::backends::BackendConfig;
use crate::backends::{Backend, Item105Error, Item105Result};

pub struct BskyBackend {
    user: String,
    pass: String,
    agent: Option<BskyAgent>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BskyConfig {
    pub user: String,
    pub pass: String,
}

pub struct BskyBackendConfig {}

impl BackendConfig for BskyBackendConfig {
    fn name(&self) -> String {
        "Bluesky".into()
    }

    fn configure(&self, config: String) -> Item105Result<Box<impl Backend>> {
        let x: BskyConfig = serde_json::from_str(config.as_str()).or(Err(Item105Error {
            msg: "Bluesky backend configuration error".into(),
        }))?;

        Item105Result::Ok(Box::new(BskyBackend {
            user: x.user,
            pass: x.pass,
            agent: None,
        }))
    }
}

impl BskyBackend {
    pub async fn format_msg(&self, msg: String) -> Item105Result<RichText> {
        RichText::new_with_detect_facets(msg)
            .await
            .map_err(|e| Item105Error { msg: e.to_string() })
    }

    pub async fn record_for_msg(
        &self,
        msg: String,
    ) -> Item105Result<atrium_api::app::bsky::feed::post::RecordData> {
        let formatted_msg = self.format_msg(msg).await?;

        Ok(atrium_api::app::bsky::feed::post::RecordData {
            created_at: Datetime::now(),
            embed: None,
            entities: None,
            facets: formatted_msg.facets,
            labels: None,
            langs: None,
            reply: None,
            tags: None,
            text: formatted_msg.text,
        })
    }
}
impl Backend for BskyBackend {
    async fn post(&self, msg: String) -> Item105Result<()> {
        let agent = self.agent.as_ref().ok_or(Item105Error {
            msg: "No agent present in backend".into(),
        })?;

        let post_record = self.record_for_msg(msg).await?;

        agent
            .create_record(post_record)
            .await
            .map_err(|e| Item105Error {
                msg: format!("Could not post: {e}"),
            })?;
        Ok(())
    }

    async fn login(&mut self) -> Item105Result<()> {
        if self.agent.is_some() {
            return Err(Item105Error {
                msg: "Already logged in".to_string(),
            });
        }
        let agent = BskyAgent::builder()
            .build()
            .await
            .map_err(|e| Item105Error {
                msg: format!("Could not login: Could not create agent builder: {e}"),
            })?;
        agent
            .login(self.user.clone(), self.pass.clone())
            .await
            .map_err(|e| Item105Error {
                msg: format!("Could not login: {e}"),
            })?;
        self.agent = Some(agent);
        Ok(())
    }

    async fn status(&self, msg: String) -> Item105Result<()> {
        let agent = self.agent.as_ref().ok_or(Item105Error {
            msg: "No agent present in backend".into(),
        })?;
        let r = atrium_api::app::bsky::actor::status::RecordData {
            created_at: Datetime::now(),
            duration_minutes: None,
            embed: None,
            status: msg,
        };
        r.put(
            agent,
            RecordKey::new("self".to_string()).map_err(|e| Item105Error {
                msg: format!("Could not update status: Could not make self key: {e}"),
            })?,
        )
        .await
        .map_err(|e| Item105Error {
            msg: format!("Could not update status: {e}"),
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::bsky_backend::BskyBackend;

    #[tokio::test]
    async fn test_rich_text() {
        let backend = BskyBackend {
            user: "user".to_string(),
            pass: "pass".to_string(),
            agent: None,
        };
        let msg = "@item105.eyesec.it for https://www.wsj.com/.";

        let r = backend
            .format_msg(msg.to_string())
            .await
            .expect("Should be able to build record");

        let segments = r.segments();
        assert_eq!(segments.len(), 4);

        assert!(segments[0].text == "@item105.eyesec.it" && segments[0].mention().is_some());
        assert!(segments[1].text == " for ");
        assert!(segments[2].text == "https://www.wsj.com/" && segments[2].link().is_some());
        assert!(segments[3].text == ".");
    }
}
