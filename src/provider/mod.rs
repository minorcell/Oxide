use async_trait::async_trait;
#[cfg(any(feature = "openai", feature = "anthropic"))]
use reqwest::Response;

use crate::error::AiError;
#[cfg(any(feature = "openai", feature = "anthropic"))]
use crate::error::{provider_http_error, transport_error};
#[cfg(any(feature = "openai", feature = "anthropic"))]
use crate::types::ProviderKind;
use crate::types::{GenerateTextRequest, GenerateTextResponse, TextStream};

#[cfg(feature = "anthropic")]
pub mod anthropic;
#[cfg(feature = "openai")]
pub mod openai;

#[async_trait]
pub(crate) trait ProviderAdapter: Send + Sync {
    async fn generate_text(
        &self,
        req: &GenerateTextRequest,
    ) -> Result<GenerateTextResponse, AiError>;
    async fn stream_text(&self, req: &GenerateTextRequest) -> Result<TextStream, AiError>;
}

#[cfg(any(feature = "openai", feature = "anthropic"))]
pub(crate) async fn check_response_status(
    provider: ProviderKind,
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
    Err(provider_http_error(
        provider,
        status.as_u16(),
        body,
        request_id,
    ))
}

#[cfg(any(feature = "openai", feature = "anthropic"))]
pub(crate) fn map_send_error(provider: ProviderKind, err: reqwest::Error) -> AiError {
    transport_error(provider, err)
}
