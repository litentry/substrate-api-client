/*
   Copyright 2019 Supercomputing Systems AG

   Licensed under the Apache License, Version 2.0 (the "License");
   you may not use this file except in compliance with the License.
   You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

   Unless required by applicable law or agreed to in writing, software
   distributed under the License is distributed on an "AS IS" BASIS,
   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
   See the License for the specific language governing permissions and
   limitations under the License.

*/
use log::info;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ApiClientError;

#[cfg(feature = "ws-client")]
pub use ws_client::WsRpcClient;

#[cfg(feature = "ws-client")]
pub mod ws_client;

#[cfg(feature = "tungstenite-client")]
pub use tungstenite_client::client::TungsteniteRpcClient;

#[cfg(feature = "tungstenite-client")]
pub mod tungstenite_client;

pub mod json_req;

#[derive(Debug, thiserror::Error)]
pub enum RpcClientError {
    #[error("Serde json error: {0}")]
    Serde(#[from] serde_json::error::Error),
    #[error("Extrinsic Error: {0}")]
    Extrinsic(String),
    #[error("mpsc send Error: {0}")]
    Send(#[from] std::sync::mpsc::SendError<String>),
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum XtStatus {
    // Todo: some variants to not return a hash with `send_extrinsics`: #175.
    Finalized,
    InBlock,
    Broadcast,
    Ready,
    Future,
    /// uses `author_submit`
    SubmitOnly,
    Error,
    Unknown,
}

// Exact structure from
// https://github.com/paritytech/substrate/blob/master/client/rpc-api/src/state/helpers.rs
// Adding manually so we don't need sc-rpc-api, which brings in async dependencies
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadProof<Hash> {
    /// Block hash used to generate the proof
    pub at: Hash,
    /// A proof used to prove that storage entries are included in the storage trie
    pub proof: Vec<sp_core::Bytes>,
}

pub(crate) fn parse_status(msg: &str) -> Result<(XtStatus, Option<String>), ApiClientError> {
    let value: serde_json::Value = serde_json::from_str(msg)?;

    if value["error"].as_object().is_some() {
        return Err(ApiClientError::RpcClient(into_extrinsic_err(&value)));
    }

    match value["params"]["result"].as_object() {
        Some(obj) => {
            if let Some(hash) = obj.get("finalized") {
                info!("finalized: {:?}", hash);
                Ok((XtStatus::Finalized, Some(hash.to_string())))
            } else if let Some(hash) = obj.get("inBlock") {
                info!("inBlock: {:?}", hash);
                Ok((XtStatus::InBlock, Some(hash.to_string())))
            } else if let Some(array) = obj.get("broadcast") {
                info!("broadcast: {:?}", array);
                Ok((XtStatus::Broadcast, Some(array.to_string())))
            } else {
                Ok((XtStatus::Unknown, None))
            }
        }
        None => match value["params"]["result"].as_str() {
            Some("ready") => Ok((XtStatus::Ready, None)),
            Some("future") => Ok((XtStatus::Future, None)),
            Some(&_) => Ok((XtStatus::Unknown, None)),
            None => Ok((XtStatus::Unknown, None)),
        },
    }
}

/// Todo: this is the code that was used in `parse_status` Don't we want to just print the
/// error as is instead of introducing our custom format here?
pub(crate) fn into_extrinsic_err(resp_with_err: &Value) -> RpcClientError {
    let err_obj = resp_with_err["error"].as_object().unwrap();

    let error = err_obj
        .get("message")
        .map_or_else(|| "", |e| e.as_str().unwrap());
    let code = err_obj
        .get("code")
        .map_or_else(|| -1, |c| c.as_i64().unwrap());
    let details = err_obj
        .get("data")
        .map_or_else(|| "", |d| d.as_str().unwrap());

    RpcClientError::Extrinsic(format!(
        "extrinsic error code {}: {}: {}",
        code, error, details
    ))
}

pub(crate) fn result_from_json_response(resp: &str) -> Result<String, RpcClientError> {
    let value: serde_json::Value = serde_json::from_str(resp)?;

    let resp = value["result"]
        .as_str()
        .ok_or_else(|| into_extrinsic_err(&value))?;

    Ok(resp.to_string())
}
