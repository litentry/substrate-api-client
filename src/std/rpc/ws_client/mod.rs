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
use ac_primitives::ExtrinsicParams;
use log::{debug, error, info, warn};
use std::sync::mpsc::{SendError, Sender as ThreadOut};
use ws::{CloseCode, Handler, Handshake, Message, Result as WsResult, Sender};

use crate::std::rpc::RpcClientError;
use crate::std::XtStatus;
use crate::std::{Api, ApiResult};
use crate::Subscriber;

use crate::rpc::{parse_status, result_from_json_response};
pub use client::WsRpcClient;

pub mod client;

pub type OnMessageFn = fn(msg: Message, out: Sender, result: ThreadOut<String>) -> WsResult<()>;

pub struct RpcClient {
    pub out: Sender,
    pub request: String,
    pub result: ThreadOut<String>,
    pub on_message_fn: OnMessageFn,
}

impl Handler for RpcClient {
    fn on_open(&mut self, _: Handshake) -> WsResult<()> {
        info!("sending request: {}", self.request);
        self.out.send(self.request.clone())?;
        Ok(())
    }

    fn on_message(&mut self, msg: Message) -> WsResult<()> {
        (self.on_message_fn)(msg, self.out.clone(), self.result.clone())
    }
}

impl<P, Params> Api<P, WsRpcClient, Params>
where
    Params: ExtrinsicParams,
{
    pub fn default_with_url(url: &str) -> ApiResult<Self> {
        let client = WsRpcClient::new(url);
        Self::new(client)
    }
}

pub fn on_get_request_msg(msg: Message, out: Sender, result: ThreadOut<String>) -> WsResult<()> {
    out.close(CloseCode::Normal)
        .unwrap_or_else(|_| warn!("Could not close Websocket normally"));

    info!("Got get_request_msg {}", msg);
    let result_str = serde_json::from_str(msg.as_text()?)
        .map(|v: serde_json::Value| v["result"].to_string())
        .map_err(|e| Box::new(RpcClientError::Serde(e)))?;

    result
        .send(result_str)
        .map_err(|e| Box::new(RpcClientError::Send(e)).into())
}

pub fn on_subscription_msg(msg: Message, out: Sender, result: ThreadOut<String>) -> WsResult<()> {
    info!("got on_subscription_msg {}", msg);
    let value: serde_json::Value =
        serde_json::from_str(msg.as_text()?).map_err(|e| Box::new(RpcClientError::Serde(e)))?;

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
                        Some(change_set) => {
                            if let Err(SendError(e)) = result.send(change_set.to_owned()) {
                                debug!("SendError: {}. will close ws", e);
                                out.close(CloseCode::Normal)?;
                            }
                        }
                        None => println!("No events happened"),
                    };
                }
                Some("chain_finalizedHead") => {
                    let head = serde_json::to_string(&value["params"]["result"])
                        .map_err(|e| Box::new(RpcClientError::Serde(e)))?;

                    if let Err(e) = result.send(head) {
                        debug!("SendError: {}. will close ws", e);
                        out.close(CloseCode::Normal)?;
                    }
                }
                _ => error!("unsupported method"),
            }
        }
    };
    Ok(())
}

pub fn on_extrinsic_msg_until_finalized(
    msg: Message,
    out: Sender,
    result: ThreadOut<String>,
) -> WsResult<()> {
    let retstr = msg.as_text().unwrap();
    debug!("got msg {}", retstr);
    match parse_status(retstr) {
        Ok((XtStatus::Finalized, val)) => end_process(out, result, val),
        Ok((XtStatus::Future, _)) => {
            warn!("extrinsic has 'future' status. aborting");
            end_process(out, result, None)
        }
        Err(e) => {
            end_process(out, result, None)?;
            Err(Box::new(e).into())
        }
        _ => Ok(()),
    }
}

