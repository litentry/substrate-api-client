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
use std::net::TcpStream;
use std::sync::mpsc::Sender as ThreadOut;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

use crate::rpc::tungstenite_client::{
    on_extrinsic_msg_submit_only, on_extrinsic_msg_until_broadcast,
    on_extrinsic_msg_until_finalized, on_extrinsic_msg_until_in_block,
    on_extrinsic_msg_until_ready, on_get_request_msg, on_subscription_msg, read_until_text_message,
    OnMessageFn,
};
use log::{debug, error, info, warn};
use serde_json::Value;
use sp_core::H256 as Hash;
use tungstenite::{
    client::connect_with_config,
    protocol::{frame::coding::CloseCode, CloseFrame},
    stream::MaybeTlsStream,
    Message, WebSocket,
};
use url::Url;

use crate::std::rpc::json_req;
use crate::std::ApiClientError;
use crate::std::ApiResult;
use crate::std::FromHexString;
use crate::std::RpcClient as RpcClientTrait;
use crate::std::XtStatus;
use crate::Subscriber;

pub(crate) type MySocket = WebSocket<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
pub struct TungsteniteRpcClient {
    url: String,
    max_attempts: u8,
}

impl TungsteniteRpcClient {
    pub fn new(url: &str, max_attempts: u8) -> TungsteniteRpcClient {
        TungsteniteRpcClient {
            url: url.to_string(),
            max_attempts,
        }
    }

    fn direct_rpc_request(
        &self,
        json_req: String,
        on_message_fn: OnMessageFn,
    ) -> ApiResult<String> {
        let url = Url::parse(self.url.as_str()).map_err(|e| ApiClientError::Other(e.into()))?;
        let mut current_attempt: u8 = 1;
        let mut socket: MySocket;

        let send_request =
            |socket: &mut MySocket, json_req: String| -> Result<String, (ApiClientError, bool)> {
                match socket.write_message(Message::Text(json_req.clone())) {
                    Ok(_) => {
                        let r = on_message_fn(socket);
                        let _ = socket.close(None);
                        r
                    }
                    Err(e) => {
                        let _ = socket.close(None);
                        Err((ApiClientError::TungsteniteWebSocket(e), true))
                    }
                }
            };

        while current_attempt <= self.max_attempts {
            match connect_with_config(url.clone(), None, 20) {
                Ok(res) => {
                    socket = res.0;
                    let response = res.1;
                    debug!(
                        "Connected to the server. Response HTTP code: {}",
                        response.status()
                    );
                    if socket.can_read() {
                        current_attempt = 1;
                        let ping = socket.read_message();
                        if ping.is_err() {
                            error!(
                                "failed to read ping message. error: {:?}",
                                ping.unwrap_err()
                            );
                        } else {
                            debug!(
                                "read ping message:{:?}. Connected successfully.",
                                ping.unwrap()
                            );

                            match send_request(&mut socket, json_req.clone()) {
                                Ok(e) => return Ok(e),
                                Err((e, retry)) => {
                                    error!(
                                        "failed to send request. error:{:?}, retry:{:?}",
                                        e, retry
                                    );
                                    if retry {
                                        if current_attempt == self.max_attempts {
                                            return Err(e);
                                        }
                                    } else {
                                        return Err(e);
                                    }
                                }
                            };
                        }
                    }
                    let _ = socket.close(Some(CloseFrame {
                        code: CloseCode::Normal,
                        reason: Default::default(),
                    }));
                }
                Err(e) => {
                    error!(
                        "failed to connect the server({:?}). error: {:?}",
                        self.url, e
                    );
                }
            };
            warn!(
                "attempt to request after {} sec. current attempt {}",
                5 * current_attempt,
                current_attempt
            );
            sleep(Duration::from_secs((5 * current_attempt) as u64));
            current_attempt += 1;
        }
        Err(ApiClientError::ConnectionAttemptsExceeded)
    }
}

impl RpcClientTrait for TungsteniteRpcClient {
    fn get_request(&self, jsonreq: Value) -> ApiResult<String> {
        self.direct_rpc_request(jsonreq.to_string(), on_get_request_msg)
    }

