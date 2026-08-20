// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Response-size budget for MCP tool results.
//!
//! MCP clients cap the size of a tool result (Claude Code, for instance, at
//! roughly 25k tokens). A server that returns more fails opaquely on the
//! client side, with no signal the model can act on. Tools should bound their
//! own output, but a single guard at the handler edge turns any residual
//! overrun into an actionable in-band error instead of a broken response.
//!
//! [`BudgetedHandler`] wraps any [`ServerHandler`] and post-processes only
//! `call_tool`; every other method is forwarded unchanged, so it is a
//! transparent wrapper for prompts, resources, completion and notifications.
//! Wrap once and pass it to every transport ([`crate::streamable_http_router`],
//! the SSE accept loop, and `serve` over stdio) so the guard applies uniformly.
//!
//! ```rust,ignore
//! use mcp_core::budget::{BudgetedHandler, DEFAULT_MAX_RESPONSE_CHARS};
//!
//! let handler = BudgetedHandler::new(MyServer::new(), DEFAULT_MAX_RESPONSE_CHARS)
//!     .with_hint("Use the tool's pagination parameters to fetch less at once.");
//! ```

use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{NotificationContext, RequestContext, RoleServer};
use rmcp::ErrorData as McpError;

/// A sensible default char budget: below the ~25k-token client cap with room
/// for the response envelope (prose runs a few characters per token).
/// Consumers may pass their own value; the number is policy, not framework.
pub const DEFAULT_MAX_RESPONSE_CHARS: usize = 50_000;

/// The default recovery hint, used when a consumer does not set one.
const DEFAULT_HINT: &str =
    "The response exceeds the size budget. Narrow the request — use the tool's \
     pagination or chunking parameters, or a more specific tool.";

/// Total length, in bytes, of the text content of a tool result.
fn response_chars(result: &CallToolResult) -> usize {
    result
        .content
        .iter()
        .map(|content| content.as_text().map(|t| t.text.len()).unwrap_or_default())
        .sum()
}

/// If `result`'s text content exceeds `max_chars`, replace it with an
/// `is_error` result carrying a machine-readable `response_exceeds_budget`
/// payload (the measured size, the budget, and a recovery `hint`); otherwise
/// return it unchanged.
pub fn enforce_response_budget(
    result: CallToolResult,
    max_chars: usize,
    hint: &str,
) -> CallToolResult {
    let chars = response_chars(&result);
    if chars <= max_chars {
        return result;
    }
    // Built by hand rather than via serde_json: the slim `stdio` and
    // `streamable-http` transports do not pull serde_json, and the payload is
    // a fixed shape (only `hint` is free text, so only it needs escaping).
    let payload = format!(
        r#"{{"error":"response_exceeds_budget","response_chars":{chars},"budget_chars":{max_chars},"hint":"{}"}}"#,
        json_escape(hint)
    );
    CallToolResult::error(vec![Content::text(payload)])
}

/// Escape a string for embedding in a JSON string literal.
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Wraps a [`ServerHandler`], enforcing a response-size budget on every
/// `call_tool` result. All other handler methods are forwarded unchanged.
#[derive(Debug, Clone)]
pub struct BudgetedHandler<H> {
    inner: H,
    max_chars: usize,
    hint: String,
}

impl<H> BudgetedHandler<H> {
    /// Wrap `inner`, capping tool results at `max_chars` bytes of text.
    pub fn new(inner: H, max_chars: usize) -> Self {
        Self {
            inner,
            max_chars,
            hint: DEFAULT_HINT.to_string(),
        }
    }

    /// Set the recovery hint returned when a result exceeds the budget
    /// (name the tool's own pagination parameters here).
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }

    /// The wrapped handler.
    pub fn inner(&self) -> &H {
        &self.inner
    }
}

