mod bootstrap;
mod checkpoint;
mod markdown;
mod maintenance;
mod model;
mod response;
mod retrieval;
mod state;
mod storage;

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::tools::context::ToolContext;
use crate::tools::workspace::{WorkspaceError, WorkspaceResult};

pub fn bootstrap(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    bootstrap::bootstrap(ctx, args)
}

fn validate_session_key(
    value: &str,
    source: &'static str,
) -> WorkspaceResult<(String, &'static str)> {
    const MAX_SESSION_KEY_BYTES: usize = 4 * 1024;
    if value.len() <= MAX_SESSION_KEY_BYTES {
        return Ok((value.to_string(), source));
    }
    Err(history_error(
        "SESSION_ID_INVALID",
        "session_key exceeds the supported length limit.",
        "validation",
        false,
        json!({"maximum_bytes": MAX_SESSION_KEY_BYTES}),
    ))
}

pub fn checkpoint(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    checkpoint::checkpoint(ctx, args)
}

pub fn validate(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    maintenance::validate(ctx, args)
}

pub fn search(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    retrieval::search(ctx, args)
}

pub fn read(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    retrieval::read(ctx, args)
}

fn host_session_key(args: &Value) -> Option<&str> {
    args.get("_host_session_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required_checkpoint_argument(args: &Value, name: &str) -> WorkspaceResult<String> {
    let value = args
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            history_error(
                "CHECKPOINT_TARGET_REQUIRED",
                "Pass session_key and expected_path exactly as returned by history_session_bootstrap.",
                "validation",
                false,
                json!({"missing_argument": name}),
            )
        })?;
    if name == "session_key" {
        return validate_session_key(&value, "explicit_session_key")
            .map(|(session_key, _)| session_key);
    }
    Ok(value)
}

fn resolve_dir(ctx: &ToolContext, args: &Value) -> WorkspaceResult<std::path::PathBuf> {
    storage::resolve_history_dir(
        &ctx.workspace,
        args.get("workspace_root").and_then(Value::as_str),
        args.get("history_dir").and_then(Value::as_str),
    )
}

fn resolve_session_key(args: &Value) -> WorkspaceResult<(String, &'static str)> {
    if let Some(value) = args
        .get("session_key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return validate_session_key(value, "explicit_session_key");
    }
    if let Some(value) = host_session_key(args) {
        return validate_session_key(value, "platform_conversation_id");
    }
    Err(history_error(
        "SESSION_ID_UNAVAILABLE",
        "A stable ChatGPT session identifier is required.",
        "validation",
        false,
        json!({}),
    ))
}

fn reject_ambiguous_history(report: &model::ScanReport) -> WorkspaceResult<()> {
    if report.duplicate_session_keys.is_empty() {
        return Ok(());
    }
    Err(history_error(
        "HISTORY_INDEX_CONFLICT",
        "Multiple history files declare the same session_key.",
        "validation",
        false,
        json!({"duplicate_session_keys": report.duplicate_session_keys}),
    ))
}

fn session_not_bootstrapped() -> WorkspaceError {
    history_error(
        "SESSION_NOT_BOOTSTRAPPED",
        "The session_key has not been bootstrapped.",
        "not_found",
        false,
        json!({}),
    )
}

fn history_error(
    code: &'static str,
    message: &str,
    category: &'static str,
    retryable: bool,
    details: Value,
) -> WorkspaceError {
    WorkspaceError::ToolDetails {
        code,
        message: message.into(),
        category,
        retryable,
        details,
    }
}

fn history_dir_display(ctx: &ToolContext, path: &std::path::Path) -> String {
    crate::tools::workspace::relative_display(ctx.workspace.root(), path)
}

fn now_timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("unix:{seconds}")
}
