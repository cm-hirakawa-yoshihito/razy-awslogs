use chrono::prelude::*;
use failure::{format_err, ResultExt};
use futures::prelude::*;
use rusoto_logs::{
    CloudWatchLogs, CloudWatchLogsClient, FilterLogEventsError, FilterLogEventsRequest,
    FilterLogEventsResponse, FilteredLogEvent, GetLogEventsError, GetLogEventsRequest,
    GetLogEventsResponse, OutputLogEvent,
};

use super::event::{LogEvent, LogEventsRequest, LogEventsResponse};
use crate::errors;

////////////////////////////////////////////////////////////////////////////////
//
// Helper functions
//
////////////////////////////////////////////////////////////////////////////////

fn from_epoch_millis(epoch_millis: i64) -> DateTime<Utc> {
    Utc.timestamp(
        epoch_millis / 1000,
        ((epoch_millis % 1000) * 1_000_000) as u32, // ミリ秒→ナノ秒に変換
    )
}

////////////////////////////////////////////////////////////////////////////////
//
// LogEventResponseFuture
//
////////////////////////////////////////////////////////////////////////////////

pub type LogEventResponseFuture = Future<Item = LogEventsResponse, Error = errors::Error> + Send;

////////////////////////////////////////////////////////////////////////////////
//
// LogEventsReader
//
////////////////////////////////////////////////////////////////////////////////

pub trait LogEventsReader {
    fn read_log_events(&self, next_token: Option<String>) -> Box<LogEventResponseFuture>;
}

////////////////////////////////////////////////////////////////////////////////
//
// GetLogEventsReader
//
////////////////////////////////////////////////////////////////////////////////

pub struct GetLogEventsReader {
    pub client: CloudWatchLogsClient,
    pub group_name: String,
    pub stream_name: String,
    pub request: LogEventsRequest,
}

impl LogEventsReader for GetLogEventsReader {
    fn read_log_events(&self, next_token: Option<String>) -> Box<LogEventResponseFuture> {
        let get_request = GetLogEventsRequest {
            start_time: self.request.start_time_value(),
            end_time: self.request.end_time_value(),
            log_group_name: self.group_name.clone(),
            log_stream_name: self.stream_name.clone(),
            next_token,
            ..Default::default()
        };
        let stream_name = self.stream_name.clone();
        Box::new(
            self.client
                .get_log_events(get_request)
                .map(move |res| LogEventsResponse::from((res, stream_name)))
                .map_err(|e| errors::Error::from(e)),
        )
    }
}

impl From<(OutputLogEvent, String)> for LogEvent {
    fn from((event, stream_name): (OutputLogEvent, String)) -> Self {
        LogEvent {
            message: event.message.unwrap(),
            timestamp: from_epoch_millis(event.timestamp.unwrap()),
            stream_name: Some(stream_name),
        }
    }
}

impl From<(GetLogEventsResponse, String)> for LogEventsResponse {
    fn from((res, stream_name): (GetLogEventsResponse, String)) -> Self {
        let events = res
            .events
            .map(|events| {
                events
                    .into_iter()
                    .map(|event| LogEvent::from((event, stream_name.clone()))) // NOTE: クローンしないと怒られる、 クロージャ使ってるところでmoveかけてもダメっぽい、あとで調査したい
                    .collect()
            })
            .unwrap_or(Vec::new());
        LogEventsResponse {
            events,
            next_token: res.next_forward_token,
        }
    }
}

impl From<GetLogEventsError> for errors::Error {
    fn from(e: GetLogEventsError) -> Self {
        let context = match e {
            GetLogEventsError::Unknown(http_error) => {
                let body = String::from_utf8(http_error.body)
                    .expect("Cannot deserialize body of GetLogEventsError");
                Err::<(), failure::Error>(format_err!("{}", body))
                    .context(errors::ErrorKind::Rusoto)
                    .unwrap_err()
            }
            _ => Err::<(), GetLogEventsError>(e)
                .context(errors::ErrorKind::Rusoto)
                .unwrap_err(),
        };

        errors::Error::from(context)
    }
}

////////////////////////////////////////////////////////////////////////////////
//
// FilterLogEventsReader
//
////////////////////////////////////////////////////////////////////////////////

pub struct FilterLogEventsReader {
    pub client: CloudWatchLogsClient,
    pub group_name: String,
    pub stream_names: Option<Vec<String>>,
    pub filter_expression: String,
    pub request: LogEventsRequest,
}

impl LogEventsReader for FilterLogEventsReader {
    fn read_log_events(&self, next_token: Option<String>) -> Box<LogEventResponseFuture> {
        let filter_request = FilterLogEventsRequest {
            log_group_name: self.group_name.clone(),
            log_stream_names: self.stream_names.clone(),
            start_time: self.request.start_time_value(),
            end_time: self.request.end_time_value(),
            filter_pattern: Some(self.filter_expression.clone()),
            next_token,
            ..Default::default()
        };

        Box::new(
            self.client
                .filter_log_events(filter_request)
                .map(LogEventsResponse::from)
                .map_err(errors::Error::from),
        )
    }
}

impl From<FilteredLogEvent> for LogEvent {
    fn from(event: FilteredLogEvent) -> Self {
        LogEvent {
            message: event.message.unwrap(),
            timestamp: from_epoch_millis(event.timestamp.unwrap()),
            stream_name: event.log_stream_name,
        }
    }
}

impl From<FilterLogEventsResponse> for LogEventsResponse {
    fn from(res: FilterLogEventsResponse) -> Self {
        let events = res
            .events
            .map(|events| events.into_iter().map(LogEvent::from).collect())
            .unwrap_or(Vec::new());
        LogEventsResponse {
            events,
            next_token: res.next_token,
        }
    }
}

impl From<FilterLogEventsError> for errors::Error {
    fn from(e: FilterLogEventsError) -> Self {
        errors::Error::from(
            Err::<(), FilterLogEventsError>(e) // NOTE: Okの型を明示しないとダメっぽい
                .context(errors::ErrorKind::Rusoto)
                .unwrap_err(),
        )
    }
}
