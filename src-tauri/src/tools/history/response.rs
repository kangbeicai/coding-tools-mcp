use std::fs;

use serde_json::{json, Value};

use super::storage;

const BOOTSTRAP_RESPONSE_BUDGET: usize = 64 * 1024;

pub(super) fn bound_bootstrap_result(result: &mut Value) {
    if encoded_len(result) <= BOOTSTRAP_RESPONSE_BUDGET {
        return;
    }
    reduce_state(result);
    if encoded_len(result) <= BOOTSTRAP_RESPONSE_BUDGET {
        return;
    }
    if let Some(object) = result.as_object_mut() {
        object.insert(
            "assistant_instructions".into(),
            Value::String(
                "Use history_session_search and history_session_read for exact archive context; checkpoint raw_user_input before final response."
                    .into(),
            ),
        );
        object.insert(
            "warnings".into(),
            json!(["Bootstrap state was reduced to preserve a bounded remote response."]),
        );
    }
}

pub(super) fn derived_file_warnings(history_dir: &std::path::Path) -> Vec<String> {
    let mut warnings = Vec::new();
    for (label, result) in derived_files(history_dir) {
        match result {
            Ok(true) => {}
            Ok(false) => warnings.push(format!("{label} 缺失，已根据 Markdown 档案重建。")),
            Err(_) => warnings.push(format!("{label} 损坏，已根据 Markdown 档案重建。")),
        }
    }
    let readme = history_dir.join("README.md");
    if readme.exists() {
        let _ = fs::read_to_string(readme);
    }
    warnings
}

fn encoded_len(value: &Value) -> usize {
    serde_json::to_vec(value).map(|bytes| bytes.len()).unwrap_or(0)
}

fn reduce_state(result: &mut Value) {
    if let Some(state) = result.get_mut("state").and_then(Value::as_object_mut) {
        state.insert("recent_changes".into(), json!([]));
        state.insert("open_items".into(), json!([]));
        state.insert("open_items_source".into(), Value::Null);
        state.insert("references".into(), json!([]));
        state.insert(
            "current_focus".into(),
            Value::String("当前状态已收紧；请用 history_session_search 定位档案。".into()),
        );
    }
    if let Some(object) = result.as_object_mut() {
        object.insert("state_truncated".into(), Value::Bool(true));
    }
}

fn derived_files(
    history_dir: &std::path::Path,
) -> [(&'static str, crate::tools::workspace::WorkspaceResult<bool>); 4] {
    [
        (
            "历史索引",
            storage::read_index(history_dir).map(|value| value.is_some()),
        ),
        (
            "memory/manifest.json",
            storage::read_manifest(history_dir).map(|value| value.is_some()),
        ),
        (
            "memory/state.json",
            storage::read_state(history_dir).map(|value| value.is_some()),
        ),
        (
            "memory/snapshot.json",
            storage::read_snapshot(history_dir).map(|value| value.is_some()),
        ),
    ]
}
