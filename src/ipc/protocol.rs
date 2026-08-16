use serde::{Deserialize, Serialize};

use crate::dispatch::{Command, DispatchError, DispatchResult, Event};

/// Line-delimited JSON request received over the UNIX socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcRequest {
    pub id: Option<u64>,
    #[serde(flatten)]
    pub command: Command,
}

/// Line-delimited JSON response sent back to the IPC client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub id: Option<u64>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<DispatchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn success(id: Option<u64>, result: DispatchResult) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<u64>, error: DispatchError) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error.to_string()),
        }
    }
}

/// Envelope for streamed events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcEventMessage {
    #[serde(flatten)]
    pub event: Event,
}
