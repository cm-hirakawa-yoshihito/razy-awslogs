use chrono::prelude::*;

#[derive(Debug, Clone)]
pub struct LogEvent {
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub stream_name: Option<String>,
}

#[derive(Debug, Copy, Clone)]
pub struct LogEventsRequest {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

impl LogEventsRequest {
    pub fn start_time_value(&self) -> Option<i64> {
        self.start_time.map(|t| t.timestamp_millis())
    }
    pub fn end_time_value(&self) -> Option<i64> {
        self.end_time.map(|t| t.timestamp_millis())
    }
}

#[derive(Debug, Clone)]
pub struct LogEventsResponse {
    pub events: Vec<LogEvent>,
    pub next_token: Option<String>,
}
