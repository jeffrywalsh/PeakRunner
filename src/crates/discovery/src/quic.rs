//! TLS-verified discovery/control transport, independent of gameplay packets.
use std::{io, net::SocketAddr, sync::Arc, time::Duration};
use serde::Serialize;
use quinn::{Connection, Endpoint};
use tokio::time::timeout;
use crate::wire::invalid;
pub const ALPN: &[u8] = b"peakrunner/4";
pub fn endpoint_url(address: &str) -> io::Result<url::Url> {
    let url = url::Url::parse(address).map_err(|_| invalid("invalid QUIC URL"))?;
    if url.scheme() != "quic" || url.host_str().is_none() || !url.username().is_empty()
        || url.password().is_some() || url.query().is_some() || url.fragment().is_some()
        || !["", "/"].contains(&url.path()) { return Err(invalid("use quic://hostname:7777")); }
    Ok(url)
}
pub fn transport() -> quinn::TransportConfig {
    let mut cfg = quinn::TransportConfig::default();
    cfg.max_concurrent_bidi_streams(1u32.into()).max_concurrent_uni_streams(0u32.into())
        .stream_receive_window(8192u32.into()).receive_window(16384u32.into())
        .datagram_receive_buffer_size(Some(256 * 1024)).datagram_send_buffer_size(16 * 1024)
        .max_idle_timeout(Some(Duration::from_secs(10).try_into().unwrap()))
        .keep_alive_interval(Some(Duration::from_secs(1)));
    cfg
}
pub fn client_config(extra_root: Option<rustls::pki_types::CertificateDer<'static>>) -> io::Result<quinn::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(root) = extra_root { roots.add(root).map_err(io::Error::other)?; }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut tls = rustls::ClientConfig::builder_with_provider(provider).with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(io::Error::other)?.with_root_certificates(roots).with_no_client_auth();
    tls.alpn_protocols = vec![ALPN.to_vec()];
    let crypto = quinn::crypto::rustls::QuicClientConfig::try_from(tls).map_err(io::Error::other)?;
    // Inputs are tiny and replaceable. A snapshot-sized transmit queue can hold
    // seconds of obsolete controls when congestion control reduces the rate.
    let mut input_transport = transport();
    input_transport.datagram_send_buffer_size(1024);
    let mut cfg = quinn::ClientConfig::new(Arc::new(crypto)); cfg.transport_config(Arc::new(input_transport)); Ok(cfg)
}
pub async fn send_control<T: Serialize>(stream: &mut quinn::SendStream, data: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(data).map_err(io::Error::other)?;
    if bytes.len() > 4096 { return Err(invalid("control message too large")); }
    stream.write_all(&(bytes.len() as u32).to_le_bytes()).await.map_err(io::Error::other)?;
    stream.write_all(&bytes).await.map_err(io::Error::other)
}
pub async fn read_control<T: serde::de::DeserializeOwned>(stream: &mut quinn::RecvStream) -> io::Result<T> {
    let mut len = [0u8; 4]; stream.read_exact(&mut len).await.map_err(io::Error::other)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > 4096 { return Err(invalid("control message too large")); }
    let mut bytes = vec![0; len]; stream.read_exact(&mut bytes).await.map_err(io::Error::other)?;
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
pub async fn connect_endpoint(address: &str) -> io::Result<(Endpoint, Connection)> {
    let url = endpoint_url(address)?;
    let host = url.host_str().unwrap().trim_matches(['[', ']']);
    // For pre-DNS deployment checks only. Hostname/certificate verification is
    // unchanged, even when a diagnostic explicitly selects the destination IP.
    let addr = if let Ok(ip) = std::env::var("PEAKRUNNER_QUIC_TEST_IP") {
        SocketAddr::new(ip.parse().map_err(|_| invalid("invalid diagnostic IP"))?, url.port().unwrap_or(7777))
    } else { tokio::net::lookup_host((host, url.port().unwrap_or(7777))).await?
        .next().ok_or_else(|| invalid("server address missing"))? };
    let mut endpoint = Endpoint::client(if addr.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" }.parse().unwrap())?;
    endpoint.set_default_client_config(client_config(None)?);
    let connecting = endpoint.connect(addr, host).map_err(io::Error::other)?;
    let conn = timeout(Duration::from_secs(6), connecting).await.map_err(|_| invalid("UDP connection timed out; check network/firewall"))?
        .map_err(|_| invalid("encrypted UDP connection failed; check server availability and certificate"))?;
    Ok((endpoint, conn))
}
pub async fn query_status(address: &str) -> io::Result<crate::MatchStatus> {
    let (endpoint, conn) = connect_endpoint(address).await?;
    let result = timeout(Duration::from_secs(3), async {
        let (mut send, mut recv) = conn.open_bi().await.map_err(io::Error::other)?;
        send_control(&mut send, &crate::StatusRequest::Status).await?;
        match read_control::<crate::StatusReply>(&mut recv).await? {
            crate::StatusReply::Status { status } => Ok(status),
        }
    }).await.map_err(io::Error::other)?;
    conn.close(0u32.into(), b"status complete"); endpoint.wait_idle().await; result
}
