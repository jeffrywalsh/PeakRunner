use std::{collections::{BTreeMap, VecDeque}, io, time::{Duration, Instant}};
use peakrunner_core::sim::{Command, Snapshot, MAX_PLAYERS};
use serde::{Deserialize, Serialize};
use peakrunner_discovery::wire::invalid;
const CHUNK: usize = 1050;
const MAX_PARTS: usize = 12;
const MAX_SNAPSHOT: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
pub struct Inputs { pub count: u8, pub frames: [Command; 3] }
pub fn inputs(history: &VecDeque<Command>) -> Vec<u8> {
    let mut data = Inputs { count: history.len() as u8, frames: [Command::default(); 3] };
    for (i, command) in history.iter().enumerate() { data.frames[i] = *command; }
    postcard::to_allocvec(&data).expect("fixed input packet")
}
pub fn decode_inputs(bytes: &[u8]) -> io::Result<Vec<Command>> {
    if bytes.len() > 256 { return Err(invalid("input packet too large")); }
    let data: Inputs = postcard::from_bytes(bytes).map_err(|_| invalid("invalid input packet"))?;
    if !(1..=3).contains(&data.count) { return Err(invalid("invalid input count")); }
    let frames = &data.frames[..data.count as usize];
    if frames.iter().any(|c| !c.valid()) || frames.windows(2).any(|w| w[0].seq >= w[1].seq) {
        return Err(invalid("invalid input sequence"));
    }
    Ok(frames.to_vec())
}
pub fn chunks(snapshot: &Snapshot) -> io::Result<Vec<Vec<u8>>> {
    let bytes = postcard::to_allocvec(snapshot).map_err(io::Error::other)?;
    if bytes.len() > MAX_SNAPSHOT { return Err(invalid("snapshot limit exceeded")); }
    let bytes = lz4_flex::compress_prepend_size(&bytes);
    if bytes.len() > CHUNK * MAX_PARTS { return Err(invalid("compressed snapshot limit exceeded")); }
    let count = bytes.len().div_ceil(CHUNK);
    Ok(bytes.chunks(CHUNK).enumerate().map(|(i, chunk)| {
        let mut packet = Vec::with_capacity(chunk.len() + 10);
        packet.extend_from_slice(&snapshot.tick.to_le_bytes());
        packet.push(i as u8); packet.push(count as u8); packet.extend_from_slice(chunk); packet
    }).collect())
}
struct Assembly { born: Instant, parts: Vec<Option<Vec<u8>>> }
#[derive(Default)]
pub struct Reassembly { frames: BTreeMap<u64, Assembly>, completed: u64 }
impl Reassembly {
    pub fn accept(&mut self, packet: &[u8]) -> io::Result<Option<Snapshot>> {
        if packet.len() < 11 || packet.len() > CHUNK + 10 { return Err(invalid("invalid snapshot fragment")); }
        let tick = u64::from_le_bytes(packet[..8].try_into().unwrap());
        let index = packet[8] as usize; let count = packet[9] as usize;
        if count == 0 || count > MAX_PARTS || index >= count { return Err(invalid("invalid fragment count")); }
        if tick <= self.completed { return Ok(None); }
        self.frames.retain(|_, a| a.born.elapsed() < Duration::from_millis(250));
        if !self.frames.contains_key(&tick) && self.frames.len() >= 4 {
            if tick < *self.frames.first_key_value().unwrap().0 { return Ok(None); }
            self.frames.pop_first();
        }
        let frame = self.frames.entry(tick).or_insert_with(|| Assembly { born: Instant::now(), parts: vec![None; count] });
        if frame.parts.len() != count { return Err(invalid("inconsistent snapshot fragments")); }
        frame.parts[index] = Some(packet[10..].to_vec());
        if frame.parts.iter().any(Option::is_none) { return Ok(None); }
        let mut bytes = Vec::new();
        for part in &frame.parts { bytes.extend_from_slice(part.as_ref().unwrap()); }
        if bytes.len() < 4 { return Err(invalid("invalid compressed snapshot")); }
        let size = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        if size == 0 || size > MAX_SNAPSHOT { return Err(invalid("snapshot expansion limit exceeded")); }
        let mut decoded = vec![0; size];
        let decoded_len = lz4_flex::decompress_into(&bytes[4..], &mut decoded).map_err(|_| invalid("invalid snapshot compression"))?;
        if decoded_len != size { return Err(invalid("invalid expanded snapshot length")); }
        let snapshot: Snapshot = postcard::from_bytes(&decoded).map_err(|_| invalid("invalid snapshot data"))?;
        if snapshot.tick != tick || snapshot.players.len() != MAX_PLAYERS || snapshot.acks.len() != MAX_PLAYERS {
            return Err(invalid("invalid snapshot identity"));
        }
        self.completed = tick; self.frames.retain(|key, _| *key > tick);
        Ok(Some(snapshot))
    }
}


impl Reassembly { pub fn pending_frames(&self) -> usize { self.frames.len() } }
