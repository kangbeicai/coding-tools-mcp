use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::gateway::{gateway_exposure_status, gateway_status, normalize_public_origin};

const TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayHealthItem {
    pub key: String,
    pub layer: String,
    pub label: String,
    /// `ok`, `warn`, `fail`, or `skip`.
    pub status: String,
    pub detail: String,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayHealthReport {
    pub chatgpt_ready: bool,
    pub summary: String,
    pub public_base_url: String,
    pub items: Vec<GatewayHealthItem>,
}

pub async fn run_gateway_health_checks(state: &AppState) -> AppResult<GatewayHealthReport> {
    let (settings, workspace_count, oauth_client_id, oauth_password, oauth_token_secret, bearer) =
        state.with_data(|store| {
            Ok((
                store.settings(),
                store.list().len(),
                store.get_shared_secret("oauth_client_id"),
                store.get_shared_secret("oauth_password"),
                store.get_shared_secret("oauth_token_secret"),
                store.get_shared_secret("bearer_token"),
            ))
        })?;
    let gateway = settings.gateway;
    let exposure_config = settings.gateway_exposure;
    let gateway_status = gateway_status(state)?;
    let exposure_status = gateway_exposure_status(state)?;
    let canonical = normalize_public_origin(&gateway.public_url)?;
    let mode = exposure_config.mode.trim().to_ascii_lowercase();
    let effective = if !exposure_status.effective_public_url.trim().is_empty() {
        exposure_status
            .effective_public_url
            .trim_end_matches('/')
            .to_string()
    } else {
        canonical.clone()
    };
    let connector_base = if canonical.is_empty() {
        effective.clone()
    } else {
        canonical.clone()
    };

    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|error| AppError::Message(format!("创建健康检查 HTTP 客户端失败: {error}")))?;

    let mut items = Vec::new();

    items.push(if workspace_count > 0 {
        ok_item(
            "workspace_registry",
            "config",
            "Workspace 注册",
            format!("已注册 {workspace_count} 个工作区"),
        )
    } else {
        fail_item(
            "workspace_registry",
            "config",
            "Workspace 注册",
            "当前没有已注册工作区".into(),
            "先在 Web Console 添加至少一个 Workspace。",
        )
    });

    // User intent is expressed by the canonical public URL itself.  Passive
    // network topology (local/direct/external) must not be another required
    // user decision: if a URL is configured, validate it. Managed providers
    // remain special because Coding Tools owns their child process lifecycle.
    let managed_provider = matches!(mode.as_str(), "frp" | "cloudflare");
    let public_required = !canonical.is_empty() || managed_provider;
    let canonical_https = canonical.to_ascii_lowercase().starts_with("https://");
    let quick_without_canonical = mode == "cloudflare"
        && exposure_config
            .cloudflare_mode
            .eq_ignore_ascii_case("quick")
        && canonical.is_empty();
    items.push(if !public_required {
        skip_item(
            "public_identity",
            "config",
            "Canonical 公网身份",
            "Local-only 模式不要求公网 URL".into(),
        )
    } else if canonical_https {
        ok_item(
            "public_identity",
            "config",
            "Canonical 公网身份",
            canonical.clone(),
        )
    } else if quick_without_canonical && effective.to_ascii_lowercase().starts_with("https://") {
        warn_item(
            "public_identity",
            "config",
            "Canonical 公网身份",
            format!("Quick Tunnel 临时身份: {effective}"),
            "Quick URL 可以临时使用，但长期 ChatGPT 插件建议配置固定 HTTPS canonical URL。",
        )
    } else if canonical.is_empty() {
        fail_item(
            "public_identity",
            "config",
            "Canonical 公网身份",
            "未配置公网 URL".into(),
            "配置固定的 https:// 公网 origin；不要附加 /mcp 以外的路径。",
        )
    } else {
        fail_item(
            "public_identity",
            "config",
            "Canonical 公网身份",
            canonical.clone(),
            "ChatGPT 公网 MCP 应使用 HTTPS canonical URL。",
        )
    });

    let auth_type = gateway.auth_type.trim().to_ascii_lowercase();
    let auth_ok = match auth_type.as_str() {
        "oauth" => {
            let missing = [
                ("oauth_client_id", oauth_client_id.as_deref()),
                ("oauth_password", oauth_password.as_deref()),
                ("oauth_token_secret", oauth_token_secret.as_deref()),
            ]
            .into_iter()
            .filter_map(|(key, value)| {
                value
                    .filter(|v| !v.trim().is_empty())
                    .is_none()
                    .then_some(key)
            })
            .collect::<Vec<_>>();
            if missing.is_empty() {
                items.push(ok_item(
                    "auth_config",
                    "config",
                    "认证配置",
                    "OAuth shared credentials 已就绪".into(),
                ));
                true
            } else {
                items.push(fail_item(
                    "auth_config",
                    "config",
                    "认证配置",
                    format!("缺少: {}", missing.join(", ")),
                    "在共享密钥页面补齐 Gateway OAuth 凭据。",
                ));
                false
            }
        }
        "bearer" => {
            let ok = bearer
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty());
            items.push(if ok {
                ok_item(
                    "auth_config",
                    "config",
                    "认证配置",
                    "Bearer Token 已配置".into(),
                )
            } else {
                fail_item(
                    "auth_config",
                    "config",
                    "认证配置",
                    "Bearer Token 为空".into(),
                    "在共享密钥页面设置 bearer_token。",
                )
            });
            ok
        }
        "noauth" => {
            items.push(warn_item(
                "auth_config",
                "config",
                "认证配置",
                "noauth：技术上可连接，但公网部署风险高".into(),
                "公网 Gateway 推荐使用 OAuth；至少使用 Bearer Token。",
            ));
            true
        }
        other => {
            items.push(fail_item(
                "auth_config",
                "config",
                "认证配置",
                format!("未知认证模式: {other}"),
                "仅使用 oauth、bearer 或 noauth。",
            ));
            false
        }
    };

    let runtime_ok = gateway_status.state == "running";
    items.push(if runtime_ok {
        ok_item(
            "gateway_runtime",
            "local",
            "Gateway Runtime",
            "Gateway process is running in the owning service".into(),
        )
    } else {
        fail_item(
            "gateway_runtime",
            "local",
            "Gateway Runtime",
            format!("state={}", gateway_status.state),
            "通过 Web Admin/systemd 启动 Gateway；如果这里只是 CLI 离线检查，优先确认 Admin 是否正在运行。",
        )
    });

    let (local_ok, local_detail) = probe_mcp(&client, &gateway_status.local_endpoint).await;
    items.push(if local_ok {
        ok_item(
            "local_gateway",
            "local",
            "Local Gateway",
            format!("{} · {local_detail}", gateway_status.local_endpoint),
        )
    } else {
        fail_item(
            "local_gateway",
            "local",
            "Local Gateway",
            format!("{} · {local_detail}", gateway_status.local_endpoint),
            "确认 Gateway 已启动，监听地址/端口未被占用。",
        )
    });

    let (provider_ok, provider_required) = match mode.as_str() {
        "local" | "direct" | "external" => {
            if canonical.is_empty() {
                items.push(skip_item(
                    "public_provider",
                    "provider",
                    "公网传输",
                    "未配置公网 URL；按本地模式运行".into(),
                ));
                (true, false)
            } else {
                items.push(ok_item(
                    "public_provider",
                    "provider",
                    "公网传输",
                    "公网 URL 由用户提供；无需声明 Direct / External 实现方式".into(),
                ));
                (true, true)
            }
        }
        "frp" | "cloudflare" => {
            let ok = exposure_status.state == "running";
            items.push(if ok {
                ok_item(
                    "public_provider",
                    "provider",
                    "Public Access Provider",
                    format!(
                        "{} running{}",
                        mode,
                        exposure_status
                            .pid
                            .map(|pid| format!(" · pid={pid}"))
                            .unwrap_or_default()
                    ),
                )
            } else {
                fail_item(
                    "public_provider",
                    "provider",
                    "Public Access Provider",
                    format!("{} · {}", mode, exposure_status.message),
                    "启动 managed exposure，并检查 provider 日志。",
                )
            });
            (ok, true)
        }
        other => {
            items.push(fail_item(
                "public_provider",
                "provider",
                "Public Access Provider",
                format!("未知模式: {other}"),
                "修正 Public Access mode。",
            ));
            (false, true)
        }
    };

    let transport_differs = provider_required
        && !effective.is_empty()
        && !connector_base.is_empty()
        && effective != connector_base;
    let transport_ok = if transport_differs {
        let transport_url = endpoint(&effective, "/mcp");
        let (ok, detail) = probe_mcp(&client, &transport_url).await;
        items.push(if ok {
            ok_item(
                "transport_mcp",
                "provider",
                "Managed Transport MCP",
                format!("{transport_url} · {detail}"),
            )
        } else {
            fail_item(
                "transport_mcp",
                "provider",
                "Managed Transport MCP",
                format!("{transport_url} · {detail}"),
                "Managed provider 已启动但其实际 tunnel URL 不可达；检查 provider 日志和出站网络。",
            )
        });
        ok
    } else {
        true
    };

    let public_https = connector_base.to_ascii_lowercase().starts_with("https://");
    let public_mcp_url = endpoint(&connector_base, "/mcp");
    let (public_ok, public_detail) = if !provider_required || connector_base.is_empty() {
        (false, String::new())
    } else {
        probe_mcp(&client, &public_mcp_url).await
    };
    items.push(if !provider_required {
        skip_item(
            "public_mcp",
            "public",
            "Public MCP",
            "Local-only 模式".into(),
        )
    } else if connector_base.is_empty() {
        fail_item(
            "public_mcp",
            "public",
            "Public MCP",
            "没有可检查的公网 endpoint".into(),
            "配置 canonical URL；Cloudflare Quick 需先启动 managed exposure。",
        )
    } else if public_ok {
        ok_item(
            "public_mcp",
            "public",
            "Public MCP",
            format!("{public_mcp_url} · {public_detail}"),
        )
    } else {
        fail_item(
            "public_mcp",
            "public",
            "Public MCP",
            format!("{public_mcp_url} · {public_detail}"),
            "检查 DNS、TLS、反向代理/隧道路由以及防火墙。",
        )
    });

    let expected_oauth_base = connector_base.clone();
    let oauth_metadata_ok =
        if auth_type == "oauth" && provider_required && !expected_oauth_base.is_empty() {
            let auth_meta = endpoint(&connector_base, "/.well-known/oauth-authorization-server");
            let protected_meta = endpoint(&connector_base, "/.well-known/oauth-protected-resource");
            let auth_result =
                check_authorization_metadata(&client, &auth_meta, &expected_oauth_base).await;
            let protected_result =
                check_protected_resource_metadata(&client, &protected_meta, &expected_oauth_base)
                    .await;
            let ok = auth_result.0 && protected_result.0;
            items.push(if ok {
                ok_item(
                    "oauth_metadata",
                    "oauth",
                    "OAuth Metadata",
                    format!("{}; {}", auth_result.1, protected_result.1),
                )
            } else {
                fail_item(
                "oauth_metadata",
                "oauth",
                "OAuth Metadata",
                format!("{}; {}", auth_result.1, protected_result.1),
                "确认 canonical URL 与实际 HTTPS 入口一致，反向代理必须转发 Host/X-Forwarded-*。",
            )
            });
            ok
        } else if auth_type == "oauth" && provider_required {
            items.push(fail_item(
                "oauth_metadata",
                "oauth",
                "OAuth Metadata",
                "没有可检查的公网 OAuth base URL".into(),
                "先建立公网 endpoint。",
            ));
            false
        } else {
            items.push(skip_item(
                "oauth_metadata",
                "oauth",
                "OAuth Metadata",
                if auth_type == "oauth" {
                    "Local-only 模式不执行公网 OAuth 检查".into()
                } else {
                    format!("认证模式为 {auth_type}")
                },
            ));
            true
        };

    let chatgpt_ready = workspace_count > 0
        && auth_ok
        && runtime_ok
        && local_ok
        && provider_required
        && provider_ok
        && transport_ok
        && public_https
        && public_ok
        && oauth_metadata_ok;
    let summary = if chatgpt_ready {
        "ChatGPT-ready：本地 Gateway、公网入口和认证检查均通过。".to_string()
    } else if canonical.is_empty() && !managed_provider {
        "Local-ready，但尚未配置 ChatGPT 所需的公网 HTTPS 入口。".to_string()
    } else {
        "尚未达到 ChatGPT-ready；请按失败项逐层排查。".to_string()
    };

    Ok(GatewayHealthReport {
        chatgpt_ready,
        summary,
        public_base_url: connector_base,
        items,
    })
}

