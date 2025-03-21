use nekoton_utils::time::Clock;

pub fn get_gen_timings(clock: &dyn Clock, last_transaction_tl: u64) -> (u32, u64) {
    let (gen_utime, gen_lt) = {
        pub const UNKNOWN_TRANSACTION_LT_OFFSET: u64 = 10;
        let now_ms = clock.now_ms_u64();
        (
            (now_ms / 1000) as u32,
            last_transaction_tl + UNKNOWN_TRANSACTION_LT_OFFSET,
        )
    };

    (gen_utime, gen_lt)
}
