use codec::Compact;

#[cfg(feature = "std")]
use crate::{Api, compose_extrinsic};
use primitives::crypto::Pair;
use super::xt_primitives::*;
use node_primitives::Hash;

pub const LITENTRY_MODULE: &str = "LitentryModule";
pub const REGISTER_IDENTITY: &str = "register_identity";
pub const CREATE_AUTHORIZED_TOKEN: &str = "create_authorized_token";

pub type RegisterIdentityFn = ([u8; 2]);
pub type RegisterIdentityXt<Pair> = UncheckedExtrinsicV3<RegisterIdentityFn, Pair>;

// pub type CreateAuthorizedTokenFn = ([u8; 2], GenericAddress, Hash, Compact<u128>, Compact<u64>, Compact<u64>, Compact<u64>);
pub type CreateAuthorizedTokenFn = ([u8; 2], GenericAddress, Hash, u128, Vec<u8>, u64, u64);
pub type CreateAuthorizedTokenXt<Pair> = UncheckedExtrinsicV3<CreateAuthorizedTokenFn, Pair>;

#[cfg(feature = "std")]
impl<P: Pair> Api<P> {
    pub fn register_identity(&self) -> RegisterIdentityXt<P> {
        compose_extrinsic!(
            self,
            LITENTRY_MODULE,
            REGISTER_IDENTITY
        )
    }

    pub fn create_authorized_token(&self, to: GenericAddress, identity_hash: Hash,
                                cost: u128, data: Vec<u8>, data_type: u64, expired: u64) -> CreateAuthorizedTokenXt<P> {
        compose_extrinsic!(
            self,
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
}