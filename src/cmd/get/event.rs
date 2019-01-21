use chrono::prelude::*;

#[derive(Debug, Clone)]
pub struct LogEvent {
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub stream_name: Option<String>,
}
