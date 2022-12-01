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

use crate::rpc::tungstenite_client::client::TungsteniteRpcClient;
use crate::rpc::{parse_status, result_from_json_response};
use crate::std::rpc::RpcClientError;
use crate::std::XtStatus;
use crate::std::{Api, ApiResult};
use crate::ApiClientError;

use ac_primitives::ExtrinsicParams;
use log::{debug, error, warn};
use serde_json::Value;
use tungstenite::Message;

pub mod client;

use client::MySocket;

pub type OnMessageFn = fn(socket: &mut MySocket) -> Result<String, (ApiClientError, bool)>; // message, (error, retry)

impl<P, Params> Api<P, TungsteniteRpcClient, Params>
where
    Params: ExtrinsicParams,
{
    pub fn default_with_url(url: &str, max_attempts: u8) -> ApiResult<Self> {
        let client = TungsteniteRpcClient::new(url, max_attempts);
        Self::new(client)
    }
}

pub fn on_get_request_msg(socket: &mut MySocket) -> Result<String, (ApiClientError, bool)> {
    let msg = read_until_text_message(socket)
        .map_err(|e| (ApiClientError::TungsteniteWebSocket(e), true))?;
    debug!("Got get_request_msg {}", msg);
    let result_str = serde_json::from_str(msg.as_str())
        .map(|v: serde_json::Value| v["result"].to_string())
        .map_err(|e| (ApiClientError::RpcClient(RpcClientError::Serde(e)), false))?;
    Ok(result_str)
}

pub fn on_extrinsic_msg_until_finalized(
    socket: &mut MySocket,
) -> Result<String, (ApiClientError, bool)> {
    loop {
        let msg = read_until_text_message(socket)
            .map_err(|e| (ApiClientError::TungsteniteWebSocket(e), true))?;
        debug!("receive msg:{:?}", msg);
        match parse_status(msg.as_str()) {
            Ok((XtStatus::Finalized, val)) => return Ok(val.unwrap_or("".to_string())),
            Ok((XtStatus::Future, _)) => {
                warn!("extrinsic has 'future' status. aborting");
                return Err((ApiClientError::UnsupportedXtStatus(XtStatus::Future), false));
            }
            Err(e) => return Err((e, false)),
            _ => continue,
        }
    }
}

pub fn on_extrinsic_msg_until_in_block(
    socket: &mut MySocket,
) -> Result<String, (ApiClientError, bool)> {
    loop {
        let msg = read_until_text_message(socket)
            .map_err(|e| (ApiClientError::TungsteniteWebSocket(e), true))?;
        match parse_status(msg.as_str()) {
            Ok((XtStatus::Finalized, val)) => return Ok(val.unwrap_or("".to_string())),
            Ok((XtStatus::InBlock, val)) => return Ok(val.unwrap_or("".to_string())),
            Ok((XtStatus::Future, _)) => {
                warn!("extrinsic has 'future' status. aborting");
                return Err((ApiClientError::UnsupportedXtStatus(XtStatus::Future), false));
            }
            Err(e) => return Err((e, false)),
            _ => continue,
        }
    }
}

pub fn on_extrinsic_msg_until_ready(
    socket: &mut MySocket,
) -> Result<String, (ApiClientError, bool)> {
    loop {
        let msg = read_until_text_message(socket)
            .map_err(|e| (ApiClientError::TungsteniteWebSocket(e), true))?;
        match parse_status(msg.as_str()) {
            Ok((XtStatus::Finalized, val)) => return Ok(val.unwrap_or("".to_string())),
            Ok((XtStatus::Ready, _)) => return Ok("".to_string()),
            Ok((XtStatus::Future, _)) => {
                warn!("extrinsic has 'future' status. aborting");
                return Err((ApiClientError::UnsupportedXtStatus(XtStatus::Future), false));
            }
            Err(e) => return Err((e, false)),
            _ => continue,
        }
    }
}

pub fn on_extrinsic_msg_until_broadcast(
    socket: &mut MySocket,
) -> Result<String, (ApiClientError, bool)> {
    loop {
        let msg = read_until_text_message(socket)
            .map_err(|e| (ApiClientError::TungsteniteWebSocket(e), true))?;
        match parse_status(msg.as_str()) {
            Ok((XtStatus::Finalized, val)) => return Ok(val.unwrap_or("".to_string())),
            Ok((XtStatus::Broadcast, _)) => return Ok("".to_string()),
            Ok((XtStatus::Future, _)) => {
                warn!("extrinsic has 'future' status. aborting");
                // let _ = end_process(socket, None);
                return Err((ApiClientError::UnsupportedXtStatus(XtStatus::Future), false));
            }
            Err(e) => return Err((e, false)),
            _ => continue,
        }
    }
}

pub fn on_extrinsic_msg_submit_only(
    socket: &mut MySocket,
) -> Result<String, (ApiClientError, bool)> {
    let msg = read_until_text_message(socket)
        .map_err(|e| (ApiClientError::TungsteniteWebSocket(e), true))?;
    debug!("got msg {}", msg);
    return match result_from_json_response(msg.as_str()) {
        Ok(val) => Ok(val),
        Err(e) => Err((ApiClientError::RpcClient(e), false)),
    };
}

pub fn on_subscription_msg(socket: &mut MySocket) -> Result<String, (ApiClientError, bool)> {
    loop {
        let msg = read_until_text_message(socket)
            .map_err(|e| (ApiClientError::TungsteniteWebSocket(e), true))?;
        debug!("got on_subscription_msg {}", msg);
        let value: Value = serde_json::from_str(msg.as_str())
            .map_err(|e| (ApiClientError::RpcClient(RpcClientError::Serde(e)), false))?;

        match value["id"].as_str() {
            Some(_idstr) => {}
            _ => {
                // subscriptions
                debug!("no id field found in response. must be subscription");
                debug!("method: {:?}", value["method"].as_str());
                match value["method"].as_str() {
                    Some("state_storage") => {
                        let changes = &value["params"]["result"]["changes"];
                        match changes[0][1].as_str() {
                            Some(change_set) => return Ok(change_set.to_string()),
                            None => println!("No events happened"),
                        };
                    }
                    Some("chain_finalizedHead") => {
                        let head =
                            serde_json::to_string(&value["params"]["result"]).map_err(|e| {
                                (ApiClientError::RpcClient(RpcClientError::Serde(e)), false)
                            })?;
                        return Ok(head);
                    }
                    _ => error!("unsupported method"),
                }
            }
        };
    }
}

pub(crate) fn read_until_text_message(socket: &mut MySocket) -> Result<String, tungstenite::Error> {
    loop {
        match socket.read_message() {
            Ok(Message::Text(s)) => {
                debug!("receive text: {:?}", s);
                break Ok(s);
            }
            Ok(Message::Binary(_)) => {
                debug!("skip binary msg");
            }
            Ok(Message::Ping(_)) => {
                debug!("skip ping msg");
            }
            Ok(Message::Pong(_)) => {
                debug!("skip ping msg");
            }
            Ok(Message::Close(_)) => {
                debug!("skip close msg");
            }
            Ok(Message::Frame(_)) => {
                debug!("skip frame msg");
            }
            Err(e) => break Err(e),
        }
    }
}