    fn send_extrinsic(
        &self,
        xthex_prefixed: String,
        exit_on: XtStatus,
    ) -> ApiResult<Option<sp_core::H256>> {
        // Todo: Make all variants return a H256: #175.

        let jsonreq = match exit_on {
            XtStatus::SubmitOnly => json_req::author_submit_extrinsic(&xthex_prefixed).to_string(),
            _ => json_req::author_submit_and_watch_extrinsic(&xthex_prefixed).to_string(),
        };

        match exit_on {
            XtStatus::Finalized => {
                let res = self.direct_rpc_request(jsonreq, on_extrinsic_msg_until_finalized)?;
                info!("finalized: {}", res);
                Ok(Some(Hash::from_hex(res)?))
            }
            XtStatus::InBlock => {
                let res = self.direct_rpc_request(jsonreq, on_extrinsic_msg_until_in_block)?;
                info!("inBlock: {}", res);
                Ok(Some(Hash::from_hex(res)?))
            }
            XtStatus::Broadcast => {
                let res = self.direct_rpc_request(jsonreq, on_extrinsic_msg_until_broadcast)?;
                info!("broadcast: {}", res);
                Ok(None)
            }
            XtStatus::Ready => {
                let res = self.direct_rpc_request(jsonreq, on_extrinsic_msg_until_ready)?;
                info!("ready: {}", res);
                Ok(None)
            }
            XtStatus::SubmitOnly => {
                let res = self.direct_rpc_request(jsonreq, on_extrinsic_msg_submit_only)?;
                info!("submitted xt: {}", res);
                Ok(None)
            }
            _ => Err(ApiClientError::UnsupportedXtStatus(exit_on)),
        }
    }
}

impl Subscriber for TungsteniteRpcClient {
    fn start_subscriber(&self, json_req: String, result_in: ThreadOut<String>) -> ApiResult<()> {
        self.start_rpc_client_thread(json_req, result_in, on_subscription_msg)
    }
}

impl TungsteniteRpcClient {
    pub fn get(&self, json_req: String, result_in: ThreadOut<String>) -> ApiResult<()> {
        self.start_rpc_client_thread(json_req, result_in, on_get_request_msg)
    }

    pub fn send_extrinsic(&self, json_req: String, result_in: ThreadOut<String>) -> ApiResult<()> {
        self.start_rpc_client_thread(json_req, result_in, on_extrinsic_msg_submit_only)
    }

    pub fn send_extrinsic_until_ready(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> ApiResult<()> {
        self.start_rpc_client_thread(json_req, result_in, on_extrinsic_msg_until_ready)
    }

    pub fn send_extrinsic_and_wait_until_broadcast(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> ApiResult<()> {
        self.start_rpc_client_thread(json_req, result_in, on_extrinsic_msg_until_broadcast)
    }

    pub fn send_extrinsic_and_wait_until_in_block(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> ApiResult<()> {
        self.start_rpc_client_thread(json_req, result_in, on_extrinsic_msg_until_in_block)
    }

    pub fn send_extrinsic_and_wait_until_finalized(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
    ) -> ApiResult<()> {
        self.start_rpc_client_thread(json_req, result_in, on_extrinsic_msg_until_finalized)
    }

    pub fn start_rpc_client_thread(
        &self,
        json_req: String,
        result_in: ThreadOut<String>,
        on_message_fn: OnMessageFn,
    ) -> ApiResult<()> {
        let url = Url::parse(self.url.as_str()).map_err(|e| ApiClientError::URL(e))?;
        let max_attempts = self.max_attempts;

        thread::spawn(move || {
            let mut current_attempt: u8 = 1;
            let mut socket: MySocket;

            while current_attempt <= max_attempts {
                match connect_with_config(url.clone(), None, 20) {
                    Ok(res) => {
                        socket = res.0;
                        let response = res.1;
                        debug!(
                            "Connected to the server. Response HTTP code: {}",
                            response.status()
                        );
                        match socket.write_message(Message::Text(json_req.clone())) {
                            Ok(_) => {}
                            Err(e) => {
                                error!("write msg error:{:?}", e);
                            }
                        }
                        if socket.can_read() {
                            current_attempt = 1;
                            // After sending the subscription request, there will be a response(result)
                            let msg_from_req = read_until_text_message(&mut socket);
                            match msg_from_req {
                                Ok(msg_from_req) => {
                                    debug!("response message: {:?}", msg_from_req);
                                    loop {
                                        let msg = read_until_text_message(&mut socket);
                                        if msg.is_err() {
                                            error!("err:{:?}", msg.unwrap_err());
                                            break;
                                        }

                                        match on_message_fn(&mut socket) {
                                            Ok(msg) => {
                                                if let Err(e) = result_in.send(msg) {
                                                    error!("failed to send channel: {:?} ", e);
                                                    return;
                                                }
                                            }
                                            Err((e, retry)) => {
                                                error!("on_subscription_msg: {:?}", e);
                                                if retry {
                                                    if current_attempt == max_attempts {
                                                        return;
                                                    }
                                                } else {
                                                    return;
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("response message error:{:?}", e);
                                }
                            };
                        }
                        let _ = socket.close(Some(CloseFrame {
                            code: CloseCode::Normal,
                            reason: Default::default(),
                        }));
                    }
                    Err(e) => {
                        error!("failed to connect the server({:?}). error: {:?}", url, e);
                    }
                };
                warn!(
                    "attempt to request after {} sec. current attempt {}",
                    5 * current_attempt,
                    current_attempt
                );
                sleep(Duration::from_secs((5 * current_attempt) as u64));
                current_attempt += 1;
            }
            error!("max request attempts exceeded");
        });
        Ok(())
    }
}
