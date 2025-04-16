static mut DEBUG_LOGGING_ENABLED: bool = false;

/// Enables or disables debug logging.
pub fn enable_debug_logging(enable: bool) {
    unsafe {
        DEBUG_LOGGING_ENABLED = enable;
    }
}

pub struct DebugLog {
    prefix: &'static str,
}

impl DebugLog {
    pub const fn new(prefix: &'static str) -> DebugLog {
        DebugLog { prefix }
    }

    pub fn print(&self, msg: &str) {
        unsafe {
            if DEBUG_LOGGING_ENABLED {
                println!("[{}] {}", self.prefix, msg);
            }
        }
    }
}