async fn probe_mcp(client: &reqwest::Client, url: &str) -> (bool, String) {
    if url.trim().is_empty() {
        return (false, "URL 未配置".into());
    }
    match client.get(url).send().await {
        Ok(response) => {
            let code = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            let lower = body.to_ascii_lowercase();
            if lower.contains("powered by") && lower.contains("frp") {
                return (false, format!("HTTP {code}; FRP 默认 404 页"));
            }
            (matches!(code, 200 | 401 | 405), format!("HTTP {code}"))
        }
        Err(error) => (false, error.to_string()),
    }
}

async fn check_authorization_metadata(
    client: &reqwest::Client,
    url: &str,
    expected_base: &str,
) -> (bool, String) {
    let expected_base = expected_base.trim_end_matches('/');
    match get_json(client, url).await {
        Ok(value) => {
            let issuer = value
                .get("issuer")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let authorize = value
                .get("authorization_endpoint")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let token = value
                .get("token_endpoint")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let ok = issuer == expected_base
                && authorize == format!("{expected_base}/oauth/authorize")
                && token == format!("{expected_base}/oauth/token");
            (ok, format!("authorization metadata issuer={issuer}"))
        }
        Err(error) => (false, error),
    }
}

async fn check_protected_resource_metadata(
    client: &reqwest::Client,
    url: &str,
    expected_base: &str,
) -> (bool, String) {
    let expected_base = expected_base.trim_end_matches('/');
    match get_json(client, url).await {
        Ok(value) => {
            let resource = value
                .get("resource")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let servers_ok = value
                .get("authorization_servers")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|items| {
                    items
                        .iter()
                        .any(|item| item.as_str() == Some(expected_base))
                });
            (
                resource == expected_base && servers_ok,
                format!("protected resource={resource}"),
            )
        }
        Err(error) => (false, error),
    }
}

