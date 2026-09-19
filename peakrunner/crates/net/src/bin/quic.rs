#[tokio::main(worker_threads = 2)]
async fn main() -> std::io::Result<()> {
    if std::env::args().any(|s| s == "--healthcheck") {
        let read = || -> std::io::Result<peakrunner_net::public::MatchStatus> {
            serde_json::from_slice(&peakrunner_net::public::http_get("http://127.0.0.1:8080/status", false)?)
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
    let addr = std::env::var("PEAKRUNNER_UDP_BIND").unwrap_or("0.0.0.0:7777".into()).parse()
        .map_err(std::io::Error::other)?;
    peakrunner_net::quic::run_server(addr, &cert, &key).await
}
