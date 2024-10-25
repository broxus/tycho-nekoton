pub fn now_sec_u64() -> u64 {
    use crate::traits::TrustMe;
    use std::time::SystemTime;

    (SystemTime::now().duration_since(SystemTime::UNIX_EPOCH))
        .trust_me()
        .as_secs()
}
