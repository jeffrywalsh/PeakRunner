use std::io::{self, BufRead, Write};
use std::net::{SocketAddr, ToSocketAddrs};

use serde::de::DeserializeOwned;
use serde::Serialize;

pub fn resolve(addr: &str) -> io::Result<SocketAddr> {
    addr.to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("no address for {addr}")))
}

pub fn write_msg(stream: &mut impl Write, msg: &impl Serialize) -> io::Result<()> {
    let mut line = serde_json::to_string(msg)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
    line.push('\n');
    stream.write_all(line.as_bytes())?;
    stream.flush()
}

pub fn read_msg<T: DeserializeOwned>(reader: &mut impl BufRead) -> io::Result<T> {
    let mut line = String::new();
    let n = reader.read_line(&mut line)?;
    if n == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "connection closed"));
    }
    serde_json::from_str(line.trim()).map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))
}
