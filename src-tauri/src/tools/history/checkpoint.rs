use serde_json::{json, Value};

use crate::tools::context::ToolContext;
use crate::tools::workspace::{tool_ok, WorkspaceError, WorkspaceResult};

use super::model::{CheckpointRecord, MemoryManifest, MemoryState};
use super::{
    history_error, host_session_key, markdown, now_timestamp, reject_ambiguous_history,
    required_checkpoint_argument, resolve_dir, session_not_bootstrapped, state, storage,
};

pub(super) fn checkpoint(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let target = CheckpointTarget::from_args(args)?;
    let history_dir = resolve_dir(ctx, args)?;
    if !history_dir.exists() {
        return Err(session_not_bootstrapped());
    }
    let _lock = storage::lock_directory(&history_dir)?;
    let document = load_target_document(ctx, &history_dir, &target)?;
    let outcome = update_record(args, &document)?;
    if outcome.updated {
        storage::write_markdown(
            &history_dir.join(format!("{}.md", document.number)),
            &outcome.content,
        )?;
    }
    let (manifest, current_state) = refresh_derived(ctx, &history_dir, document.number)?;
    Ok(checkpoint_result(
        &target,
        &document,
        &outcome,
        &manifest,
        &current_state,
    ))
}

struct CheckpointTarget {
    session_key: String,
    expected_path: String,
    host_mismatch: bool,
}

impl CheckpointTarget {
    fn from_args(args: &Value) -> WorkspaceResult<Self> {
        let session_key = required_checkpoint_argument(args, "session_key")?;
        let expected_path = required_checkpoint_argument(args, "expected_path")?;
        let host_mismatch = host_session_key(args)
            .map(|host| host != session_key.as_str())
            .unwrap_or(false);
        Ok(Self {
            session_key,
            expected_path,
            host_mismatch,
        })
    }
}

struct TargetDocument {
    number: u64,
    path: String,
    content: String,
}

fn load_target_document(
    ctx: &ToolContext,
    history_dir: &std::path::Path,
    target: &CheckpointTarget,
) -> WorkspaceResult<TargetDocument> {
    let report = storage::scan(&ctx.workspace, history_dir)?;
    reject_ambiguous_history(&report)?;
    let document = report
        .documents
        .iter()
        .find(|document| document.session_key.as_deref() == Some(target.session_key.as_str()))
        .ok_or_else(session_not_bootstrapped)?;
    if document.path != target.expected_path {
        return Err(history_error(
            "SESSION_TARGET_MISMATCH",
            "The checkpoint target does not match the session initialized by bootstrap.",
            "validation",
            false,
            json!({
                "expected_path": target.expected_path,
                "resolved_path": document.path,
                "session_key": target.session_key
            }),
        ));
    }
    Ok(TargetDocument {
        number: document.number,
        path: document.path.clone(),
        content: document.content.clone(),
    })
}

struct CheckpointOutcome {
    record: CheckpointRecord,
    updated: bool,
    duplicate_ignored: bool,
    user_input_captured: bool,
    redacted: bool,
    content: String,
}

fn update_record(args: &Value, document: &TargetDocument) -> WorkspaceResult<CheckpointOutcome> {
    let timestamp = now_timestamp();
    let mut record = markdown::checkpoint_from_args(args, &timestamp)
        .map_err(WorkspaceError::invalid_argument)?;
    let user_input_captured = !record.raw_user_input.trim().is_empty();
    let redacted = markdown::redact_record(&mut record);
    record.content_hash = markdown::checkpoint_fingerprint(&record);
    let existing = markdown::parse_checkpoint_records(&document.content);
    let same_turn = existing
        .iter()
        .filter(|existing| existing.turn_id == record.turn_id)
        .collect::<Vec<_>>();
    let duplicate_ignored = is_duplicate(&same_turn, &record);
    let content = if duplicate_ignored {
        document.content.clone()
    } else {
        apply_revision(&mut record, &same_turn, &document.content)
    };
    Ok(CheckpointOutcome {
        record,
        updated: !duplicate_ignored,
        duplicate_ignored,
        user_input_captured,
        redacted,
        content,
    })
}

fn is_duplicate(existing: &[&CheckpointRecord], record: &CheckpointRecord) -> bool {
    existing.iter().any(|existing| {
        let fingerprint = if existing.content_hash.is_empty() {
            markdown::checkpoint_fingerprint(existing)
        } else {
            existing.content_hash.clone()
        };
        fingerprint == record.content_hash
    })
}

fn apply_revision(
    record: &mut CheckpointRecord,
    existing: &[&CheckpointRecord],
    content: &str,
) -> String {
    let latest = existing.iter().max_by_key(|existing| existing.revision);
    record.revision = latest.map(|existing| existing.revision + 1).unwrap_or(1);
    record.supersedes =
        latest.map(|existing| format!("{} revision-{}", existing.turn_id, existing.revision));
    let content = markdown::with_updated_at(content, &record.timestamp);
    markdown::append_checkpoint_record(&content, record)
}

fn refresh_derived(
    ctx: &ToolContext,
    history_dir: &std::path::Path,
    current_number: u64,
) -> WorkspaceResult<(MemoryManifest, MemoryState)> {
    let report = storage::scan(&ctx.workspace, history_dir)?;
    let manifest = state::build_manifest(&report);
    let revision = storage::read_state(history_dir)
        .ok()
        .flatten()
        .map(|state| state.state_revision + 1)
        .unwrap_or(1);
    let current_state = state::build_state(
        &report,
        &manifest,
        Some(current_number),
        &now_timestamp(),
        revision,
    );
    storage::write_index(history_dir, &storage::rebuild_index(&report))?;
    storage::write_manifest(history_dir, &manifest)?;
    storage::write_state(history_dir, &current_state)?;
    Ok((manifest, current_state))
}

fn checkpoint_result(
    target: &CheckpointTarget,
    document: &TargetDocument,
    outcome: &CheckpointOutcome,
    manifest: &MemoryManifest,
    current_state: &MemoryState,
) -> Value {
    let warnings = checkpoint_warnings(outcome, target.host_mismatch);
    tool_ok(json!({
        "session_number": document.number,
        "path": document.path,
        "session_key": target.session_key,
        "expected_path": target.expected_path,
        "host_session_key_mismatch": target.host_mismatch,
        "turn_id": outcome.record.turn_id,
        "revision": outcome.record.revision,
        "supersedes": outcome.record.supersedes,
        "created": false,
        "updated": outcome.updated,
        "duplicate_ignored": outcome.duplicate_ignored,
        "user_input_captured": outcome.user_input_captured,
        "content_hash": storage::sha256(outcome.content.as_bytes()),
        "archive_revision": manifest.archive_revision,
        "state_revision": current_state.state_revision,
        "warnings": warnings
    }))
}

fn checkpoint_warnings(outcome: &CheckpointOutcome, host_mismatch: bool) -> Vec<String> {
    let mut warnings = Vec::new();
    if !outcome.user_input_captured {
        warnings.push(
            "未提供 raw_user_input；服务端无法读取未作为工具参数传入的本轮用户输入。".into(),
        );
    }
    if outcome.redacted {
        warnings.push("检测到疑似敏感信息，归档内容已脱敏。".into());
    }
    if host_mismatch {
        warnings.push(
            "宿主会话标识已变化；本次仍使用 bootstrap 返回的稳定目标，未切换历史文件。".into(),
        );
    }
    warnings
}
