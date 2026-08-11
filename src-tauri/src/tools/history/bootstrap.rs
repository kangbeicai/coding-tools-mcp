use serde_json::{json, Value};

use crate::tools::context::ToolContext;
use crate::tools::workspace::{tool_ok, WorkspaceResult};

use super::model::{InitialInputRecord, MemoryManifest, MemoryState, ScanReport};
use super::{
    history_dir_display, history_error, host_session_key, markdown, now_timestamp, reject_ambiguous_history,
    resolve_dir, resolve_session_key, response, state, storage,
};

pub(super) fn bootstrap(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let identity = BootstrapIdentity::from_args(args)?;
    let history_dir = resolve_dir(ctx, args)?;
    storage::ensure_directory(&history_dir)?;
    let _lock = storage::lock_directory(&history_dir)?;
    let report = checked_report(ctx, &history_dir)?;
    let mut warnings = bootstrap_warnings(&history_dir, &identity);
    let session = resolve_session(ctx, args, &history_dir, &report, &identity, &mut warnings)?;
    let refreshed = checked_report(ctx, &history_dir)?;
    let (manifest, current_state) = refresh_derived(&history_dir, &refreshed, session.number)?;
    let mut result = bootstrap_result(
        &identity,
        &session,
        &refreshed,
        manifest,
        current_state,
        warnings,
    );
    response::bound_bootstrap_result(&mut result);
    Ok(tool_ok(result))
}

struct BootstrapIdentity {
    session_key: String,
    source: &'static str,
    host_mismatch: bool,
}

impl BootstrapIdentity {
    fn from_args(args: &Value) -> WorkspaceResult<Self> {
        let (session_key, source) = resolve_session_key(args)?;
        let host_mismatch = host_session_key(args)
            .map(|host| host != session_key.as_str())
            .unwrap_or(false);
        Ok(Self {
            session_key,
            source,
            host_mismatch,
        })
    }
}

struct BootstrapSession {
    number: u64,
    path: String,
    created: bool,
    resumed: bool,
    initial_input_captured: bool,
}

fn checked_report(ctx: &ToolContext, history_dir: &std::path::Path) -> WorkspaceResult<ScanReport> {
    let report = storage::scan(&ctx.workspace, history_dir)?;
    reject_ambiguous_history(&report)?;
    if report.missing_numbers.is_empty() {
        return Ok(report);
    }
    Err(history_error(
        "HISTORY_SEQUENCE_CONFLICT",
        "History numbering contains gaps; run history_session_validate before creating a session.",
        "validation",
        true,
        json!({"missing_numbers": report.missing_numbers}),
    ))
}

fn bootstrap_warnings(
    history_dir: &std::path::Path,
    identity: &BootstrapIdentity,
) -> Vec<String> {
    let mut warnings = response::derived_file_warnings(history_dir);
    if identity.host_mismatch {
        warnings.push(
            "宿主会话标识与显式 session_key 不一致，已使用显式 session_key 保持会话连续。"
                .into(),
        );
    }
    warnings
}

fn resolve_session(
    ctx: &ToolContext,
    args: &Value,
    history_dir: &std::path::Path,
    report: &ScanReport,
    identity: &BootstrapIdentity,
    warnings: &mut Vec<String>,
) -> WorkspaceResult<BootstrapSession> {
    let requested = requested_initial_input(args);
    if let Some(document) = report.documents.iter().find(|document| {
        document.session_key.as_deref() == Some(identity.session_key.as_str())
    }) {
        return resume_session(history_dir, document, requested, warnings);
    }
    create_session(ctx, args, history_dir, report, identity, requested, warnings)
}

