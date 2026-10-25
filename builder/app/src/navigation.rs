/// Allow the embedded macOS app origin and, in debug builds, the local Vite origin.
pub fn is_allowed_navigation(url: &tauri::Url, dev: bool) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    // tauri-2.12.0/src/manager/mod.rs:334-337 uses protocol::origin("tauri", ...).
    // tauri-2.12.0/src/protocol/mod.rs:30-38 selects tauri://localhost on macOS.
    (url.scheme() == "tauri" && url.host_str() == Some("localhost") && url.port().is_none())
        || (dev
            && url.scheme() == "http"
            && url.host_str() == Some("127.0.0.1")
            && url.port() == Some(5180))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_only_the_production_origin() {
        for dev in [false, true] {
            for url in [
                "tauri://localhost",
                "tauri://localhost/index.html",
                "tauri://localhost/path?x=1#part",
            ] {
                assert!(is_allowed_navigation(&url.parse().unwrap(), dev), "{url}");
            }
        }
    }

    #[test]
    fn development_origin_requires_dev_mode() {
        for url in [
            "http://127.0.0.1:5180",
            "http://127.0.0.1:5180/path?x=1#part",
        ] {
            let url = url.parse().unwrap();
            assert!(is_allowed_navigation(&url, true));
            assert!(!is_allowed_navigation(&url, false));
        }
    }

    #[test]
    fn rejects_other_hosts_ports_schemes_and_credentials() {
        for dev in [false, true] {
            for url in [
                "tauri://elsewhere",
                "tauri://localhost.evil",
                "tauri://localhost:5180",
                "tauri://user@localhost",
                "tauri://localhost@evil",
                "http://localhost:5180",
                "http://127.0.0.1:5173",
                "http://127.0.0.1:5181",
                "http://127.0.0.1",
                "http://evil:5180",
                "http://user:pass@127.0.0.1:5180",
                "https://127.0.0.1:5180",
                "https://tauri.localhost",
                "https://example.com",
                "file:///tmp/private",
                "data:text/html,hello",
                "javascript:alert(1)",
                "about:blank",
                "blob:tauri://localhost/id",
            ] {
                assert!(
                    !is_allowed_navigation(&url.parse().unwrap(), dev),
                    "{url}, dev={dev}"
                );
            }
        }
    }
}
