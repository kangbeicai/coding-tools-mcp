use serde_json::{json, Value};

use crate::tools::context::ToolContext;
use crate::tools::workspace::{tool_ok, WorkspaceResult};

use super::{now_timestamp, resolve_dir, state, storage};

pub(super) fn validate(ctx: &ToolContext, args: &Value) -> WorkspaceResult<Value> {
    let history_dir = resolve_dir(ctx, args)?;
    let repair = args.get("repair").and_then(Value::as_bool).unwrap_or(false);
    if repair {
        storage::ensure_directory(&history_dir)?;
    }
    let report = storage::scan(&ctx.workspace, &history_dir)?;
    let warnings = validation_warnings(&report);
    let repaired = repair_derived_files(ctx, &history_dir, repair)?;
    let statuses = DerivedStatuses::read(&history_dir, &report);
    let latest_number = report.latest_number();
    let latest_path = latest_number.and_then(|number| {
        report
            .documents
            .iter()
            .find(|document| document.number == number)
            .map(|document| document.path.clone())
    });

    Ok(tool_ok(json!({
        "sequence_valid": report.sequence_valid(),
        "archive_integrity_valid": report.archive_integrity_valid(),
        "numbers": report.numbers,
        "missing_numbers": report.missing_numbers,
        "duplicate_session_keys": report.duplicate_session_keys,
        "invalid_files": report.invalid_files,
        "empty_files": report.empty_files,
        "malformed_blocks": report.malformed_blocks,
        "latest_number": latest_number,
        "latest_path": latest_path,
        "archive_count": report.documents.len(),
        "total_archive_bytes": report.total_bytes(),
        "index_status": statuses.index,
        "manifest_status": statuses.manifest,
        "state_status": statuses.state,
        "snapshot_status": statuses.snapshot,
        "index_fresh": statuses.index_fresh,
        "manifest_fresh": statuses.manifest_fresh,
        "state_fresh": statuses.state_fresh,
        "snapshot_fresh": statuses.snapshot_fresh,
        "derived_snapshot_status": statuses.snapshot_consistency,
        "repaired": repaired,
        "warnings": warnings
    })))
}

struct DerivedStatuses {
    index: &'static str,
    manifest: &'static str,
    state: &'static str,
    snapshot: &'static str,
    index_fresh: bool,
    manifest_fresh: bool,
    state_fresh: bool,
    snapshot_fresh: bool,
    snapshot_consistency: &'static str,
}

impl DerivedStatuses {
    fn read(history_dir: &std::path::Path, report: &super::model::ScanReport) -> Self {
        let expected_index = storage::rebuild_index(report);
        let expected_manifest = state::build_manifest(report);
        let index = storage::read_index(history_dir);
        let manifest = storage::read_manifest(history_dir);
        let current_state = storage::read_state(history_dir);
        let snapshot = storage::read_snapshot(history_dir);

        let index_status = derived_status(&index);
        let manifest_status = derived_status(&manifest);
        let state_status = derived_status(&current_state);
        let snapshot_status = derived_status(&snapshot);
        let index_fresh = matches!(&index, Ok(Some(value)) if value == &expected_index);
        let manifest_fresh = matches!(
            &manifest,
            Ok(Some(value)) if value.archive_revision == expected_manifest.archive_revision
        );
        let state_fresh = matches!(
            &current_state,
            Ok(Some(value))
                if value.version >= 3
                    && value.archive_revision == expected_manifest.archive_revision
        );
        let snapshot_fresh = match (&snapshot, &current_state) {
            (Ok(Some(snapshot)), Ok(Some(current_state))) => {
                snapshot.version == 1
                    && snapshot.archive_revision == expected_manifest.archive_revision
                    && snapshot.state_revision == current_state.state_revision
                    && state_fresh
            }
            _ => false,
        };
        let snapshot_consistency = if [index_status, manifest_status, state_status, snapshot_status]
            .contains(&"invalid")
        {
            "invalid"
        } else if [index_status, manifest_status, state_status, snapshot_status]
            .contains(&"missing")
        {
            "incomplete"
        } else if index_fresh && manifest_fresh && state_fresh && snapshot_fresh {
            "consistent"
        } else {
            "stale"
        };

        Self {
            index: index_status,
            manifest: manifest_status,
            state: state_status,
            snapshot: snapshot_status,
            index_fresh,
            manifest_fresh,
            state_fresh,
            snapshot_fresh,
            snapshot_consistency,
        }
    }
}

fn validation_warnings(report: &super::model::ScanReport) -> Vec<String> {
    let mut warnings = Vec::new();
    if !report.duplicate_session_keys.is_empty() {
        warnings.push("存在重复 session_key，相关映射未写入索引。".into());
    }
    if !report.malformed_blocks.is_empty() {
        warnings.push(
            "历史档案包含无法解析的结构化 JSON block；有效记录仍保留，但派生状态可能不完整。"
                .into(),
        );
    }
    warnings
}

fn repair_derived_files(
    ctx: &ToolContext,
    history_dir: &std::path::Path,
    repair: bool,
) -> WorkspaceResult<bool> {
    if !repair {
        return Ok(false);
    }
    let _lock = storage::lock_directory(history_dir)?;
    let report = storage::scan(&ctx.workspace, history_dir)?;
    state::refresh_derived(
        history_dir,
        &report,
        report.latest_number(),
        &now_timestamp(),
    )?;
    Ok(true)
}

fn derived_status<T>(result: &WorkspaceResult<Option<T>>) -> &'static str {
    match result {
        Ok(Some(_)) => "valid",
        Ok(None) => "missing",
        Err(_) => "invalid",
    }
}