fn requested_initial_input(args: &Value) -> Option<String> {
    args.get("initial_user_input")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn resume_session(
    history_dir: &std::path::Path,
    document: &super::model::HistoryDocument,
    requested: Option<String>,
    warnings: &mut Vec<String>,
) -> WorkspaceResult<BootstrapSession> {
    let records = markdown::parse_initial_input_records(&document.content);
    let previously_captured = !records.is_empty();
    let supplied = requested.is_some();
    if let Some(input) = requested {
        append_initial_revision(history_dir, document, &records, input, warnings)?;
    } else if !previously_captured {
        warnings.push(missing_initial_input_warning());
    }
    Ok(BootstrapSession {
        number: document.number,
        path: document.path.clone(),
        created: false,
        resumed: true,
        initial_input_captured: previously_captured || supplied,
    })
}

fn append_initial_revision(
    history_dir: &std::path::Path,
    document: &super::model::HistoryDocument,
    records: &[InitialInputRecord],
    mut input: String,
    warnings: &mut Vec<String>,
) -> WorkspaceResult<()> {
    redact_initial_input(&mut input, warnings);
    let content_hash = markdown::initial_input_fingerprint(&input);
    if records.iter().any(|record| record.content_hash == content_hash) {
        return Ok(());
    }
    let latest = records.iter().max_by_key(|record| record.revision);
    let record = InitialInputRecord {
        raw_user_input: input,
        captured_at: now_timestamp(),
        revision: latest.map(|record| record.revision + 1).unwrap_or(1),
        supersedes: latest.map(|record| format!("initial-input revision-{}", record.revision)),
        content_hash,
    };
    let content = markdown::with_updated_at(&document.content, &record.captured_at);
    storage::write_markdown(
        &history_dir.join(format!("{}.md", document.number)),
        &markdown::append_initial_input_revision(&content, &record),
    )
}

fn create_session(
    ctx: &ToolContext,
    args: &Value,
    history_dir: &std::path::Path,
    report: &ScanReport,
    identity: &BootstrapIdentity,
    requested: Option<String>,
    warnings: &mut Vec<String>,
) -> WorkspaceResult<BootstrapSession> {
    ensure_creation_allowed(args, identity.source)?;
    let number = report.latest_number().unwrap_or(0) + 1;
    let timestamp = now_timestamp();
    let initial = requested.map(|mut input| {
        redact_initial_input(&mut input, warnings);
        InitialInputRecord {
            content_hash: markdown::initial_input_fingerprint(&input),
            raw_user_input: input,
            captured_at: timestamp.clone(),
            revision: 1,
            supersedes: None,
        }
    });
    if initial.is_none() {
        warnings.push(missing_initial_input_warning());
    }
    write_new_document(args, history_dir, number, &timestamp, identity, initial.as_ref())?;
    Ok(BootstrapSession {
        number,
        path: format!("{}/{number}.md", history_dir_display(ctx, history_dir)),
        created: true,
        resumed: false,
        initial_input_captured: initial.is_some(),
    })
}

fn ensure_creation_allowed(args: &Value, source: &'static str) -> WorkspaceResult<()> {
    if args
        .get("create_if_missing")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return Ok(());
    }
    Err(history_error(
        "SESSION_NOT_BOOTSTRAPPED",
        "No history mapping exists for this session_key.",
        "not_found",
        false,
        json!({"session_key_source": source}),
    ))
}

fn write_new_document(
    args: &Value,
    history_dir: &std::path::Path,
    number: u64,
    timestamp: &str,
    identity: &BootstrapIdentity,
    initial: Option<&InitialInputRecord>,
) -> WorkspaceResult<()> {
    let title = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("开发会话");
    let content = markdown::render_document(
        number,
        title,
        &identity.session_key,
        timestamp,
        initial,
    );
    storage::write_markdown(&history_dir.join(format!("{number}.md")), &content)
}

fn redact_initial_input(input: &mut String, warnings: &mut Vec<String>) {
    if markdown::redact_text(input) {
        warnings.push("首次用户输入含疑似敏感信息，归档内容已脱敏。".into());
    }
}

fn missing_initial_input_warning() -> String {
    "未提供 initial_user_input；服务端无法读取未作为工具参数传入的首次用户输入。".into()
}

fn refresh_derived(
    history_dir: &std::path::Path,
    report: &ScanReport,
    current_number: u64,
) -> WorkspaceResult<(MemoryManifest, MemoryState)> {
    let manifest = state::build_manifest(report);
    let revision = storage::read_state(history_dir)
        .ok()
        .flatten()
        .map(|state| state.state_revision + 1)
        .unwrap_or(1);
    let current_state = state::build_state(
        report,
        &manifest,
        Some(current_number),
        &now_timestamp(),
        revision,
    );
    storage::write_index(history_dir, &storage::rebuild_index(report))?;
    storage::write_manifest(history_dir, &manifest)?;
    storage::write_state(history_dir, &current_state)?;
    Ok((manifest, current_state))
}

fn bootstrap_result(
    identity: &BootstrapIdentity,
    session: &BootstrapSession,
    report: &ScanReport,
    manifest: MemoryManifest,
    current_state: MemoryState,
    warnings: Vec<String>,
) -> Value {
    json!({
        "is_new_session": session.created,
        "session_key": identity.session_key,
        "session_key_source": identity.source,
        "host_session_key_mismatch": identity.host_mismatch,
        "current_number": session.number,
        "current_path": session.path,
        "created": session.created,
        "resumed": session.resumed,
        "initial_input_captured": session.initial_input_captured,
        "sequence_valid": report.sequence_valid(),
        "history_count": report.documents.len(),
        "total_history_bytes": report.total_bytes(),
        "state_revision": current_state.state_revision,
        "archive_revision": manifest.archive_revision,
        "state": current_state,
        "history_read_mode": "bounded_state_with_on_demand_search_and_read",
        "persistence_mode": "model_mediated_tool_calls",
        "assistant_instructions": "Use the bounded state to begin work. To recover exact earlier context, call history_session_search and then history_session_read for only the relevant archive. Preserve session_key and current_path. Before the final response for each user task, call history_session_checkpoint with the user's verbatim raw_user_input. The server can only save text passed as tool arguments and reports missing input explicitly.",
        "required_next_actions": [
            "review_bounded_state",
            "search_or_read_relevant_archives_when_precision_is_needed",
            "verify_workspace_state",
            "execute_user_task",
            "checkpoint_with_raw_user_input_before_final_response"
        ],
        "checkpoint_policy": {
            "tool": "history_session_checkpoint",
            "session_key": identity.session_key,
            "expected_path": session.path,
            "raw_user_input_required_for_full_fidelity": true,
            "required_before_final_response": true,
            "automatic_background_persistence": false
        },
        "search_guide": {
            "tool": "history_session_search",
            "then_read_with": "history_session_read",
            "archive_is_lossless": true
        },
        "warnings": warnings
    })
}
