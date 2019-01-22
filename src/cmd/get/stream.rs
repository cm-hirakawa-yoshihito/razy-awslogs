use futures::prelude::*;
use futures::stream::{self, Stream};

use super::event::LogEventsResponse;
use super::reader::LogEventsReader;
use crate::errors;

#[derive(Debug)]
enum StreamState {
    Initial,
    Running(Option<String>),
    Complete,
}

pub type LogEventResponseStream = Stream<Item = LogEventsResponse, Error = errors::Error> + Send;

pub fn create_log_events_stream(
    reader: Box<LogEventsReader + Send>,
) -> Box<LogEventResponseStream> {
    Box::new(stream::unfold(StreamState::Initial, move |state| {
        let (has_next, next_token) = match state {
            StreamState::Initial => (true, None),
            StreamState::Running(token) => (token.is_some(), token),
            StreamState::Complete => (false, None),
        };

        let current_token = next_token.clone().unwrap_or(String::new());
        if has_next {
            let fut = reader
                .read_log_events(next_token)
                .map(move |res| {
                    let next_token = res.next_token.clone();
                    let next_state = match next_token.as_ref() {
                        Some(s) if s.as_str() == current_token.as_str() => StreamState::Complete,
                        Some(s) => StreamState::Running(Some(s.to_string())),
                        None => StreamState::Complete,
                    };

                    (res, next_state)
                })
                .map_err(errors::Error::from);

            Some(fut)
        } else {
            None
        }
    }))
}
