use codec::Compact;

use crate::{Api, compose_extrinsic};

use super::xt_primitives::*;

pub const LITENTRY_MODULE: &str = "LitentryModule";
pub const REGISTER_IDENTITY: &str = "register_identity";

pub type RegisterIdentityFn = ([u8; 2]);

pub type RegisterIdentityXt = UncheckedExtrinsicV3<RegisterIdentityFn>;

pub fn register_identity(api: Api) -> RegisterIdentityXt {
    compose_extrinsic!(
		api,
		LITENTRY_MODULE,
		REGISTER_IDENTITY
	)
}