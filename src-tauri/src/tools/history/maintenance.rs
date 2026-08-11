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
    let statuses = DerivedStatuses::read(&history_dir);
    let report = storage::scan(&ctx.workspace, &history_dir)?;
    let warnings = duplicate_key_warnings(&report);
    let repaired = repair_derived_files(ctx, &history_dir, repair)?;
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
        "numbers": report.numbers,
        "missing_numbers": report.missing_numbers,
        "duplicate_session_keys": report.duplicate_session_keys,
        "invalid_files": report.invalid_files,
        "empty_files": report.empty_files,
        "latest_number": latest_number,
        "latest_path": latest_path,
        "archive_count": report.documents.len(),
        "total_archive_bytes": report.total_bytes(),
        "index_status": statuses.index,
        "manifest_status": statuses.manifest,
        "state_status": statuses.state,
        "repaired": repaired,
        "warnings": warnings
    })))
}

struct DerivedStatuses {
    index: &'static str,
    manifest: &'static str,
    state: &'static str,
}

impl DerivedStatuses {
    fn read(history_dir: &std::path::Path) -> Self {
        Self {
            index: derived_status(storage::read_index(history_dir)),
            manifest: derived_status(storage::read_manifest(history_dir)),
            state: derived_status(storage::read_state(history_dir)),
        }
    }
}

fn duplicate_key_warnings(report: &super::model::ScanReport) -> Vec<String> {
    if report.duplicate_session_keys.is_empty() {
        Vec::new()
    } else {
        vec!["存在重复 session_key，相关映射未写入索引。".into()]
    }
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
    let manifest = state::build_manifest(&report);
    let state_revision = storage::read_state(history_dir)
        .ok()
        .flatten()
        .map(|state| state.state_revision + 1)
        .unwrap_or(1);
    let state = state::build_state(
        &report,
        &manifest,
        report.latest_number(),
        &now_timestamp(),
        state_revision,
    );
    storage::write_index(history_dir, &storage::rebuild_index(&report))?;
    storage::write_manifest(history_dir, &manifest)?;
    storage::write_state(history_dir, &state)?;
    Ok(true)
}

fn derived_status<T>(result: WorkspaceResult<Option<T>>) -> &'static str {
    match result {
        Ok(Some(_)) => "valid",
        Ok(None) => "missing",
        Err(_) => "invalid",
    }
}
