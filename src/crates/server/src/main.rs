#[tokio::main(worker_threads = 2)]
async fn main() -> std::io::Result<()> {
    if std::env::args().any(|s| s == "--healthcheck") {
        let read = || -> std::io::Result<peakrunner_discovery::MatchStatus> {
            serde_json::from_slice(&peakrunner_discovery::http::http_get("http://127.0.0.1:8080/status", false)?)
                .map_err(std::io::Error::other)
        };
        let first = read()?;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if read()?.tick <= first.tick { return Err(std::io::Error::other("simulation is not advancing")); }
        return Ok(());
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cert = std::fs::read(std::env::var("PEAKRUNNER_TLS_CERT").unwrap_or("/run/peakrunner/fullchain.pem".into()))?;
    let key = std::fs::read(std::env::var("PEAKRUNNER_TLS_KEY").unwrap_or("/run/peakrunner/privkey.pem".into()))?;
    // Hosted servers announce themselves to a public directory: set
    // PEAKRUNNER_DIRECTORY_URL (e.g. https://dir.peakrunner.net/announce) and
    // PEAKRUNNER_ADVERTISE_URL (this server's quic://host:port as players
    // reach it). The directory checks the address and queries our status
    // before listing us; announcing again keeps the listing alive.
    if let (Ok(directory), Ok(me)) = (std::env::var("PEAKRUNNER_DIRECTORY_URL"), std::env::var("PEAKRUNNER_ADVERTISE_URL")) {
        peakrunner_discovery::quic::endpoint_url(&me)?;
        std::thread::spawn(move || {
            let https = directory.starts_with("https://");
            std::thread::sleep(std::time::Duration::from_secs(5));
            loop {
                let body = peakrunner_discovery::Announce { url: me.clone() };
                match peakrunner_discovery::http::http_post_json(&directory, &body, https) {
                    Ok(code) if (200..300).contains(&code) => log::debug!("announced {me} ({code})"),
                    Ok(code) => log::warn!("directory refused announce of {me}: HTTP {code}"),
                    Err(e) => log::warn!("directory unreachable for announce: {e}"),
                }
                std::thread::sleep(std::time::Duration::from_secs(20));
            }
        });
    }
    let addr = std::env::var("PEAKRUNNER_UDP_BIND").unwrap_or("0.0.0.0:7777".into()).parse()
        .map_err(std::io::Error::other)?;
    peakrunner_server::quic::run_server(addr, &cert, &key).await
}
