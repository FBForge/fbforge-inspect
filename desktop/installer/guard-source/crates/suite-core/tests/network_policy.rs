use fbforge_suite_core::network::{
    AllowedHosts, CancellationToken, DownloadLimits, HttpsUrl, NetworkClient, NetworkConfig,
    NetworkError, ProxyMode,
};
use std::time::Duration;

#[test]
fn proxy_configuration_preserves_system_and_requires_one_explicit_endpoint() {
    let system = NetworkConfig::default();
    assert_eq!(system.proxy(), &ProxyMode::System);
    for server in [
        "proxy.example:8080",
        "proxy:3128",
        "127.0.0.1:8080",
        "[::1]:8080",
    ] {
        let config = NetworkConfig::new(ProxyMode::Explicit {
            server: server.into(),
        })
        .unwrap();
        let json = serde_json::to_string(&config).unwrap();
        let restored: NetworkConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, restored);
    }
    for server in [
        "",
        "proxy",
        "proxy:0",
        "proxy:65536",
        "proxy:080",
        "http://proxy:80",
        "user:password@proxy:80",
        "proxy:80/",
        "proxy:80;DIRECT",
        "proxy:80\n",
        "-proxy:80",
        "proxy..example:80",
        "999.999.999.999:80",
        "[fe80::1%eth0]:80",
    ] {
        assert_eq!(
            NetworkConfig::new(ProxyMode::Explicit {
                server: server.into()
            }),
            Err(NetworkError::InvalidConfiguration)
        );
    }
    for json in [
        r#"{"proxy":{"mode":"explicit","server":"user:secret@proxy:80"}}"#,
        r#"{"proxy":{"mode":"system"},"disableTls":true}"#,
        r#"{"proxy":{"mode":"system","bypass":["*"]}}"#,
    ] {
        assert!(
            serde_json::from_str::<NetworkConfig>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn https_urls_are_bounded_unambiguous_and_do_not_disclose_signed_query_strings() {
    let url =
        HttpsUrl::parse("https://DOWNLOADS.example:443/packages/tool.exe?token=dummy").unwrap();
    assert_eq!(url.host(), "downloads.example");
    assert_eq!(
        HttpsUrl::parse("https://downloads.example")
            .unwrap()
            .request_target(),
        "/"
    );
    assert!(!format!("{url:?}").contains("dummy"));
    for url in [
        "http://downloads.example/tool.exe",
        "https://user:secret@downloads.example/tool.exe",
        "https://downloads.example:444/tool.exe",
        "https://downloads.example./tool.exe",
        "https://downloads.example\\@evil.example/tool.exe",
        "https://127.0.0.1/tool.exe",
        "https://downloads.example/a#fragment",
        "https://downloads.example/a%xx",
        "https://downloads.example/a\r\nInjected:yes",
        "https://downloads.example/<tag>",
    ] {
        assert_eq!(HttpsUrl::parse(url), Err(NetworkError::InvalidUrl));
    }
    assert_eq!(
        HttpsUrl::parse(&format!("https://downloads.example/{}", "a".repeat(32768))),
        Err(NetworkError::InvalidUrl)
    );
}

#[test]
fn each_redirect_is_bound_to_an_exact_allowed_https_host_and_a_hop_limit() {
    let allowed = AllowedHosts::new(["downloads.example", "assets.example"]).unwrap();
    let initial = HttpsUrl::parse("https://downloads.example/releases/tool.exe").unwrap();
    assert!(allowed.permits(&initial));
    let redirected = allowed
        .redirect(&initial, "https://assets.example/content?id=fixture", 0)
        .unwrap();
    assert_eq!(redirected.host(), "assets.example");
    assert_eq!(
        allowed
            .redirect(&initial, "/latest/tool.exe", 4)
            .unwrap()
            .request_target(),
        "/latest/tool.exe"
    );
    for location in [
        "http://assets.example/tool.exe",
        "https://assets.example.attacker.invalid/tool.exe",
        "//attacker.invalid/tool.exe",
        "https://user:dummy@assets.example/tool.exe",
        "relative.exe",
    ] {
        assert!(allowed.redirect(&initial, location, 0).is_err());
    }
    assert_eq!(
        allowed.redirect(&initial, "/tool.exe", 5),
        Err(NetworkError::RedirectLimit)
    );
    assert_eq!(
        AllowedHosts::new(["*.example"]),
        Err(NetworkError::InvalidConfiguration)
    );
    assert_eq!(
        AllowedHosts::new(["assets.example", "ASSETS.EXAMPLE"]),
        Err(NetworkError::InvalidConfiguration)
    );
}

#[test]
fn download_limits_are_finite_and_security_failures_are_not_retried_as_offline() {
    assert!(DownloadLimits::new(1_048_576, Duration::from_secs(60)).is_ok());
    for (size, seconds) in [
        (0, 60),
        (16 * 1024 * 1024 * 1024 + 1, 60),
        (1024, 0),
        (1024, 3601),
    ] {
        assert_eq!(
            DownloadLimits::new(size, Duration::from_secs(seconds)),
            Err(NetworkError::InvalidConfiguration)
        );
    }
    assert!(NetworkError::Timeout.is_transient());
    assert!(NetworkError::HttpStatus(503).is_transient());
    for error in [
        NetworkError::TlsRejected,
        NetworkError::TlsValidationUnavailable,
        NetworkError::SourceNotAllowed,
        NetworkError::ProxyAuthenticationRequired,
        NetworkError::InvalidResponse,
        NetworkError::HttpStatus(404),
    ] {
        assert!(!error.is_transient());
        assert!(!error.to_string().contains("http"));
    }
}

#[test]
fn cancellation_is_shared_and_never_turns_into_success_or_output() {
    let token = CancellationToken::default();
    let other = token.clone();
    other.cancel();
    assert!(token.is_cancelled());
    let client = NetworkClient::new(NetworkConfig::default());
    let url = HttpsUrl::parse("https://downloads.example/tool.exe").unwrap();
    let allowed = AllowedHosts::new(["downloads.example"]).unwrap();
    let mut bytes = Vec::new();
    assert_eq!(
        client.download(
            &url,
            &allowed,
            DownloadLimits::new(1024, Duration::from_secs(30)).unwrap(),
            &token,
            &mut bytes,
            |_| {}
        ),
        Err(NetworkError::Cancelled)
    );
    assert!(bytes.is_empty());
}

#[cfg(not(windows))]
#[test]
fn unsupported_transport_never_claims_a_successful_download() {
    let client = NetworkClient::new(NetworkConfig::default());
    let url = HttpsUrl::parse("https://downloads.example/tool.exe").unwrap();
    let allowed = AllowedHosts::new(["downloads.example"]).unwrap();
    let mut output = Vec::new();
    assert_eq!(
        client.download(
            &url,
            &allowed,
            DownloadLimits::new(1024, Duration::from_secs(30)).unwrap(),
            &CancellationToken::default(),
            &mut output,
            |_| {}
        ),
        Err(NetworkError::UnsupportedPlatform)
    );
    assert!(output.is_empty());
}
