use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct RequestEnvelope {
    pub request_id: String,
    pub command: String,
    pub payload: Value,
}

#[derive(Debug, Serialize)]
pub struct ResponseEnvelope {
    pub request_id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    pub kind: String,
    pub message: String,
}

impl ResponseEnvelope {
    pub fn ok(request_id: String, payload: Value) -> Self {
        Self {
            request_id,
            ok: true,
            payload: Some(payload),
            error: None,
        }
    }

    pub fn err(request_id: String, kind: &str, message: String) -> Self {
        Self {
            request_id,
            ok: false,
            payload: None,
            error: Some(ErrorPayload {
                kind: kind.to_string(),
                message,
            }),
        }
    }
}
