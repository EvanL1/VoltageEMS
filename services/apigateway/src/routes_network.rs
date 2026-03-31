use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use axum::{extract::Query, extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use serde_json::json;
use tracing::error;

use crate::models::{NetworkConfig, NetworkUpdateRequest};
use crate::state::AppState;

// ── LAN file mapping ──────────────────────────────────────────────────────────

fn lan_file(config_dir: &str, lan: u8) -> Option<PathBuf> {
    let file = match lan {
        1 => "10-eth0.network",
        2 => "11-eth1.network",
        3 => "12-eth2.network",
        4 => "13-eth3.network",
        _ => return None,
    };
    Some(Path::new(config_dir).join(file))
}

fn lan_iface(lan: u8) -> Option<&'static str> {
    match lan {
        1 => Some("eth0"),
        2 => Some("eth1"),
        3 => Some("eth2"),
        4 => Some("eth3"),
        _ => None,
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

fn cidr_to_mask(cidr: u8) -> String {
    if cidr == 0 {
        return "0.0.0.0".to_string();
    }
    let mask: u32 = if cidr >= 32 {
        0xFFFF_FFFF
    } else {
        0xFFFF_FFFF << (32 - cidr)
    };
    let [a, b, c, d] = mask.to_be_bytes();
    format!("{}.{}.{}.{}", a, b, c, d)
}

fn mask_to_cidr(mask: &str) -> Option<u8> {
    let addr = Ipv4Addr::from_str(mask).ok()?;
    let bits: u32 = u32::from(addr);
    if bits == 0 {
        return Some(0);
    }
    // Must be a contiguous mask
    let trailing = bits.trailing_zeros();
    let cidr = (32 - trailing) as u8;
    if u32::from(Ipv4Addr::from(0xFFFF_FFFF_u32 << trailing)) == bits {
        Some(cidr)
    } else {
        None
    }
}

fn parse_config(content: &str) -> NetworkConfig {
    #[derive(Default, Clone)]
    struct Fields {
        dhcp: Option<bool>,
        ip: String,
        subnet_mask: String,
        gateway: String,
        dns: Vec<String>,
    }

    let mut active = Fields::default();
    let mut commented = Fields::default();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let is_commented = line.starts_with('#');
        let parsed = if is_commented {
            line.trim_start_matches('#').trim()
        } else {
            line
        };

        if !parsed.contains('=') {
            continue;
        }

        let (key, value) = parsed.split_once('=').unwrap();
        let key = key.trim();
        let value = value.trim();

        let target = if is_commented {
            &mut commented
        } else {
            &mut active
        };

        match key {
            "DHCP" => target.dhcp = Some(value.eq_ignore_ascii_case("yes")),
            "Address" => {
                if let Some((ip, cidr)) = value.rsplit_once('/') {
                    target.ip = ip.trim().to_string();
                    target.subnet_mask = cidr
                        .trim()
                        .parse::<u8>()
                        .map(cidr_to_mask)
                        .unwrap_or_default();
                } else {
                    target.ip = value.to_string();
                }
            },
            "Gateway" => target.gateway = value.to_string(),
            "DNS" => target.dns.push(value.to_string()),
            _ => {},
        }
    }

    let dns1_active = active.dns.first().cloned().unwrap_or_default();
    let dns2_active = active.dns.get(1).cloned().unwrap_or_default();
    let dns1_commented = commented.dns.first().cloned().unwrap_or_default();
    let dns2_commented = commented.dns.get(1).cloned().unwrap_or_default();

    NetworkConfig {
        dhcp: active.dhcp.unwrap_or(false),
        ip: if active.ip.is_empty() {
            commented.ip
        } else {
            active.ip
        },
        subnet_mask: if active.subnet_mask.is_empty() {
            commented.subnet_mask
        } else {
            active.subnet_mask
        },
        gateway: if active.gateway.is_empty() {
            commented.gateway
        } else {
            active.gateway
        },
        dns1: if dns1_active.is_empty() {
            dns1_commented
        } else {
            dns1_active
        },
        dns2: if dns2_active.is_empty() {
            dns2_commented
        } else {
            dns2_active
        },
    }
}

fn build_config(
    iface: &str,
    dhcp: bool,
    ip: &str,
    cidr: u8,
    gateway: &str,
    dns1: &str,
    dns2: &str,
) -> String {
    let comment = |enabled: bool, line: &str| -> String {
        if enabled {
            line.to_string()
        } else {
            format!("#{}", line)
        }
    };

    let static_enabled = !dhcp;
    let address_line = if ip.is_empty() {
        "Address=".to_string()
    } else {
        format!("Address={}/{}", ip, cidr)
    };
    let gateway_line = if gateway.is_empty() {
        "Gateway=".to_string()
    } else {
        format!("Gateway={}", gateway)
    };
    let dns1_line = if dns1.is_empty() {
        "DNS=".to_string()
    } else {
        format!("DNS={}", dns1)
    };
    let dns2_line = if dns2.is_empty() {
        "DNS=".to_string()
    } else {
        format!("DNS={}", dns2)
    };

    format!(
        "[Match]\nName={iface}\nKernelCommandLine=!root=/dev/nfs\n\n[Network]\n{dhcp_line}\n{addr_line}\n{gw_line}\n{d1_line}\n{d2_line}\n\n[DHCP]\nClientIdentifier=mac\n",
        iface = iface,
        dhcp_line = comment(dhcp, "DHCP=yes"),
        addr_line = comment(static_enabled, &address_line),
        gw_line = comment(static_enabled, &gateway_line),
        d1_line = comment(static_enabled, &dns1_line),
        d2_line = comment(static_enabled, &dns2_line),
    )
}

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::IntoParams)]
pub struct LanQuery {
    /// LAN 口编号（1-4）
    lan: u8,
}

