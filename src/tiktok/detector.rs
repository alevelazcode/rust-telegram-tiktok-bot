use url::Url;

const TIKTOK_HOSTS: &[&str] = &[
    "tiktok.com",
    "www.tiktok.com",
    "vm.tiktok.com",
    "vt.tiktok.com",
    "m.tiktok.com",
];

/// Maximum URL length to prevent abuse from extremely long strings.
const MAX_URL_LENGTH: usize = 512;

/// Maximum TikTok URLs to extract per message to prevent abuse.
const MAX_URLS_PER_MESSAGE: usize = 3;

pub fn is_tiktok_url(url_str: &str) -> bool {
    if url_str.len() > MAX_URL_LENGTH {
        return false;
    }

    let Ok(url) = Url::parse(url_str) else {
        return false;
    };

    // Only allow HTTPS
    if url.scheme() != "https" {
        return false;
    }

    // Reject URLs with embedded credentials
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }

    // Reject non-standard ports
    if url.port().is_some() {
        return false;
    }

    is_tiktok_host(url.host_str())
}

fn is_tiktok_host(host: Option<&str>) -> bool {
    match host {
        Some(h) => TIKTOK_HOSTS.iter().any(|&known| h == known),
        None => false,
    }
}

pub fn extract_tiktok_urls(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace()
        .filter(|word| is_tiktok_url(word))
        .filter(|word| seen.insert(*word))
        .map(|s| s.to_string())
        .take(MAX_URLS_PER_MESSAGE)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("https://www.tiktok.com/@user/video/123", true)]
    #[case("https://tiktok.com/@user/video/123", true)]
    #[case("https://vm.tiktok.com/ZMxyz123/", true)]
    #[case("https://vt.tiktok.com/ZSabc456/", true)]
    #[case("https://m.tiktok.com/@user/video/789", true)]
    #[case("https://www.youtube.com/watch?v=abc", false)]
    #[case("https://instagram.com/reel/123", false)]
    #[case("https://example.com", false)]
    #[case("not-a-url", false)]
    #[case("", false)]
    #[case("tiktok.com", false)] // no scheme
    #[case("https://faketiktok.com/video/123", false)]
    #[case("https://tiktok.com.evil.com/video/123", false)]
    fn is_tiktok_url_cases(#[case] url: &str, #[case] expected: bool) {
        assert_eq!(is_tiktok_url(url), expected, "URL: {}", url);
    }

    #[rstest]
    #[case("http://www.tiktok.com/@user/video/123", false)] // HTTP rejected
    #[case("https://user:pass@tiktok.com/video/123", false)] // credentials rejected
    #[case("https://tiktok.com:8080/video/123", false)] // non-standard port rejected
    fn security_hardening_cases(#[case] url: &str, #[case] expected: bool) {
        assert_eq!(is_tiktok_url(url), expected, "URL: {}", url);
    }

    #[test]
    fn rejects_very_long_urls() {
        let long_url = format!("https://tiktok.com/{}", "a".repeat(600));
        assert!(!is_tiktok_url(&long_url));
    }

    #[test]
    fn extract_caps_at_max_urls() {
        let urls: Vec<String> = (0..10)
            .map(|i| format!("https://tiktok.com/@user/video/{}", i))
            .collect();
        let text = urls.join(" ");
        let extracted = extract_tiktok_urls(&text);
        assert_eq!(extracted.len(), MAX_URLS_PER_MESSAGE);
    }

    #[test]
    fn extract_single_url() {
        let text = "Mira este video https://www.tiktok.com/@user/video/123 es genial";
        let urls = extract_tiktok_urls(text);
        assert_eq!(urls, vec!["https://www.tiktok.com/@user/video/123"]);
    }

    #[test]
    fn extract_multiple_urls() {
        let text = "https://vm.tiktok.com/abc https://www.tiktok.com/@x/video/1";
        let urls = extract_tiktok_urls(text);
        assert_eq!(urls.len(), 2);
    }

    #[test]
    fn extract_ignores_non_tiktok_urls() {
        let text = "https://youtube.com/watch?v=1 https://tiktok.com/v/2 https://google.com";
        let urls = extract_tiktok_urls(text);
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("tiktok.com"));
    }

    #[test]
    fn extract_deduplicates_urls() {
        let text = "https://tiktok.com/@user/video/123 check this https://tiktok.com/@user/video/123";
        let urls = extract_tiktok_urls(text);
        assert_eq!(urls.len(), 1);
    }

    #[test]
    fn extract_empty_text() {
        assert!(extract_tiktok_urls("").is_empty());
    }

    #[test]
    fn extract_no_urls_in_text() {
        assert!(extract_tiktok_urls("just some regular text here").is_empty());
    }
}
