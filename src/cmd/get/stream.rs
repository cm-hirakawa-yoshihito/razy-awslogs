use chrono::prelude::*;
use failure::{format_err, ResultExt};
use futures::prelude::*;
use futures::stream::{self, Stream};
use log::debug;
use rusoto_logs::{
    CloudWatchLogs, CloudWatchLogsClient, FilterLogEventsError, FilterLogEventsRequest,
    FilterLogEventsResponse, FilteredLogEvent, GetLogEventsError, GetLogEventsRequest,
    GetLogEventsResponse, OutputLogEvent,
};

use super::event::LogEvent;
use crate::errors;

#[derive(Debug)]
enum StreamState {
    Initial,
    Running(Option<String>),
    Complete,
}

#[derive(Debug, Copy, Clone)]
pub struct LogEventsReadRequest {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

impl LogEventsReadRequest {
    pub fn start_time_value(&self) -> Option<i64> {
        self.start_time.map(|t| t.timestamp())
    }
    pub fn end_time_value(&self) -> Option<i64> {
        self.end_time.map(|t| t.timestamp())
    }
}

#[derive(Debug, Clone)]
pub struct LogEventsReadResponse {
    pub events: Vec<LogEvent>,
    pub next_token: Option<String>,
}

fn from_epoch_millis(epoch_millis: i64) -> DateTime<Utc> {
    Utc.timestamp(
        epoch_millis / 1000,
        ((epoch_millis % 1000) * 1_000_000) as u32, // ミリ秒→ナノ秒に変換
    )
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

impl From<(GetLogEventsResponse, String)> for LogEventsReadResponse {
    fn from((res, stream_name): (GetLogEventsResponse, String)) -> Self {
        LogEventsReadResponse {
            events: res
                .events
                .map(|events| {
                    events
                        .into_iter()
                        .map(|event| LogEvent::from((event, stream_name.clone()))) // NOTE: クローンしないと怒られる、 クロージャ使ってるところでmoveかけてもダメっぽい、あとで調査したい
                        .collect()
                })
                .unwrap_or(Vec::new()),
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

pub fn get_log_events_stream(
    client: CloudWatchLogsClient,
    group_name: String,
    stream_name: String,
    request: LogEventsReadRequest,
) -> Box<Stream<Item = LogEventsReadResponse, Error = errors::Error> + Send> {
    Box::new(stream::unfold(StreamState::Initial, move |state| {
        let (has_next, next_token) = match state {
            StreamState::Initial => (true, None),
            StreamState::Running(token) => {
                debug!("token: {:?}", token);
                (token.is_some(), token)
            }
            StreamState::Complete => (false, None),
        };

        let current_token = next_token.clone().unwrap_or(String::new());
        if has_next {
            let get_request = GetLogEventsRequest {
                start_time: request.start_time_value(),
                end_time: request.end_time_value(),
                log_group_name: group_name.clone(),
                log_stream_name: stream_name.clone(),
                next_token,
                ..Default::default()
            };

            debug!("get-log-events: request={:?}", get_request);

            // NOTE: 引数はキャプチャ扱いになるっぽい？ move入れてもダメなので一旦クローンで逃げる
            let stream_name = stream_name.clone();
            let fut = client
                .get_log_events(get_request)
                .map(move |res| {
                    let next_token = res.next_forward_token.clone();
                    let next_state = match next_token.as_ref() {
                        Some(s) if s.as_str() == current_token.as_str() => StreamState::Complete,
                        Some(s) => StreamState::Running(Some(s.to_string())),
                        None => StreamState::Complete,
                    };
                    (
                        LogEventsReadResponse::from((res, stream_name.clone())),
                        next_state,
                    )
                })
                .map_err(|e| errors::Error::from(e));

            Some(fut)
        } else {
            None
        }
    }))
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

impl From<FilterLogEventsResponse> for LogEventsReadResponse {
    fn from(res: FilterLogEventsResponse) -> Self {
        let events = res
            .events
            .map(|events| events.into_iter().map(LogEvent::from).collect())
            .unwrap_or(Vec::new());
        LogEventsReadResponse {
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

pub fn filter_log_events_stream(
    client: CloudWatchLogsClient,
    group_name: String,
    stream_names: Option<Vec<String>>,
    filter_expression: String,
    request: LogEventsReadRequest,
) -> Box<Stream<Item = LogEventsReadResponse, Error = errors::Error> + Send> {
    Box::new(stream::unfold(StreamState::Initial, move |state| {
        let (has_next, next_token) = match state {
            StreamState::Initial => (true, None),
            StreamState::Running(token) => (token.is_some(), token),
            StreamState::Complete => (false, None),
        };

        let current_token = next_token.clone().unwrap_or(String::new());
        if has_next {
            let filter_request = FilterLogEventsRequest {
                log_group_name: group_name.clone(),
                log_stream_names: stream_names.clone(),
                start_time: request.start_time_value(),
                end_time: request.end_time_value(),
                filter_pattern: Some(filter_expression.clone()),
                next_token,
                ..Default::default()
            };

            let fut = client
                .filter_log_events(filter_request)
                .map(move |res| {
                    let next_token = res.next_token.clone();
                    let next_state = match next_token.as_ref() {
                        Some(s) if s.as_str() == current_token.as_str() => StreamState::Complete,
                        Some(s) => StreamState::Running(Some(s.to_string())),
                        None => StreamState::Complete,
                    };
                    (LogEventsReadResponse::from(res), next_state)
                })
                .map_err(|e| errors::Error::from(e));

            Some(fut)
        } else {
            None
        }
    }))
}
