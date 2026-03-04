use std::convert::Infallible;

use axum::response::sse::{Event, Sse};
use futures_util::StreamExt;
use serde_json::json;

use crate::types::{StreamEvent, TextStream};

pub fn stream_to_sse(
    stream: TextStream,
) -> Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>> {
    let mapped = stream.map(|item| {
        let event = match item {
            Ok(StreamEvent::TextDelta { text }) => Event::default()
                .event("token")
                .data(json!({ "text": text }).to_string()),
            Ok(StreamEvent::ToolCallReady { call }) => Event::default()
                .event("tool_call")
                .data(json!({ "call": call }).to_string()),
            Ok(StreamEvent::Usage { usage }) => Event::default()
                .event("usage")
                .data(json!({ "usage": usage }).to_string()),
            Ok(StreamEvent::Done) => Event::default().event("done").data("{}"),
            Err(err) => Event::default().event("error").data(
                json!({ "code": format!("{:?}", err.code), "message": err.message }).to_string(),
            ),
        };
        Ok::<Event, Infallible>(event)
    });

    Sse::new(mapped)
}
