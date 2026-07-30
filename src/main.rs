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

#[tokio::main]
async fn main() -> Result<()> {
    let bytes = std::fs::read("sample.torrent")?;
    let torrent = Torrent::new(
        bencode_parser::decode(&bytes)?,
        "hellomynameisgamer12",
        "6881",
    );

    let info_bencode = Arc::new(torrent.get_info_bencode());
    let ips = torrent.get_peer_ips(&info_bencode).await?;

    let hashed_torrent_pieces: Arc<Vec<String>> = Arc::new(torrent.get_piece_hashes()?);
    let bytes_obtained_pieces =
        Arc::new(Mutex::new(vec![vec![]; hashed_torrent_pieces.len()]));

    let pieces_left = Arc::new(Mutex::new((0..hashed_torrent_pieces.len()).collect()));
    let pieces_done = Arc::new(AtomicUsize::new(0));
    let all_done_event = Arc::new(Notify::new());

    let mut tasks = Vec::with_capacity(ips.len());
    println!("Peers: {:?}", ips);
    for ip in ips.iter() {
        let peer = Peer::new(ip);
        peer.connect(
            &mut tasks,
            Arc::clone(&info_bencode),
            Arc::clone(&hashed_torrent_pieces),
            Arc::clone(&bytes_obtained_pieces),
            Arc::clone(&pieces_left),
            Arc::clone(&pieces_done),
            torrent.total_length.clone() as u32,
            torrent.piece_length.clone() as u32,
            Arc::clone(&all_done_event),
        );
    }

    for task in tasks {
        task.await?;
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
