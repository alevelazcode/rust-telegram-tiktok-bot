use std::net::{IpAddr, ToSocketAddrs};
use url::Url;

use crate::error::BotError;

/// Validates that a download URL is safe (not pointing to internal/private resources).
/// Prevents SSRF attacks where the TikWM API could return a malicious URL.
pub fn validate_download_url(video_url: &str) -> Result<(), BotError> {
    let url = Url::parse(video_url).map_err(|_| BotError::UnsafeUrl)?;

    // Only allow HTTP/HTTPS
    if url.scheme() != "https" && url.scheme() != "http" {
        tracing::warn!(url = %video_url, "Blocked non-HTTP scheme");
        return Err(BotError::UnsafeUrl);
    }

    // Must have a host
    let host = url.host_str().ok_or(BotError::UnsafeUrl)?;

    // Reject credentials in URL
    if !url.username().is_empty() || url.password().is_some() {
        tracing::warn!(url = %video_url, "Blocked URL with embedded credentials");
        return Err(BotError::UnsafeUrl);
    }

    // Resolve DNS and check all resulting IPs are public
    let port = url.port_or_known_default().unwrap_or(443);
    let socket_addr = format!("{}:{}", host, port);
    let addrs: Vec<_> = socket_addr
        .to_socket_addrs()
        .map_err(|_| BotError::UnsafeUrl)?
        .collect();

    if addrs.is_empty() {
        return Err(BotError::UnsafeUrl);
    }

    for addr in &addrs {
        if is_private_or_reserved(addr.ip()) {
            tracing::warn!(
                url = %video_url,
                resolved_ip = %addr.ip(),
                "Blocked download URL resolving to private/reserved IP"
            );
            return Err(BotError::UnsafeUrl);
        }
    }

    Ok(())
}

fn is_private_or_reserved(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()          // 127.0.0.0/8
            || v4.is_private()        // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
            || v4.is_link_local()     // 169.254.0.0/16 (AWS metadata endpoint)
            || v4.is_broadcast()      // 255.255.255.255
            || v4.is_unspecified()    // 0.0.0.0
            || v4.is_documentation()  // 192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24
            || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64)  // 100.64.0.0/10 (CGNAT)
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()       // ::1
            || v6.is_unspecified() // ::
            || v6.to_ipv4_mapped().is_some_and(|v4| is_private_or_reserved(IpAddr::V4(v4)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_scheme() {
        assert!(validate_download_url("ftp://cdn.example.com/video.mp4").is_err());
        assert!(validate_download_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn rejects_urls_with_credentials() {
        assert!(validate_download_url("https://user:pass@cdn.example.com/video.mp4").is_err());
    }

    #[test]
    fn rejects_localhost() {
        assert!(validate_download_url("http://127.0.0.1/video.mp4").is_err());
        assert!(validate_download_url("http://localhost/video.mp4").is_err());
    }

    #[test]
    fn rejects_private_ips() {
        assert!(validate_download_url("http://10.0.0.1/video.mp4").is_err());
        assert!(validate_download_url("http://192.168.1.1/video.mp4").is_err());
        assert!(validate_download_url("http://172.16.0.1/video.mp4").is_err());
    }

    #[test]
    fn rejects_aws_metadata_endpoint() {
        assert!(validate_download_url("http://169.254.169.254/latest/meta-data/").is_err());
    }

    #[test]
    fn rejects_no_host() {
        assert!(validate_download_url("https:///video.mp4").is_err());
    }

    #[test]
    fn private_ip_detection() {
        use std::net::Ipv4Addr;

        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1))));

        // Public IPs should pass
        assert!(!is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }
}
