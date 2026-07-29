mod bencode_parser;
#[cfg(test)]
mod bencode_tests;
mod peer;
mod torrent;

use anyhow::Result;
use sha1::{Digest, Sha1};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use torrent::Torrent;

#[tokio::main]
async fn main() -> Result<()> {
    let bytes = std::fs::read("sample.torrent")?;
    let torrent = Torrent::new(bencode_parser::decode(&bytes)?, "hellomynameisgamer12", "6881");
    
    let info_bencode = Arc::new(torrent.get_info_bencode());
    let ips = torrent.get_peer_ips(&info_bencode).await?;

    let hashed_torrent_pieces = Arc::new(torrent.get_piece_hashes()?);
    let hashed_obtained_pieces = Arc::new(Mutex::new(Vec::with_capacity(hashed_torrent_pieces.len())));
    let obtained_pieces_bytes = Arc::new(Mutex::new(Vec::with_capacity(hashed_torrent_pieces.len())));

    let mut connections = Vec::with_capacity(ips.len());
    {
        // TODO: IMPLEMENT MULTI-PEER
        let info_hash = Arc::clone(&info_bencode);
        let hashed_torrent_pieces = Arc::clone(&hashed_torrent_pieces);
        let hashed_obtained_pieces = Arc::clone(&hashed_obtained_pieces);
        let obtained_pieces_bytes = Arc::clone(&obtained_pieces_bytes);

        let ip = ips[0].clone();
        let total_length = torrent.total_length.clone() as u32;
        let piece_length = torrent.piece_length.clone() as u32;

        connections.push(tokio::spawn(async move {
            let mut stream = TcpStream::connect(ip).await.unwrap();

            let mut h1 = [0u8; 68]; // To establish a connection we need to do a handshake
            h1[0] = 19;
            h1[1..20].copy_from_slice(b"BitTorrent protocol");
            h1[20..28].fill(0);
            h1[28..48].copy_from_slice(&Sha1::digest(&*info_hash));
            h1[48..68].copy_from_slice(b"hellomynameisgamer12");
            stream.write_all(&h1).await.unwrap();
            let mut h2 = [0u8; 68]; // The peer returns something similar in return
            stream.read_exact(&mut h2).await.unwrap();

            // Get 'bitfield' payload
            let mut length_bytes = [0u8; 4];
            let mut message_id = [0u8; 1];
            stream.read_exact(&mut length_bytes).await.unwrap();
            stream.read_exact(&mut message_id).await.unwrap(); // message_id for bitfield is 5
            let mut bitfield_payload_bytes =
                vec![0u8; (u32::from_be_bytes(length_bytes) - 1) as usize]; // Message takes 1 length
            stream
                .read_exact(&mut bitfield_payload_bytes)
                .await
                .unwrap();

            // Send 'interested' Message
            let length_bytes = (1 as u32).to_be_bytes();
            let message_id: [u8; 1] = [2];
            stream.write_all(&length_bytes).await.unwrap();
            stream.write_all(&message_id).await.unwrap();

            // Get 'unchoke' message
            let mut length_bytes = [0u8; 4];
            let mut message_id = [0u8; 1];
            stream.read_exact(&mut length_bytes).await.unwrap();
            stream.read_exact(&mut message_id).await.unwrap(); // message id for unchoke is 1

            // Send 'request' message
            for i in 0..hashed_torrent_pieces.len() {
                let this_piece_length = if i == hashed_torrent_pieces.len() - 1 {
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

                    payload[0..4].copy_from_slice(&(i as u32).to_be_bytes());
                    payload[4..8].copy_from_slice(&begin_offset.to_be_bytes());
                    payload[8..12].copy_from_slice(&length.to_be_bytes());

                    stream.write_all(&length_bytes).await.unwrap();
                    stream.write_all(&message_id).await.unwrap();
                    stream.write_all(&payload).await.unwrap();

                    begin_offset += length;
                }
            }

            // Get 'piece' message and stich all pieces together
            for i in 0..hashed_torrent_pieces.len() {
                let this_piece_length = if i == hashed_torrent_pieces.len() - 1 {
                    total_length - piece_length * (hashed_torrent_pieces.len() as u32 - 1)
                } else {
                    piece_length
                };
                let mut block_length = 0;
                let mut piece_buffer = vec![0u8; this_piece_length as usize];
                while block_length < this_piece_length {
                    let mut length_bytes = [0u8; 4];
                    let mut message_id = [0u8; 1];
                    stream.read_exact(&mut length_bytes).await.unwrap();
                    if u32::from_be_bytes(length_bytes) == 0 {
                        continue;
                    }
                    stream.read_exact(&mut message_id).await.unwrap(); // message_id for piece is 7
                    let mut payload = vec![0u8; (u32::from_be_bytes(length_bytes) - 1) as usize]; // Message takes 1 length
                    stream.read_exact(&mut payload).await.unwrap();
                    match message_id {
                        [7] => {
                            let _index = u32::from_be_bytes(payload[0..4].try_into().unwrap());
                            let begin = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                            let block = &payload[8..];
                            piece_buffer[begin as usize..begin as usize + block.len()]
                                .copy_from_slice(block);
                            block_length += block.len() as u32;
                        }
                        _ => (),
                    }
                }
                hashed_obtained_pieces
                    .lock()
                    .await
                    .push(torrent::sha1_bytes_to_hex(Sha1::digest(&piece_buffer), false).unwrap());
                obtained_pieces_bytes.lock().await.extend(piece_buffer);
            }
        }));
    }

    for handle in connections {
        handle.await?;
    }

    assert_eq!(&*hashed_torrent_pieces, &*hashed_obtained_pieces.lock().await); // TODO: IF NOT MATCH THEN RETRY WITH ANOTHER PEER?
    std::fs::write(torrent.save_file_name, &*obtained_pieces_bytes.lock().await).unwrap();

    Ok(())
}
