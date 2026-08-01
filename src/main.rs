mod bencode_parser;
#[cfg(test)]
mod bencode_tests;
mod cli;
mod peer;
mod torrent;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
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
    env_logger::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match cli::parse_args(&args) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("error: {e:#}\n");
            cli::print_usage();
            std::process::exit(1);
        }
    };

    let bytes = std::fs::read(&cli.torrent_file)?;
    let peer_id = generate_peer_id();
    let torrent = Torrent::new(bencode_parser::decode(&bytes)?, &peer_id, &cli.port)?;

    let info_bencode = Arc::new(torrent.get_info_bencode()?);

    let hashed_torrent_pieces: Arc<Vec<String>> = Arc::new(torrent.get_piece_hashes()?);
    let bytes_obtained_pieces = Arc::new(Mutex::new(vec![vec![]; hashed_torrent_pieces.len()]));

    let pieces_left = Arc::new(Mutex::new((0..hashed_torrent_pieces.len()).collect()));
    let pieces_done = Arc::new(AtomicUsize::new(0));
    let disconnect_event = Arc::new(Notify::new());

    // Setup progress bar
    let connected_peers = Arc::new(AtomicUsize::new(0));
    let pb = ProgressBar::new(hashed_torrent_pieces.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} pieces | {msg} | {eta}"
        )?
        .progress_chars("█▓░"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    let pb = Arc::new(pb);

    {
        // Loop for updating the number of peers
        let pb = Arc::clone(&pb);
        let connected_peers = Arc::clone(&connected_peers);
        let hashed_torrent_pieces = Arc::clone(&hashed_torrent_pieces);
        let pieces_done = Arc::clone(&pieces_done);
        tokio::spawn(async move {
            loop {
                if pieces_done.load(Ordering::Relaxed) == hashed_torrent_pieces.len() {
                    break;
                }
                pb.set_message(format!("{} peers", connected_peers.load(Ordering::Relaxed),));
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }

    // While loop because some pieces might still not have been obtained and all the peers disconneted due to whatever reason
    while pieces_done.load(Ordering::Relaxed) != hashed_torrent_pieces.len() {
        let ips = torrent.get_peer_ips(&info_bencode).await?;
        let mut tasks = Vec::with_capacity(ips.len());
        println!("Total peers: {:?}", ips.len());
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
                torrent.total_length as u32,
                torrent.piece_length as u32,
                Arc::clone(&disconnect_event),
                Arc::clone(&pb),
                Arc::clone(&connected_peers),
            );
        }

        for task in tasks {
            task.await?;
        }
    }

    pb.finish_with_message("Download complete");
    // We have disconnected all connections so I can safely get rid of the Mutex
    let mut joined_bytes = Vec::new();
    let mutex = Arc::try_unwrap(bytes_obtained_pieces).expect("Connections are still active");
    let unlocked = mutex.into_inner();

    for arr in unlocked.into_iter() {
        joined_bytes.extend(arr);
    }
    std::fs::create_dir_all(&cli.output_dir)?;
    std::fs::write(cli.output_dir.join(torrent.save_file_name), joined_bytes)?;

    Ok(())
}