async fn get_json(client: &reqwest::Client, url: &str) -> Result<serde_json::Value, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{url} -> HTTP {}", status.as_u16()));
    }
    response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("{url} -> {error}"))
}

fn endpoint(base: &str, path: &str) -> String {
    if base.trim().is_empty() {
        String::new()
    } else {
        format!("{}{}", base.trim_end_matches('/'), path)
    }
}

fn ok_item(key: &str, layer: &str, label: &str, detail: String) -> GatewayHealthItem {
    item(key, layer, label, "ok", detail, String::new())
}

fn warn_item(key: &str, layer: &str, label: &str, detail: String, hint: &str) -> GatewayHealthItem {
    item(key, layer, label, "warn", detail, hint.into())
}

fn fail_item(key: &str, layer: &str, label: &str, detail: String, hint: &str) -> GatewayHealthItem {
    item(key, layer, label, "fail", detail, hint.into())
}

fn skip_item(key: &str, layer: &str, label: &str, detail: String) -> GatewayHealthItem {
    item(key, layer, label, "skip", detail, String::new())
}

fn item(
    key: &str,
    layer: &str,
    label: &str,
    status: &str,
    detail: String,
    hint: String,
) -> GatewayHealthItem {
    GatewayHealthItem {
        key: key.into(),
        layer: layer.into(),
        label: label.into(),
        status: status.into(),
        detail,
        hint,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_joins_origin_and_path() {
        assert_eq!(
            endpoint("https://mcp.example.com/", "/mcp"),
            "https://mcp.example.com/mcp"
        );
        assert!(endpoint("", "/mcp").is_empty());
    }
}
