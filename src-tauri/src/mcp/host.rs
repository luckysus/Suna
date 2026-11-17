use std::net::IpAddr;

/// Block private / link-local / unspecified IP literals for MCP HTTP(S) URLs.
/// Loopback stays allowed for local MCP servers. DNS hostnames are allowed
/// (desktop user configures them); this stops the common SSRF IP forms.
pub fn mcp_host_is_blocked(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") {
        return false;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            ip.is_private() || ip.is_link_local() || ip.is_unspecified() || ip.is_broadcast()
        }
        Ok(IpAddr::V6(ip)) => {
            ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_unspecified()
        }
        Err(_) => false,
    }
}

pub fn reject_blocked_mcp_host(host: Option<&str>) -> Result<(), String> {
    let Some(host) = host.filter(|value| !value.is_empty()) else {
        return Err("MCP URL must include a host".to_string());
    };
    if mcp_host_is_blocked(host) {
        return Err(
            "MCP URL must not target private, link-local, or unspecified addresses".to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_loopback_and_public_hosts() {
        assert!(!mcp_host_is_blocked("localhost"));
        assert!(!mcp_host_is_blocked("127.0.0.1"));
        assert!(!mcp_host_is_blocked("::1"));
        assert!(!mcp_host_is_blocked("api.example.com"));
        assert!(!mcp_host_is_blocked("8.8.8.8"));
    }

    #[test]
    fn blocks_private_and_link_local() {
        assert!(mcp_host_is_blocked("10.0.0.1"));
        assert!(mcp_host_is_blocked("192.168.1.1"));
        assert!(mcp_host_is_blocked("172.16.0.1"));
        assert!(mcp_host_is_blocked("169.254.169.254"));
        assert!(mcp_host_is_blocked("0.0.0.0"));
        assert!(mcp_host_is_blocked("fd12::1"));
        assert!(mcp_host_is_blocked("fe80::1"));
    }
}
