use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Response;

use crate::error::AiError;
use crate::types::{GenerateTextRequest, GenerateTextResponse, TextStream};

pub mod anthropic;
pub mod google;
pub mod openai;
pub mod openai_compatible;

#[async_trait]
pub trait ModelAdapter: Send + Sync {
    async fn generate_text(
        &self,
        req: &GenerateTextRequest,
    ) -> Result<GenerateTextResponse, AiError>;

    async fn stream_text(&self, req: &GenerateTextRequest) -> Result<TextStream, AiError>;
}

pub type SharedAdapter = Arc<dyn ModelAdapter>;

pub(crate) async fn check_response_status(
    provider_id: &str,
    response: Response,
) -> Result<Response, AiError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let request_id = response
        .headers()
        .get("x-request-id")
        .or_else(|| response.headers().get("request-id"))
        .and_then(|v| v.to_str().ok())
        .map(ToOwned::to_owned);

    let body = response.text().await.ok();
    Err(crate::error::provider_http_error(
        provider_id,
        status.as_u16(),
        body,
        request_id,
    ))
}

pub(crate) fn map_send_error(provider_id: &str, err: reqwest::Error) -> AiError {
    crate::error::transport_error(provider_id, err)
}
