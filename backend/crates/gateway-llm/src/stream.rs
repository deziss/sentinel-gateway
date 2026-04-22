use futures::{Stream, StreamExt};
use serde_json::Value;
use std::pin::Pin;
use std::task::{Context, Poll};
use crate::token_counter::TokenCounter;

/// Intercepts SSE chunks to count tokens in real-time.
pub struct LlmStreamCounter<S> {
    stream: S,
    total_tokens: u32,
    accumulated_text: String,
}

impl<S> LlmStreamCounter<S> {
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            total_tokens: 0,
            accumulated_text: String::new(),
        }
    }

    pub fn total_tokens(&self) -> u32 {
        self.total_tokens
    }
}

impl<S, E> Stream for LlmStreamCounter<S>
where
    S: Stream<Item = Result<bytes::Bytes, E>> + Unpin,
{
    type Item = Result<bytes::Bytes, E>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.stream.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                // Parse SSE chunk (OpenAI format)
                // data: {"choices": [{"delta": {"content": "..."}}]}
                let content = String::from_utf8_lossy(&bytes);
                for line in content.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            continue;
                        }
                        if let Ok(json) = serde_json::from_str::<Value>(data) {
                            if let Some(delta) = json.get("choices")
                                .and_then(|c| c.get(0))
                                .and_then(|c| c.get("delta"))
                                .and_then(|d| d.get("content"))
                                .and_then(|c| c.as_str()) 
                            {
                                self.accumulated_text.push_str(delta);
                                // Incrementally count (or wait until end for precision)
                                // Precision counting with cl100k_base usually requires full string
                                // but we can approximate or count chunks here.
                            }
                        }
                    }
                }
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(None) => {
                // Stream finished, finalize count
                self.total_tokens = TokenCounter::count_completion(&self.accumulated_text);
                Poll::Ready(None)
            }
            res => res,
        }
    }
}
