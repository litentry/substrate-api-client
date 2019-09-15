use codec::Compact;

use crate::{Api, compose_extrinsic};
use node_primitives::Hash;
use super::xt_primitives::*;

pub const LITENTRY_MODULE: &str = "LitentryModule";
pub const REGISTER_IDENTITY: &str = "register_identity";
pub const CREATE_AUTHORIZED_TOKEN: &str = "create_authorized_token";

pub type RegisterIdentityFn = ([u8; 2]);
pub type RegisterIdentityXt = UncheckedExtrinsicV3<RegisterIdentityFn>;

// pub type CreateAuthorizedTokenFn = ([u8; 2], GenericAddress, Hash, Compact<u128>, Compact<u64>, Compact<u64>, Compact<u64>);
pub type CreateAuthorizedTokenFn = ([u8; 2], GenericAddress, Hash, u128, u64, u64, u64);
pub type CreateAuthorizedTokenXt = UncheckedExtrinsicV3<CreateAuthorizedTokenFn>;

pub fn register_identity(api: Api) -> RegisterIdentityXt {
    compose_extrinsic!(
		api,
		LITENTRY_MODULE,
		REGISTER_IDENTITY
	)
}

pub fn create_authorized_token(api: Api, to: GenericAddress, identity_hash: Hash,
                               cost: u128, data: u64, data_type: u64, expired: u64) -> CreateAuthorizedTokenXt {
    compose_extrinsic!(
        api,
        LITENTRY_MODULE,
        CREATE_AUTHORIZED_TOKEN,
        to,
        identity_hash,
        cost,
        data,
        data_type,
        expired
//        Compact(data),
//        Compact(data_type),
//        Compact(expired)
    )
}