use std::time::Duration;

// Constants
pub const MAX_PIXELS_PER_BATCH: usize = 10;
pub const BATCH_DELAY_MINUTES: u64 = 31;
pub const MAX_RETRIES: u32 = 10;
pub const RETRY_DELAY: Duration = Duration::from_secs(120); // 2 minutes
pub const BOARD_SIZE: usize = 250;