// ── GET /api/v1/network ───────────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/network", tag = "Network",
    security(("bearer_auth" = [])),
    params(LanQuery),
    responses((status = 200, description = "网络配置", body = NetworkConfig), (status = 400, description = "无效 LAN 编号")))]
pub async fn get_network_config(
    State(state): State<Arc<AppState>>,
    Query(q): Query<LanQuery>,
) -> impl IntoResponse {
    let Some(path) = lan_file(&state.config.network_config_dir, q.lan) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": format!("Invalid LAN number: {}. Valid range: 1-4", q.lan)})),
        )
            .into_response();
    };

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let config = parse_config(&content);
            Json(json!({
                "success": true,
                "message": format!("LAN{} network config retrieved", q.lan),
                "data": config,
            }))
            .into_response()
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": format!("LAN{} config file not found: {:?}", q.lan, path)})),
        )
            .into_response(),
        Err(e) => {
            error!("Read network config error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ── PUT /api/v1/network ───────────────────────────────────────────────────────

#[utoipa::path(put, path = "/api/v1/network", tag = "Network",
    security(("bearer_auth" = [])),
    request_body = NetworkUpdateRequest,
    responses((status = 200, description = "配置已保存"), (status = 400, description = "无效参数")))]
pub async fn update_network_config(
    State(state): State<Arc<AppState>>,
    Json(req): Json<NetworkUpdateRequest>,
) -> impl IntoResponse {
    let Some(path) = lan_file(&state.config.network_config_dir, req.lan) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": format!("Invalid LAN number: {}", req.lan)})),
        )
            .into_response();
    };

    let Some(iface) = lan_iface(req.lan) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": format!("Invalid LAN number: {}", req.lan)})),
        )
            .into_response();
    };

    let ip = req.ip.as_deref().unwrap_or("");
    let mask = req.subnet_mask.as_deref().unwrap_or("");
    let gateway = req.gateway.as_deref().unwrap_or("");
    let dns1 = req.dns1.as_deref().unwrap_or("");
    let dns2 = req.dns2.as_deref().unwrap_or("");

    if !req.dhcp {
        if ip.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "The ip field is required in static IP mode"})),
            )
                .into_response();
        }
        if mask.is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"success": false, "message": "The subnet_mask field is required in static IP mode"}),
                ),
            )
                .into_response();
        }
    }

    let cidr = if mask.is_empty() {
        0u8
    } else {
        match mask_to_cidr(mask) {
            Some(c) => c,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": format!("Invalid subnet mask: {}", mask)})),
                )
                    .into_response();
            },
        }
    };

    let content = build_config(iface, req.dhcp, ip, cidr, gateway, dns1, dns2);

    match std::fs::write(&path, content) {
        Ok(_) => Json(json!({
            "success": true,
            "message": format!("LAN{} network config updated", req.lan),
            "data": {
                "lan": req.lan,
                "interface": iface,
                "file": path.to_string_lossy(),
                "dhcp": req.dhcp,
            }
        }))
        .into_response(),
        Err(e) => {
            error!("Write network config error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Failed to write network config: {}", e)})),
            )
                .into_response()
        },
    }
}

// ── POST /api/v1/network/apply ────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/network/apply", tag = "Network",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "网络配置已应用"), (status = 500, description = "操作失败")))]
pub async fn apply_network_config() -> impl IntoResponse {
    use std::process::Command;

    const DOCKER_SOCK: &str = "/var/run/docker.sock";

    if !std::path::Path::new(DOCKER_SOCK).exists() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "success": false,
                "message": "Docker socket unavailable, ensure /var/run/docker.sock is mounted",
            })),
        )
            .into_response();
    }

    let result = Command::new("docker")
        .args([
            "run",
            "--rm",
            "--pid",
            "host",
            "--privileged",
            "alpine:latest",
            "nsenter",
            "--target",
            "1",
            "--mount",
            "--uts",
            "--ipc",
            "--net",
            "--pid",
            "--",
            "systemctl",
            "restart",
            "systemd-networkd",
        ])
        .output();

    match result {
        Ok(output) if output.status.success() => Json(json!({
            "success": true,
            "message": "Network config applied (systemd-networkd restarted)",
        }))
        .into_response(),
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            error!("Restart systemd-networkd failed: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Failed to restart systemd-networkd: {}", err)})),
            )
                .into_response()
        },
        Err(e) => {
            error!("Docker command error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Failed to execute docker command: {}", e)})),
            )
                .into_response()
        },
    }
}
