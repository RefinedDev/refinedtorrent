mod bencode_parser;
#[cfg(test)]
mod bencode_tests;
mod peer;
mod torrent;

use anyhow::Result;
use std::sync::{Arc, atomic::AtomicUsize};
use tokio::sync::{Mutex, Notify};

use peer::Peer;
use torrent::Torrent;

pub fn generate_peer_id() -> String {
    const CHARS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

    (0..20)
        .map(|_| CHARS[rand::random_range(0..CHARS.len())] as char)
        .collect()
}

#[tokio::main]
async fn main() -> Result<()> {
    let bytes = std::fs::read("debian.torrent")?;
    let peer_id = generate_peer_id();
    let torrent = Torrent::new(bencode_parser::decode(&bytes)?, &peer_id, "6881");

    let info_bencode = Arc::new(torrent.get_info_bencode());
    
    let hashed_torrent_pieces: Arc<Vec<String>> = Arc::new(torrent.get_piece_hashes()?);
    let bytes_obtained_pieces = Arc::new(Mutex::new(vec![vec![]; hashed_torrent_pieces.len()]));

    let pieces_left = Arc::new(Mutex::new((0..hashed_torrent_pieces.len()).collect()));
    let pieces_done = Arc::new(AtomicUsize::new(0));
    let disconnect_event = Arc::new(Notify::new());

    // While loop because some pieces might still not have been obtained and all the peers disconneted due to whatever reason
    while pieces_done.load(std::sync::atomic::Ordering::Relaxed) != hashed_torrent_pieces.len() {
        let ips = torrent.get_peer_ips(&info_bencode).await?;
        let mut tasks = Vec::with_capacity(ips.len());
        println!("{:?}", ips);
        for ip in ips.iter() {
            let peer = Peer::new(ip);
            peer.connect(
                &mut tasks,
                &peer_id,
                Arc::clone(&info_bencode),
                Arc::clone(&hashed_torrent_pieces),
                Arc::clone(&bytes_obtained_pieces),
                Arc::clone(&pieces_left),
                Arc::clone(&pieces_done),
                torrent.total_length.clone() as u32,
                torrent.piece_length.clone() as u32,
                Arc::clone(&disconnect_event),
            );
        }

        for task in tasks {
            task.await?;
        }
        println!("Done: {:?}", pieces_done);
    }

    // We have disconnected all connections so I can safely get rid of the Mutex
    let mut joined_bytes = Vec::new();
    let mutex = Arc::try_unwrap(bytes_obtained_pieces).unwrap();
    let unlocked = mutex.into_inner();

    for arr in unlocked.into_iter() {
        joined_bytes.extend(arr);
    }
    std::fs::write(torrent.save_file_name, joined_bytes).unwrap();

    Ok(())
}
