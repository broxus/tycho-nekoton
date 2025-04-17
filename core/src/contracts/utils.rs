use crate::models::GenTimings;
use everscale_types::abi::{AbiValue, NamedAbiValue};
use nekoton_utils::time::Clock;
use num_bigint::BigUint;

const ANSWER_ID: &str = "_answer_id";
pub fn answer_id() -> NamedAbiValue {
    AbiValue::Uint(32, BigUint::from(0u32)).named(ANSWER_ID)
}

pub fn get_gen_timings(clock: &dyn Clock, last_transaction_tl: u64) -> GenTimings {
    let (gen_utime, gen_lt) = {
        pub const UNKNOWN_TRANSACTION_LT_OFFSET: u64 = 10;
        let now_ms = clock.now_sec_u64();
        (
            now_ms as u32,
            last_transaction_tl + UNKNOWN_TRANSACTION_LT_OFFSET,
        )
    };

    GenTimings { gen_utime, gen_lt }
}
