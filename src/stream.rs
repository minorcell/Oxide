#[derive(Debug, Clone)]
pub struct SseFrame {
    pub event: Option<String>,
    pub data: String,
}

pub fn parse_sse_lines(input: &str) -> Vec<SseFrame> {
    let mut buffer = input.replace("\r\n", "\n");
    if !buffer.ends_with("\n\n") {
        buffer.push_str("\n\n");
    }
    drain_sse_frames(&mut buffer)
}

pub(crate) fn drain_sse_frames(buffer: &mut String) -> Vec<SseFrame> {
    let mut out = Vec::new();
    loop {
        let Some(idx) = buffer.find("\n\n") else {
            break;
        };
        let raw = buffer[..idx].to_string();
        buffer.drain(..idx + 2);
        if let Some(frame) = parse_frame(&raw) {
            out.push(frame);
        }
    }
    out
}

fn parse_frame(raw: &str) -> Option<SseFrame> {
    let mut event = None;
    let mut data_lines = Vec::new();

    for line in raw.lines() {
        if line.starts_with(':') {
            continue;
        }

        if let Some(value) = line.strip_prefix("event:") {
            event = Some(value.trim().to_string());
            continue;
        }

        if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.trim_start().to_string());
        }
    }

    if data_lines.is_empty() {
        return None;
    }

    Some(SseFrame {
        event,
        data: data_lines.join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::parse_sse_lines;

    #[test]
    fn parses_multiple_frames() {
        let input = "event: one\ndata: {\"a\":1}\n\ndata: [DONE]\n\n";
        let frames = parse_sse_lines(input);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].event.as_deref(), Some("one"));
        assert_eq!(frames[1].data, "[DONE]");
    }
}
