use atrium_api::types::string::Datetime;
use atrium_api::types::string::RecordKey;
use bsky_sdk::record::Record;
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

impl Backend for BskyBackend {
    async fn post(&self, msg: String) -> Item105Result<()> {
        let agent = self.agent.as_ref().ok_or(Item105Error {
            msg: "No agent present in backend".into(),
        })?;
        agent
            .create_record(atrium_api::app::bsky::feed::post::RecordData {
                created_at: Datetime::now(),
                embed: None,
                entities: None,
                facets: None,
                labels: None,
                langs: None,
                reply: None,
                tags: None,
                text: msg,
            })
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