impl<H: ServerHandler> ServerHandler for BudgetedHandler<H> {
    // --- the one method we post-process ------------------------------------
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let result = self.inner.call_tool(request, context).await?;
        Ok(enforce_response_budget(result, self.max_chars, &self.hint))
    }

    // --- everything else forwards verbatim ---------------------------------
    fn get_info(&self) -> ServerInfo {
        self.inner.get_info()
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.inner.get_tool(name)
    }

    async fn enqueue_task(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CreateTaskResult, McpError> {
        self.inner.enqueue_task(request, context).await
    }

    async fn ping(&self, context: RequestContext<RoleServer>) -> Result<(), McpError> {
        self.inner.ping(context).await
    }

    async fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        self.inner.initialize(request, context).await
    }

    async fn complete(
        &self,
        request: CompleteRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CompleteResult, McpError> {
        self.inner.complete(request, context).await
    }

    async fn set_level(
        &self,
        request: SetLevelRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.inner.set_level(request, context).await
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        self.inner.get_prompt(request, context).await
    }

    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        self.inner.list_prompts(request, context).await
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        self.inner.list_resources(request, context).await
    }

    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        self.inner.list_resource_templates(request, context).await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        self.inner.read_resource(request, context).await
    }

    async fn subscribe(
        &self,
        request: SubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.inner.subscribe(request, context).await
    }

    async fn unsubscribe(
        &self,
        request: UnsubscribeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        self.inner.unsubscribe(request, context).await
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        self.inner.list_tools(request, context).await
    }

    async fn on_custom_request(
        &self,
        request: CustomRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<CustomResult, McpError> {
        self.inner.on_custom_request(request, context).await
    }

    async fn on_cancelled(
        &self,
        notification: CancelledNotificationParam,
        context: NotificationContext<RoleServer>,
    ) {
        self.inner.on_cancelled(notification, context).await
    }

    async fn on_progress(
        &self,
        notification: ProgressNotificationParam,
        context: NotificationContext<RoleServer>,
    ) {
        self.inner.on_progress(notification, context).await
    }

    async fn on_initialized(&self, context: NotificationContext<RoleServer>) {
        self.inner.on_initialized(context).await
    }

    async fn on_roots_list_changed(&self, context: NotificationContext<RoleServer>) {
        self.inner.on_roots_list_changed(context).await
    }

    async fn on_custom_notification(
        &self,
        notification: CustomNotification,
        context: NotificationContext<RoleServer>,
    ) {
        self.inner
            .on_custom_notification(notification, context)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn under_budget_passes_through_unchanged() {
        let result = CallToolResult::success(vec![Content::text("small")]);
        let guarded = enforce_response_budget(result, 100, DEFAULT_HINT);
        assert_ne!(guarded.is_error, Some(true));
        assert_eq!(
            guarded.content[0].as_text().map(|t| t.text.as_str()),
            Some("small")
        );
    }

    #[test]
    fn over_budget_becomes_structured_error() {
        let big = "x".repeat(200);
        let result = CallToolResult::success(vec![Content::text(big)]);
        let guarded = enforce_response_budget(result, 50, "use pagination");

        assert_eq!(guarded.is_error, Some(true));
        let text = guarded.content[0]
            .as_text()
            .map(|t| t.text.as_str())
            .unwrap_or_default();
        let json: serde_json::Value = serde_json::from_str(text).expect("error payload is json");
        assert_eq!(json["error"], "response_exceeds_budget");
        assert_eq!(json["response_chars"], 200);
        assert_eq!(json["budget_chars"], 50);
        assert_eq!(json["hint"], "use pagination");
    }

    #[test]
    fn budget_sums_across_content_items() {
        let result = CallToolResult::success(vec![
            Content::text("x".repeat(30)),
            Content::text("y".repeat(30)),
        ]);
        // 60 chars total exceeds 50 even though no single item does.
        let guarded = enforce_response_budget(result, 50, DEFAULT_HINT);
        assert_eq!(guarded.is_error, Some(true));
    }
}
