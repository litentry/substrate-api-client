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
extern crate rand;
use clap::{App, load_yaml};
use node_primitives::Hash;
use substrate_api_client::{
    Api,
    crypto::{AccountKey, CryptoKind},
    extrinsic,
    extrinsic::xt_primitives::*,
};
use substrate_api_client::utils::hexstr_to_hash;
use rand::Rng;

fn main() {
    env_logger::init();
    let mut rng = rand::thread_rng();
    let url = get_node_url_from_cli();
    let identity = get_identity_from_cli();

    // initialize api and set the signer (sender) that is used to sign the extrinsics
    let from = AccountKey::new("//Alice", Some(""), CryptoKind::Sr25519);
    let api = Api::new(format!("ws://{}", url))
        .set_signer(from.clone());

    let to = AccountKey::public_from_suri("//Bob", Some(""), CryptoKind::Sr25519);

//    let result = api.register_identity();
//    println!("[+] Bob's Free Balance is is {}\n", result);

    // generate extrinsic
    // println!("identity hash is {:?}", hexstr_to_hash(identity.clone()));
    let xt = extrinsic::litentry::create_authorized_token(
        api.clone(),
        GenericAddress::from(to),
        hexstr_to_hash(identity),
        // Default::default(),
        rng.gen::<u128>(),
        rng.gen::<u64>(),
        rng.gen::<u64>(),
        rng.gen::<u64>()
    );

    println!("Sending an extrinsic from Alice (Key = {:?}),\n\nto Bob (Key = {:?})\n", from.public(), to);

    println!("[+] Composed extrinsic: {:?}\n", xt);

    //send and watch extrinsic until finalized
    let tx_hash = api.send_extrinsic(xt.hex_encode()).unwrap();
    println!("[+] Transaction got finalized. Hash: {:?}\n", tx_hash);

    // Verify that Bob's free Balance increased
    let result = api.get_free_balance(to);
    println!("[+] Bob's Free Balance is now {}\n", result);
}

pub fn get_node_url_from_cli() -> String {
    let yml = load_yaml!("../../src/examples/cli.yml");
    let matches = App::from_yaml(yml).get_matches();

    let node_ip = matches.value_of("node-server").unwrap_or("127.0.0.1");
    let node_port = matches.value_of("node-port").unwrap_or("9944");
    let url = format!("{}:{}", node_ip, node_port);
    println!("Interacting with node on {}\n", url);
    url
}

pub fn get_identity_from_cli() -> String {
    let yml = load_yaml!("../../src/examples/cli.yml");
    let matches = App::from_yaml(yml).get_matches();

    let identity = matches.value_of("identity-hash").unwrap_or("0x");
    println!("Interacting with node on {}\n", identity);
    identity.to_owned()
}
