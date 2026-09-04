use serde::{Deserialize, Serialize};

use crate::error::WireError;
use crate::model::{Dependency, Event, Proposal, Settings, Task};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub protocol_version: u32,
    pub request_id: String,
    pub base_input_revision: u64,
    pub now: String,
    pub settings: Settings,
    pub events: Vec<Event>,
    pub tasks: Vec<Task>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub protocol_version: u32,
    pub request_id: String,
    pub ok: bool,
    pub proposal: Option<Proposal>,
    pub error: Option<WireError>,
}

impl Response {
    pub fn success(request: &Request, proposal: Proposal) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request.request_id.clone(),
            ok: true,
            proposal: Some(proposal),
            error: None,
        }
    }

    pub fn failure(request_id: impl Into<String>, error: WireError) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_id: request_id.into(),
            ok: false,
            proposal: None,
            error: Some(error),
        }
    }
}
