use std::time::Duration;

#[derive(Debug, Clone)]
pub struct BlockchainOptions {
    /// Poll interval between message execution result.
    ///
    /// Default: `1 sec`
    pub message_poll_interval: Duration,

    /// Amount of attempts to check message execution result.
    ///
    /// Default: `10`
    pub message_poll_attempts: u16,
}

impl Default for BlockchainOptions {
    fn default() -> Self {
        Self {
            message_poll_interval: Duration::from_secs(1),
            message_poll_attempts: 10,
        }
    }
}
