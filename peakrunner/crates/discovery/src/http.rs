use std::{io::{self, Read}, time::Duration};
use crate::{PublicDirectory, ServerAdvert, PROTOCOL, wire::invalid};
pub fn http_get(url: &str, https_only: bool) -> io::Result<Vec<u8>> {
    let config = ureq::Agent::config_builder().https_only(https_only)
        .max_redirects(0).timeout_global(Some(Duration::from_secs(4))).build();
    let mut response = ureq::Agent::new_with_config(config).get(url).call().map_err(io::Error::other)?;
    let mut bytes = Vec::new();
    response.body_mut().as_reader().take(262_145).read_to_end(&mut bytes)?;
    if bytes.len() > 262_144 { return Err(invalid("directory response too large")); }
    Ok(bytes)
}
pub fn browse_https(url: &str) -> io::Result<Vec<ServerAdvert>> {
    let data: PublicDirectory = serde_json::from_slice(&http_get(url, true)?).map_err(io::Error::other)?;
    if data.protocol != PROTOCOL || data.servers.len() > 128 { return Err(invalid("incompatible directory")); }
    for s in &data.servers {
        if !s.host.starts_with("quic://") || crate::quic::endpoint_url(&s.host).is_err()
            || s.name.is_empty() || s.name.len() > 64 || s.name.chars().any(char::is_control)
            || s.players > s.max_players || !(2..=8).contains(&s.max_players) {
            return Err(invalid("unsafe directory listing"));
        }
    }
    Ok(data.servers)
}
