use anyhow::Result;
use indicatif::ProgressBar;
use log::{info, warn};
use std::sync::{
    Arc, atomic::{AtomicUsize, Ordering},
};

use sha1::{Digest, Sha1};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{Mutex, Notify},
    task::JoinHandle,
};

fn peer_has_piece(bytes: &[u8], bit: usize) -> bool {
    let bit_index = bit / 8;
    let bit_offset = bit % 8;
    (bytes[bit_index] >> (7 - bit_offset)) & 1 == 1
}
pub struct Peer<'a> {
    ip: &'a str,
}

impl<'a> Peer<'a> {
    pub fn new(ip: &'a str) -> Self {
        Peer { ip }
    }

    pub fn connect(
        &self,
        tasks: &mut Vec<JoinHandle<()>>,
        peer_id: &str,
        info_hash: Arc<Vec<u8>>,

        hashed_torrent_pieces: Arc<Vec<String>>,
        bytes_obtained_pieces: Arc<Mutex<Vec<Vec<u8>>>>,
        pieces_left: Arc<Mutex<Vec<usize>>>,
        pieces_done: Arc<AtomicUsize>,
        
        total_length: u32,
        piece_length: u32,
        disconnect_event: Arc<Notify>,
        progress_bar: Arc<ProgressBar>,
        connected_peers: Arc<AtomicUsize>,
    ) {
        let ip = self.ip.to_owned();
        let peer_id = peer_id.to_owned();
        tasks.push(tokio::spawn(async move {
            let current_piece_index = AtomicUsize::new(usize::MAX);

            let result = async || -> Result<()> {
                connected_peers.fetch_add(1, Ordering::Relaxed);
                let mut stream = TcpStream::connect(&ip).await?;
                let mut choked = true;

                let mut piece_buffer: Vec<u8> = Vec::new();
                let mut block_length: u32 = 0;

                let mut h1 = [0u8; 68]; // To establish a connection we need to do a handshake
                h1[0] = 19;
                h1[1..20].copy_from_slice(b"BitTorrent protocol");
                h1[20..28].fill(0);
                h1[28..48].copy_from_slice(&Sha1::digest(&*info_hash));
                h1[48..68].copy_from_slice(peer_id.as_bytes());
                stream.write_all(&h1).await?;
                let mut h2 = [0u8; 68]; // The peer returns something similar in return
                stream.read_exact(&mut h2).await?;

                // Get 'bitfield' payload
                let mut length_bytes = [0u8; 4];
                let mut message_id = [0u8; 1];
                stream.read_exact(&mut length_bytes).await?;
                stream.read_exact(&mut message_id).await?; // message_id for bitfield is 5
                let mut bitfield = vec![0u8; (u32::from_be_bytes(length_bytes) - 1) as usize]; // Message takes 1 length
                stream.read_exact(&mut bitfield).await?;

                // Send 'interested' Message
                let length_bytes = (1 as u32).to_be_bytes();
                let message_id: [u8; 1] = [2];
                stream.write_all(&length_bytes).await?;
                stream.write_all(&message_id).await?;

                loop {
                    if pieces_done.load(Ordering::Relaxed) == hashed_torrent_pieces.len() {
                        connected_peers.fetch_sub(1, Ordering::Relaxed);
                        drop(stream);
                        break;
                    }

                    let mut length_bytes = [0u8; 4];
                    let mut message_id = [0u8; 1];

                    tokio::select! {
                        result = stream.read_exact(&mut length_bytes) => {
                            result?;
                            stream.read_exact(&mut message_id).await?;
                        }
                        _ = disconnect_event.notified() => {
                            connected_peers.fetch_sub(1, Ordering::Relaxed);
                            drop(stream);
                            break;
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_secs(90)) => {
                            info!("<{ip}> timed out!");
                            connected_peers.fetch_sub(1, Ordering::Relaxed);
                            drop(stream);
                            break;
                        }
                    }

                    match message_id {
                        [0] => choked = true,
                        [1] => choked = false,
                        [2] => (), // peer interested but i aint seeding 👻
                        [3] => (), // peer not interested 👻
                        [4] => {}  // TODO: the peer has a piece we want now but didnt have earlier
                        [5] => (), // bitfield (we already have it)
                        [6] => (), // peer request but i aint seeding 👻
                        [7] => {
                            let current_piece_index_get = current_piece_index.load(Ordering::Relaxed);
                            // All pieces except MAYBE the last one have the same length, the piece can be truncated and then the last won't be the same as rest
                            let this_piece_length =
                                if current_piece_index_get == hashed_torrent_pieces.len() - 1 {
                                    total_length
                                        - piece_length * (hashed_torrent_pieces.len() as u32 - 1)
                                } else {
                                    piece_length
                                };

                            let mut payload =
                                vec![0u8; (u32::from_be_bytes(length_bytes) - 1) as usize]; // Message takes 1 length
                            stream.read_exact(&mut payload).await?;

                            // Sanity check; why getting a piece if never asked?
                            if current_piece_index_get == usize::MAX {
                                continue;
                            }

                            if piece_buffer.is_empty() {
                                piece_buffer = vec![0u8; this_piece_length as usize];
                            }

                            let _index = u32::from_be_bytes(payload[0..4].try_into()?);
                            let begin = u32::from_be_bytes(payload[4..8].try_into()?);
                            let block = &payload[8..];
                            piece_buffer[begin as usize..begin as usize + block.len()]
                                .copy_from_slice(block);
                            block_length += block.len() as u32;

                            if block_length >= this_piece_length {
                                // Should'nt ever be greater tho but still
                                let piece_hash = &hashed_torrent_pieces[current_piece_index_get];
                                let buffer_hashed = crate::torrent::sha1_bytes_to_hex(
                                    Sha1::digest(&piece_buffer),
                                    false,
                                )?;
                                if piece_hash == &buffer_hashed {
                                    bytes_obtained_pieces.lock().await[current_piece_index_get] =
                                        piece_buffer;
                                    let done = pieces_done.fetch_add(1, Ordering::Relaxed) + 1;
                                    progress_bar.inc(1);
                                    if done == hashed_torrent_pieces.len() {
                                        disconnect_event.notify_waiters(); // Tell every peer everything is downloaded and to disconnect
                                    }
                                    if current_piece_index_get == 0 {
                                        info!(
                                            "PiecesLeft: {:?}\nDone: {}\nTotal: {}",
                                            pieces_left,
                                            done,
                                            hashed_torrent_pieces.len()
                                        );
                                    }
                                } else {
                                    // We reject this piece, bad peer
                                    pieces_left.lock().await.push(current_piece_index_get);
                                }
                                block_length = 0;
                                current_piece_index.store(usize::MAX, Ordering::Relaxed);
                                piece_buffer = vec![];
                            }
                        }
                        [8] => (), // cancel
                        _ => (),
                    }

                    if !choked && current_piece_index.load(Ordering::Relaxed) == usize::MAX {
                        // Not choked and not already trying to join a piece
                        // check if peer has piece and then send a request
                        {
                            let pieces = &mut *pieces_left.lock().await;
                            if pieces.is_empty() {
                                continue;
                            };
                            let piece_index = pieces.last().unwrap();
                            if !peer_has_piece(&bitfield, *piece_index) {
                                continue;
                            };
                            current_piece_index.store(pieces.pop().unwrap(), Ordering::Relaxed);
                        } // piece mutex lock unlocks for other tasks

                        // All pieces except MAYBE the last one have the same length, the piece can be truncated and then the last won't be the same as rest
                        let this_piece_length = if current_piece_index.load(Ordering::Relaxed)
                            == hashed_torrent_pieces.len() - 1
                        {
                            total_length - piece_length * (hashed_torrent_pieces.len() as u32 - 1)
                        } else {
                            piece_length
                        };

                        let mut begin_offset: u32 = 0;

                        while begin_offset < this_piece_length {
                            let length = std::cmp::min(16 * 1024, this_piece_length - begin_offset);

                            let length_bytes = (13u32).to_be_bytes();
                            let message_id: [u8; 1] = [6];
                            let mut payload = [0u8; 12];

                            payload[0..4]
                                .copy_from_slice(&(current_piece_index.load(Ordering::Relaxed) as u32).to_be_bytes());
                            payload[4..8].copy_from_slice(&begin_offset.to_be_bytes());
                            payload[8..12].copy_from_slice(&length.to_be_bytes());

                            stream.write_all(&length_bytes).await?;
                            stream.write_all(&message_id).await?;
                            stream.write_all(&payload).await?;

                            begin_offset += length;
                        }
                    }
                }
                Ok(())
            };

            let mut retries = 0;
            loop {
                let r = result().await;
                match r {
                    Ok(()) => break,
                    Err(e) => {
                        connected_peers.fetch_sub(1, Ordering::Relaxed);
                        warn!("<{ip}> disconnected: {e:?}");
                        if current_piece_index.load(Ordering::Relaxed) != usize::MAX {
                            pieces_left.lock().await.push(current_piece_index.load(Ordering::Relaxed));
                            current_piece_index.store(usize::MAX, Ordering::Relaxed);
                        }
                        if pieces_done.load(Ordering::Relaxed) == hashed_torrent_pieces.len() || retries == 3 {
                            info!("<{ip}> will not try to reconnect anymore");
                            break;
                        }
                        tokio::select! {
                            _ = disconnect_event.notified() => {
                                info!("<{ip}> will not try to reconnect anymore"); 
                                break
                            },
                            _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                                retries += 1;
                                info!("Attempting to reconnect to <{ip}>; Total reconnections :<{retries}>")
                            }
                        }
                    }
                }
            }

        }));
    }
}