pub fn on_extrinsic_msg_until_in_block(
    msg: Message,
    out: Sender,
    result: ThreadOut<String>,
) -> WsResult<()> {
    let retstr = msg.as_text().unwrap();
    debug!("got msg {}", retstr);
    match parse_status(retstr) {
        Ok((XtStatus::Finalized, val)) => end_process(out, result, val),
        Ok((XtStatus::InBlock, val)) => end_process(out, result, val),
        Ok((XtStatus::Future, _)) => end_process(out, result, None),
        Err(e) => {
            end_process(out, result, None)?;
            Err(Box::new(e).into())
        }
        _ => Ok(()),
    }
}

pub fn on_extrinsic_msg_until_broadcast(
    msg: Message,
    out: Sender,
    result: ThreadOut<String>,
) -> WsResult<()> {
    let retstr = msg.as_text().unwrap();
    debug!("got msg {}", retstr);
    match parse_status(retstr) {
        Ok((XtStatus::Finalized, val)) => end_process(out, result, val),
        Ok((XtStatus::Broadcast, _)) => end_process(out, result, None),
        Ok((XtStatus::Future, _)) => end_process(out, result, None),
        Err(e) => {
            end_process(out, result, None)?;
            Err(Box::new(e).into())
        }
        _ => Ok(()),
    }
}

pub fn on_extrinsic_msg_until_ready(
    msg: Message,
    out: Sender,
    result: ThreadOut<String>,
) -> WsResult<()> {
    let retstr = msg.as_text().unwrap();
    debug!("got msg {}", retstr);
    match parse_status(retstr) {
        Ok((XtStatus::Finalized, val)) => end_process(out, result, val),
        Ok((XtStatus::Ready, _)) => end_process(out, result, None),
        Ok((XtStatus::Future, _)) => end_process(out, result, None),
        Err(e) => {
            end_process(out, result, None)?;
            Err(Box::new(e).into())
        }
        _ => Ok(()),
    }
}

pub fn on_extrinsic_msg_submit_only(
    msg: Message,
    out: Sender,
    result: ThreadOut<String>,
) -> WsResult<()> {
    let retstr = msg.as_text().unwrap();
    debug!("got msg {}", retstr);
    match result_from_json_response(retstr) {
        Ok(val) => end_process(out, result, Some(val)),
        Err(e) => {
            end_process(out, result, None)?;
            Err(Box::new(e).into())
        }
    }
}

