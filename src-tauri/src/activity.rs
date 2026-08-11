use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::gateway::GatewayWorkspaceInfo;

const MAX_TRACES: usize = 1_000;
const MAX_VALUE_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTrace {
    pub trace_id: String,
    pub rpc_id: String,
    pub method: String,
    pub tool: String,
    pub session_id: String,
    pub route: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub status: String,
    pub started_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub request: Value,
    pub response: Value,
    pub error: Value,
    pub operation_id: String,
    pub process_session_id: String,
    pub parent_trace_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityProcess {
    pub session_id: String,
    pub operation_id: String,
    pub trace_id: String,
    pub workspace_name: String,
    pub command: String,
    pub status: String,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub exit_code: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub kind: String,
    pub trace: Option<ActivityTrace>,
    pub process: Option<ActivityProcess>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityQuery {
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub tool: String,
    #[serde(default)]
    pub status: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySnapshot {
    pub traces: Vec<ActivityTrace>,
    pub active_processes: Vec<ActivityProcess>,
}

struct ActivityInner {
    traces: VecDeque<ActivityTrace>,
    processes: HashMap<String, ActivityProcess>,
    process_trace_by_session: HashMap<String, String>,
}

pub struct ActivityStore {
    inner: Mutex<ActivityInner>,
    events: broadcast::Sender<ActivityEvent>,
}

impl ActivityStore {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(256);
        Self {
            inner: Mutex::new(ActivityInner {
                traces: VecDeque::with_capacity(MAX_TRACES),
                processes: HashMap::new(),
                process_trace_by_session: HashMap::new(),
            }),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ActivityEvent> {
        self.events.subscribe()
    }

    pub fn begin_trace(
        &self,
        body: &Value,
        session_id: Option<&str>,
        route: &str,
        workspace: Option<&GatewayWorkspaceInfo>,
    ) -> String {
        let trace_id = format!("trace_{}", uuid::Uuid::new_v4().simple());
        let started_at_ms = now_ms();
        let method = body
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let tool = tool_name(body).to_string();
        let related_session = argument_session_id(body);

        let trace = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let parent_trace_id = related_session
                .and_then(|id| inner.process_trace_by_session.get(id))
                .cloned()
                .unwrap_or_default();
            let trace = ActivityTrace {
                trace_id: trace_id.clone(),
                rpc_id: body.get("id").map(Value::to_string).unwrap_or_default(),
                method,
                tool,
                session_id: session_id.unwrap_or_default().to_string(),
                route: route.to_string(),
                workspace_id: workspace.map(|w| w.id.clone()).unwrap_or_default(),
                workspace_name: workspace.map(|w| w.name.clone()).unwrap_or_default(),
                status: "running".into(),
                started_at_ms,
                finished_at_ms: None,
                duration_ms: None,
                request: limit_value(body),
                response: Value::Null,
                error: Value::Null,
                operation_id: String::new(),
                process_session_id: related_session.unwrap_or_default().to_string(),
                parent_trace_id,
            };
            inner.traces.push_back(trace.clone());
            while inner.traces.len() > MAX_TRACES {
                inner.traces.pop_front();
            }
            trace
        };
        self.emit("activity.started", Some(trace), None);
        trace_id
    }

    pub fn complete_trace(
        &self,
        trace_id: &str,
        body: &Value,
        response: &Value,
        workspace: Option<&GatewayWorkspaceInfo>,
    ) {
        let finished_at_ms = now_ms();
        let structured = structured_content(response);
        let failed = response.get("error").is_some()
            || structured
                .and_then(|value| value.get("ok"))
                .and_then(Value::as_bool)
                == Some(false)
            || structured
                .and_then(|value| value.get("command_ok"))
                .and_then(Value::as_bool)
                == Some(false);
        let related_session = argument_session_id(body).map(str::to_string);
        let mut emitted_process = None;

        let trace = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            let Some(index) = inner.traces.iter().position(|item| item.trace_id == trace_id) else {
                return;
            };

            let tool = inner.traces[index].tool.clone();
            let workspace_name = workspace
                .map(|item| item.name.clone())
                .unwrap_or_else(|| inner.traces[index].workspace_name.clone());

            if tool == "exec_command" {
                if let Some(data) = structured {
                    let process_status = data.get("status").and_then(Value::as_str).unwrap_or("");
                    if process_status == "running" {
                        if let Some(raw_session_id) = data.get("session_id").and_then(Value::as_str) {
                            let process = ActivityProcess {
                                session_id: raw_session_id.to_string(),
                                operation_id: data
                                    .get("operation_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string(),
                                trace_id: trace_id.to_string(),
                                workspace_name: workspace_name.clone(),
                                command: data
                                    .get("command")
                                    .and_then(Value::as_str)
                                    .unwrap_or_default()
                                    .to_string(),
                                status: "running".into(),
                                started_at_ms: inner.traces[index].started_at_ms,
                                updated_at_ms: finished_at_ms,
                                exit_code: None,
                            };
                            inner
                                .process_trace_by_session
                                .insert(raw_session_id.to_string(), trace_id.to_string());
                            inner.processes.insert(raw_session_id.to_string(), process.clone());
                            emitted_process = Some(process);
                        }
                    }
                }
            } else if let Some(raw_session_id) = related_session.as_deref() {
                if let Some(process) = inner.processes.get_mut(raw_session_id) {
                    if let Some(data) = structured {
                        if let Some(status) = process_status(data) {
                            process.status = status.to_string();
                        }
                        process.exit_code = data.get("exit_code").and_then(Value::as_i64);
                    }
                    process.updated_at_ms = finished_at_ms;
                    emitted_process = Some(process.clone());
                    if is_terminal_process_status(&process.status) {
                        inner.processes.remove(raw_session_id);
                        inner.process_trace_by_session.remove(raw_session_id);
                    }
                }
            }

            let trace = &mut inner.traces[index];
            trace.status = if failed { "failed" } else { "completed" }.into();
            trace.finished_at_ms = Some(finished_at_ms);
            trace.duration_ms = Some(finished_at_ms.saturating_sub(trace.started_at_ms));
            trace.response = limit_value(response);
            trace.error = response
                .get("error")
                .map(limit_value)
                .unwrap_or(Value::Null);
            if let Some(workspace) = workspace {
                trace.workspace_id = workspace.id.clone();
                trace.workspace_name = workspace.name.clone();
            }
            if let Some(data) = structured {
                trace.operation_id = data
                    .get("operation_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if trace.process_session_id.is_empty() {
                    trace.process_session_id = data
                        .get("session_id")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_default();
                }
            }
            trace.clone()
        };

        self.emit("activity.completed", Some(trace), emitted_process);
    }

    pub fn snapshot(&self, query: &ActivityQuery) -> ActivitySnapshot {
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let limit = query.limit.clamp(1, 500);
        let traces = inner
            .traces
            .iter()
            .rev()
            .filter(|trace| matches_query(trace, query))
            .take(limit)
            .cloned()
            .collect();
        let mut active_processes: Vec<_> = inner.processes.values().cloned().collect();
        active_processes.sort_by(|a, b| b.updated_at_ms.cmp(&a.updated_at_ms));
        ActivitySnapshot {
            traces,
            active_processes,
        }
    }

    pub fn get(&self, trace_id: &str) -> Option<ActivityTrace> {
        self.inner
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .traces
            .iter()
            .find(|item| item.trace_id == trace_id)
            .cloned()
    }

    fn emit(&self, kind: &str, trace: Option<ActivityTrace>, process: Option<ActivityProcess>) {
        let _ = self.events.send(ActivityEvent {
            kind: kind.to_string(),
            trace,
            process,
        });
    }
}

impl Default for ActivityStore {
    fn default() -> Self {
        Self::new()
    }
}

fn default_limit() -> usize {
    200
}

fn matches_query(trace: &ActivityTrace, query: &ActivityQuery) -> bool {
    contains_fold(&trace.workspace_name, &query.workspace)
        && contains_fold(&trace.session_id, &query.session)
        && contains_fold(&trace.tool, &query.tool)
        && contains_fold(&trace.status, &query.status)
}

fn contains_fold(value: &str, needle: &str) -> bool {
    needle.trim().is_empty() || value.to_ascii_lowercase().contains(&needle.trim().to_ascii_lowercase())
}

fn tool_name(body: &Value) -> &str {
    body.get("params")
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn argument_session_id(body: &Value) -> Option<&str> {
    body.get("params")
        .and_then(|params| params.get("arguments"))
        .and_then(|args| args.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn structured_content(response: &Value) -> Option<&Value> {
    response
        .get("result")
        .and_then(|result| result.get("structuredContent"))
}

fn process_status(data: &Value) -> Option<&str> {
    let status = data
        .get("termination_reason")
        .or_else(|| data.get("status"))
        .and_then(Value::as_str)?;
    Some(match status {
        "exited" => "exited",
        "killed" => "killed",
        "timeout" => "timeout",
        "spawn_failed" | "failed" | "error" => "failed",
        "running" => "running",
        other => other,
    })
}

fn is_terminal_process_status(status: &str) -> bool {
    matches!(status, "exited" | "failed" | "killed" | "timeout")
}

fn limit_value(value: &Value) -> Value {
    let encoded = serde_json::to_string(value).unwrap_or_default();
    if encoded.len() <= MAX_VALUE_BYTES {
        value.clone()
    } else {
        let preview: String = encoded.chars().take(MAX_VALUE_BYTES / 2).collect();
        json!({
            "truncated": true,
            "originalBytes": encoded.len(),
            "preview": preview
        })
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_ids_are_unique_and_original_payload_is_preserved() {
        let store = ActivityStore::new();
        let body = json!({
            "id": 0,
            "method": "tools/call",
            "params": {
                "name": "exec_command",
                "arguments": {
                    "password": "hidden",
                    "nested": {"api_key": "key"},
                    "cmd": "tool --token top-secret PASSWORD=another-secret --verbose"
                },
                "_meta": {"openai/session": "conversation-secret"}
            }
        });
        let first = store.begin_trace(&body, Some("conversation-secret"), "session", None);
        let second = store.begin_trace(&body, Some("conversation-secret"), "session", None);
        assert_ne!(first, second);
        let trace = store.get(&first).expect("trace");
        assert_eq!(trace.request["params"]["arguments"]["password"], "hidden");
        assert_eq!(trace.request["params"]["arguments"]["nested"]["api_key"], "key");
        assert_eq!(
            trace.request["params"]["arguments"]["cmd"],
            "tool --token top-secret PASSWORD=another-secret --verbose"
        );
        assert_eq!(trace.request["params"]["_meta"]["openai/session"], "conversation-secret");
        assert_eq!(trace.session_id, "conversation-secret");
    }

    #[test]
    fn retained_exec_process_is_separate_from_completed_trace() {
        let store = ActivityStore::new();
        let process_session = "process-session-long-123456789";
        let body = json!({
            "id": 0,
            "method": "tools/call",
            "params": {
                "name": "exec_command",
                "arguments": {"cmd": "runner --api-key raw-secret cargo test"}
            }
        });
        let trace_id = store.begin_trace(&body, Some("chat-session"), "session", None);
        let response = json!({
            "result": {"structuredContent": {
                "ok": true,
                "status": "running",
                "session_id": process_session,
                "operation_id": "operation-1",
                "command": "runner --api-key raw-secret cargo test"
            }}
        });
        store.complete_trace(&trace_id, &body, &response, None);
        let snapshot = store.snapshot(&ActivityQuery::default());
        assert_eq!(snapshot.traces[0].status, "completed");
        assert_eq!(snapshot.active_processes.len(), 1);
        assert_eq!(snapshot.active_processes[0].status, "running");
        assert_eq!(
            snapshot.active_processes[0].command,
            "runner --api-key raw-secret cargo test"
        );

        let follow_up = json!({
            "id": 0,
            "method": "tools/call",
            "params": {
                "name": "read_output",
                "arguments": {"session_id": process_session}
            }
        });
        let follow_up_trace =
            store.begin_trace(&follow_up, Some("chat-session"), "session", None);
        let follow_up_response = json!({
            "result": {"structuredContent": {
                "ok": true,
                "status": "exited",
                "termination_reason": "exited",
                "exit_code": 0,
                "session_id": process_session
            }}
        });
        store.complete_trace(&follow_up_trace, &follow_up, &follow_up_response, None);

        let snapshot = store.snapshot(&ActivityQuery::default());
        assert!(snapshot.active_processes.is_empty());
        let follow_up = store.get(&follow_up_trace).expect("follow-up trace");
        assert_eq!(follow_up.parent_trace_id, trace_id);
    }
}
