use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::settings::GatewayConfig;
use crate::tools::policy::PolicySettings;
use crate::tools::{SharedToolContext, ToolContext, Workspace};
use crate::workspace::{AuthConfig, WorkspaceProfile};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayWorkspaceInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub route: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewaySessionInfo {
    pub session_key: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub selected_at_unix_ms: u128,
}

struct GatewayWorkspace {
    info: GatewayWorkspaceInfo,
    context: SharedToolContext,
}

#[derive(Debug, Clone)]
struct SessionBinding {
    workspace_id: String,
    selected_at_unix_ms: u128,
}

pub struct GatewayState {
    workspaces: HashMap<String, GatewayWorkspace>,
    sessions: RwLock<HashMap<String, SessionBinding>>,
    pub tool_profile: String,
    pub auto_select_single_workspace: bool,
}

pub type SharedGatewayState = Arc<GatewayState>;

impl GatewayState {
    pub fn new(
        profiles: &[WorkspaceProfile],
        config: &GatewayConfig,
        gateway_auth: AuthConfig,
    ) -> Result<SharedGatewayState, String> {
        if profiles.is_empty() {
            return Err("Gateway 至少需要一个已注册工作区".into());
        }

        let mut workspaces = HashMap::new();
        for profile in profiles {
            let workspace = Workspace::new(PathBuf::from(&profile.path))
                .map_err(|error| format!("工作区“{}”不可用: {}", profile.name, error.message()))?;
            let policy = PolicySettings::from_runtime(&profile.runtime);
            let context = Arc::new(ToolContext::from_workspace(
                workspace,
                gateway_auth.clone(),
                policy,
                config.tool_profile.clone(),
                profile.runtime.permission_mode.clone(),
            ));
            let info = GatewayWorkspaceInfo {
                id: profile.id.clone(),
                name: profile.name.clone(),
                path: profile.path.clone(),
                route: format!("/w/{}/mcp", profile.id),
            };
            workspaces.insert(profile.id.clone(), GatewayWorkspace { info, context });
        }

        Ok(Arc::new(Self {
            workspaces,
            sessions: RwLock::new(HashMap::new()),
            tool_profile: crate::tools::registry::normalize_tool_profile(&config.tool_profile)
                .to_string(),
            auto_select_single_workspace: config.auto_select_single_workspace,
        }))
    }

    pub fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    pub fn session_count(&self) -> usize {
        self.sessions
            .read()
            .map(|sessions| sessions.len())
            .unwrap_or_default()
    }

    pub fn list_workspaces(&self) -> Vec<GatewayWorkspaceInfo> {
        let mut items: Vec<_> = self
            .workspaces
            .values()
            .map(|workspace| workspace.info.clone())
            .collect();
        items.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        items
    }

    pub fn clear_session(&self, session_key: &str) -> Result<bool, String> {
        let removed = self
            .sessions
            .write()
            .map_err(|_| "Gateway session registry poisoned".to_string())?
            .remove(session_key)
            .is_some();
        Ok(removed)
    }

    pub fn list_sessions(&self) -> Vec<GatewaySessionInfo> {
        let Ok(sessions) = self.sessions.read() else {
            return Vec::new();
        };
        let mut items: Vec<_> = sessions
            .iter()
            .filter_map(|(session_key, binding)| {
                let workspace = self.workspaces.get(&binding.workspace_id)?;
                Some(GatewaySessionInfo {
                    session_key: session_key.clone(),
                    workspace_id: binding.workspace_id.clone(),
                    workspace_name: workspace.info.name.clone(),
                    selected_at_unix_ms: binding.selected_at_unix_ms,
                })
            })
            .collect();
        items.sort_by(|left, right| right.selected_at_unix_ms.cmp(&left.selected_at_unix_ms));
        items
    }

    pub fn select_workspace(
        &self,
        session_key: &str,
        selector: &str,
    ) -> Result<GatewayWorkspaceInfo, String> {
        let workspace_id = self.resolve_workspace_id(selector)?;
        let workspace = self
            .workspaces
            .get(&workspace_id)
            .ok_or_else(|| format!("workspace not found: {workspace_id}"))?;
        let binding = SessionBinding {
            workspace_id,
            selected_at_unix_ms: now_unix_ms(),
        };
        self.sessions
            .write()
            .map_err(|_| "Gateway session registry poisoned".to_string())?
            .insert(session_key.to_string(), binding);
        Ok(workspace.info.clone())
    }

    pub fn current_workspace(
        &self,
        session_key: Option<&str>,
        forced_workspace: Option<&str>,
    ) -> Result<GatewayWorkspaceInfo, String> {
        let workspace_id = self.resolve_workspace_for_request(session_key, forced_workspace)?;
        Ok(self
            .workspaces
            .get(&workspace_id)
            .expect("resolved workspace must exist")
            .info
            .clone())
    }

    pub fn context_for_request(
        &self,
        session_key: Option<&str>,
        forced_workspace: Option<&str>,
    ) -> Result<SharedToolContext, String> {
        let workspace_id = self.resolve_workspace_for_request(session_key, forced_workspace)?;
        Ok(self
            .workspaces
            .get(&workspace_id)
            .expect("resolved workspace must exist")
            .context
            .clone())
    }

    fn resolve_workspace_for_request(
        &self,
        session_key: Option<&str>,
        forced_workspace: Option<&str>,
    ) -> Result<String, String> {
        if let Some(selector) = forced_workspace {
            return self.resolve_workspace_id(selector);
        }

        if let Some(session_key) = session_key.filter(|value| !value.trim().is_empty()) {
            if let Ok(sessions) = self.sessions.read() {
                if let Some(binding) = sessions.get(session_key) {
                    if self.workspaces.contains_key(&binding.workspace_id) {
                        return Ok(binding.workspace_id.clone());
                    }
                }
            }
        }

        if self.auto_select_single_workspace && self.workspaces.len() == 1 {
            return Ok(self
                .workspaces
                .keys()
                .next()
                .expect("single workspace exists")
                .clone());
        }

        Err("尚未为当前会话选择工作区；先调用 list_workspaces，再调用 select_workspace。".into())
    }

    fn resolve_workspace_id(&self, selector: &str) -> Result<String, String> {
        let selector = normalize_workspace_selector(selector);

        if self.workspaces.contains_key(&selector) {
            return Ok(selector);
        }

        let matches: Vec<_> = self
            .workspaces
            .values()
            .filter(|workspace| workspace.info.name.eq_ignore_ascii_case(&selector))
            .collect();
        match matches.as_slice() {
            [workspace] => Ok(workspace.info.id.clone()),
            [] => Err(format!("找不到工作区: {selector}")),
            _ => Err(format!("存在多个同名工作区“{selector}”，请使用 workspace id")),
        }
    }
}

fn normalize_workspace_selector(selector: &str) -> String {
    let mut value = selector.trim().trim_matches('/');
    if let Some(stripped) = value.strip_prefix("w/") {
        value = stripped;
    }
    if let Some(stripped) = value.strip_suffix("/mcp") {
        value = stripped;
    }
    value.trim_matches('/').to_string()
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_route_selector_extracts_workspace_id() {
        assert_eq!(normalize_workspace_selector("/w/abc123/mcp"), "abc123");
        assert_eq!(normalize_workspace_selector("w/abc123"), "abc123");
        assert_eq!(normalize_workspace_selector("abc123"), "abc123");
    }
}