fn end_process(out: Sender, result: ThreadOut<String>, value: Option<String>) -> WsResult<()> {
    // return result to calling thread
    debug!("Thread end result :{:?} value:{:?}", result, value);
    let val = value.unwrap_or_else(|| "".to_string());

    out.close(CloseCode::Normal)
        .unwrap_or_else(|_| warn!("Could not close WebSocket normally"));

    result
        .send(val)
        .map_err(|e| Box::new(RpcClientError::Send(e)).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::RpcClientError;
    use std::assert_matches::assert_matches;
    use std::fmt::Debug;

    fn assert_extrinsic_err<T: Debug>(result: Result<T, RpcClientError>, msg: &str) {
        assert_matches!(result.unwrap_err(), RpcClientError::Extrinsic(
			m,
		) if &m == msg)
    }

    #[test]
    fn result_from_json_response_works() {
        let msg = r#"{"jsonrpc":"2.0","result":"0xe7640c3e8ba8d10ed7fed07118edb0bfe2d765d3ea2f3a5f6cf781ae3237788f","id":"3"}"#;

        assert_eq!(
            result_from_json_response(msg).unwrap(),
            "0xe7640c3e8ba8d10ed7fed07118edb0bfe2d765d3ea2f3a5f6cf781ae3237788f"
        );
    }

    #[test]
    fn result_from_json_response_errs_on_error_response() {
        let _err_raw =
            r#"{"code":-32602,"message":"Invalid params: invalid hex character: h, at 284."}"#;

        let err_msg = format!(
            "extrinsic error code {}: {}: {}",
            -32602, "Invalid params: invalid hex character: h, at 284.", ""
        );

        let msg = r#"{"jsonrpc":"2.0","error":{"code":-32602,"message":"Invalid params: invalid hex character: h, at 284."},"id":"3"}"#;

        assert_extrinsic_err(result_from_json_response(msg), &err_msg)
    }

    #[test]
    fn extrinsic_status_parsed_correctly() {
        let msg = "{\"jsonrpc\":\"2.0\",\"result\":7185,\"id\":\"3\"}";
        assert_eq!(parse_status(msg).unwrap(), (XtStatus::Unknown, None));

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":\"ready\",\"subscription\":7185}}";
        assert_eq!(parse_status(msg).unwrap(), (XtStatus::Ready, None));

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":{\"broadcast\":[\"QmfSF4VYWNqNf5KYHpDEdY8Rt1nPUgSkMweDkYzhSWirGY\",\"Qmchhx9SRFeNvqjUK4ZVQ9jH4zhARFkutf9KhbbAmZWBLx\",\"QmQJAqr98EF1X3YfjVKNwQUG9RryqX4Hv33RqGChbz3Ncg\"]},\"subscription\":232}}";
        assert_eq!(
            parse_status(msg).unwrap(),
            (
                XtStatus::Broadcast,
                Some(
                    "[\"QmfSF4VYWNqNf5KYHpDEdY8Rt1nPUgSkMweDkYzhSWirGY\",\"Qmchhx9SRFeNvqjUK4ZVQ9jH4zhARFkutf9KhbbAmZWBLx\",\"QmQJAqr98EF1X3YfjVKNwQUG9RryqX4Hv33RqGChbz3Ncg\"]"
                        .to_string()
                )
            )
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":{\"inBlock\":\"0x3104d362365ff5ddb61845e1de441b56c6722e94c1aee362f8aa8ba75bd7a3aa\"},\"subscription\":232}}";
        assert_eq!(
            parse_status(msg).unwrap(),
            (
                XtStatus::InBlock,
                Some(
                    "\"0x3104d362365ff5ddb61845e1de441b56c6722e94c1aee362f8aa8ba75bd7a3aa\""
                        .to_string()
                )
            )
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":{\"finalized\":\"0x934385b11c483498e2b5bca64c2e8ef76ad6c74d3372a05595d3a50caf758d52\"},\"subscription\":7185}}";
        assert_eq!(
            parse_status(msg).unwrap(),
            (
                XtStatus::Finalized,
                Some(
                    "\"0x934385b11c483498e2b5bca64c2e8ef76ad6c74d3372a05595d3a50caf758d52\""
                        .to_string()
                )
            )
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"method\":\"author_extrinsicUpdate\",\"params\":{\"result\":\"future\",\"subscription\":2}}";
        assert_eq!(parse_status(msg).unwrap(), (XtStatus::Future, None));

        let msg = "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":-32700,\"message\":\"Parse error\"},\"id\":null}";
        assert_extrinsic_err(
            parse_status(msg),
            "extrinsic error code -32700: Parse error: ",
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":1010,\"message\":\"Invalid Transaction\",\"data\":\"Bad Signature\"},\"id\":\"4\"}";
        assert_extrinsic_err(
            parse_status(msg),
            "extrinsic error code 1010: Invalid Transaction: Bad Signature",
        );

        let msg = "{\"jsonrpc\":\"2.0\",\"error\":{\"code\":1001,\"message\":\"Extrinsic has invalid format.\"},\"id\":\"0\"}";
        assert_extrinsic_err(
            parse_status(msg),
            "extrinsic error code 1001: Extrinsic has invalid format.: ",
        );

        let msg = r#"{"jsonrpc":"2.0","error":{"code":1002,"message":"Verification Error: Execution(Wasmi(Trap(Trap { kind: Unreachable })))","data":"RuntimeApi(\"Execution(Wasmi(Trap(Trap { kind: Unreachable })))\")"},"id":"3"}"#;
        assert_extrinsic_err(
            parse_status(msg),
            "extrinsic error code 1002: Verification Error: Execution(Wasmi(Trap(Trap { kind: Unreachable }))): RuntimeApi(\"Execution(Wasmi(Trap(Trap { kind: Unreachable })))\")"
        );
    }
}
