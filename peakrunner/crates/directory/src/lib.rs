//! Standalone discovery service. No simulation, gameplay protocol, or client dependency.
pub mod lan;
use std::{io, sync::{Arc, Mutex}, time::{Duration, Instant}};
use axum::{Router, Json, extract::{State,Path}, http::{StatusCode, header}, response::{IntoResponse, Response}, routing::get};
use peakrunner_discovery::{MatchStatus, ServerAdvert, PublicDirectory, PROTOCOL, quic::{endpoint_url, query_status}};

#[derive(Clone)]
struct Directory {
    status: Arc<Mutex<Option<(MatchStatus, Instant)>>>,
    advertised: String,
}
fn live(directory: &Directory) -> Option<MatchStatus> {
    directory.status.lock().unwrap().as_ref()
        .filter(|(_, seen)| seen.elapsed() < Duration::from_secs(12)).map(|(s, _)| s.clone())
}
async fn health(State(directory): State<Directory>) -> Response {
    match live(&directory) { Some(s) => Json(s).into_response(), None => StatusCode::SERVICE_UNAVAILABLE.into_response() }
}
async fn listing(State(directory): State<Directory>) -> Response {
    let servers = live(&directory).map(|s| vec![ServerAdvert {
        id: "dellcon-north-spine".into(), name: s.name, map: s.map,
        host: directory.advertised.clone(),
        port: endpoint_url(&directory.advertised).unwrap().port().unwrap_or(7777),
        players: s.players, max_players: s.max_players,
    }]).unwrap_or_default();
    let mut response = Json(PublicDirectory { protocol: PROTOCOL.into(), servers }).into_response();
    response.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
}

async fn details(Path(id): Path<String>, State(directory): State<Directory>) -> Response {
    let mut response = if id != "dellcon-north-spine" {
        StatusCode::NOT_FOUND.into_response()
    } else {
        match live(&directory) {
            Some(status) => Json(status).into_response(),
            None => StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    };
    response.headers_mut().insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    response.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    response
}

pub async fn serve(bind: &str, backend: String, advertised: String) -> io::Result<()> {
    endpoint_url(&advertised)?;
    endpoint_url(&backend)?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let directory = Directory { status: Arc::new(Mutex::new(None)), advertised };
    let output = directory.status.clone();
    let poller = tokio::spawn(async move {
        loop {
            if let Ok(status) = query_status(&backend).await {
                *output.lock().unwrap() = Some((status, Instant::now()));
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    });
    let app = Router::new().route("/", get(listing)).route("/servers", get(listing))
        .route("/servers/{id}", get(details))
        .route("/healthz", get(health)).with_state(directory);
    log::info!("Directory listening on {}", listener.local_addr()?);
    let result = axum::serve(listener, app).with_graceful_shutdown(async {
        #[cfg(unix)] {
            let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM");
            tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
        }
        #[cfg(not(unix))] { let _ = tokio::signal::ctrl_c().await; }
    }).await;
    poller.abort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn listings_expire_and_never_expose_stale_matches() {
        let d = Directory { status: Arc::new(Mutex::new(None)), advertised: "quic://play.peakrunner.net:7777".into() };
        assert_eq!(health(State(d.clone())).await.status(), StatusCode::SERVICE_UNAVAILABLE);
        *d.status.lock().unwrap() = Some((MatchStatus { name: "North Spine".into(), map: "Raindance".into(), max_players: 8, ..Default::default() }, Instant::now()));
        let response = listing(State(d.clone())).await;
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let bytes = axum::body::to_bytes(response.into_body(), 4096).await.unwrap();
        let catalog: PublicDirectory = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(catalog.servers.len(), 1);
        assert_eq!(catalog.servers[0].map, "Raindance");
        assert_eq!(details(Path("unknown".into()),State(d.clone())).await.status(),StatusCode::NOT_FOUND);
        assert_eq!(details(Path("dellcon-north-spine".into()),State(d.clone())).await.status(),StatusCode::OK);
        d.status.lock().unwrap().as_mut().unwrap().1 = Instant::now() - Duration::from_secs(13);
        assert!(live(&d).is_none());
        assert_eq!(details(Path("dellcon-north-spine".into()),State(d.clone())).await.status(),StatusCode::SERVICE_UNAVAILABLE);
        let bytes = axum::body::to_bytes(listing(State(d)).await.into_body(), 4096).await.unwrap();
        assert!(serde_json::from_slice::<PublicDirectory>(&bytes).unwrap().servers.is_empty());
    }
}
