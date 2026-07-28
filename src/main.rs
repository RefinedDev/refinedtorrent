mod bencode_parser;
#[cfg(test)]
mod bencode_tests;

use anyhow::Result;
use reqwest;
use sha1::{Digest, Sha1, digest::array::ArrayN};
use std::fmt::Write;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

fn sha1_bytes_to_hex(hash: ArrayN<u8, 20>, percent: bool) -> Result<String> {
    let mut encode = String::new();
    for byte in hash {
        if percent {
            write!(encode, "%{:02x}", byte)?;
        } else {
            write!(encode, "{:02x}", byte)?;
        }
    }
    Ok(encode)
}

fn hash_bytes_to_hex(hash: &[u8]) -> Result<String> {
    let mut encode = String::new();
    for byte in hash {
        write!(encode, "{:02x}", byte)?;
    }
    Ok(encode)
}

fn get_peer_ips(bytes: &[u8]) -> Result<Vec<String>> {
    let end = bytes.len() / 6;
    let mut ips = Vec::with_capacity(end);
    for i in 0..end {
        let slice = &bytes[6 * i..6 * (i + 1)];
        let mut ip = slice[0..4]
            .iter()
            .map(|int| int.to_string())
            .collect::<Vec<String>>()
            .join(".");
        let port = u16::from_be_bytes([slice[4], slice[5]]);
        write!(ip, ":{}", port)?;
        ips.push(ip);
    }
    Ok(ips)
}

fn get_pieces(bytes: &[u8]) -> Result<Vec<String>> {
    let end = bytes.len() / 20;
    let mut pieces = Vec::with_capacity(end);
    for i in 0..end {
        let slice = &bytes[20 * i..20 * (i + 1)];
        pieces.push(hash_bytes_to_hex(slice)?);
    }
    Ok(pieces)
}

#[tokio::main]
async fn main() -> Result<()> {
    let bytes = std::fs::read("sample.torrent")?;
    let decoded_value = bencode_parser::decode(&bytes)?;

    let treemap = decoded_value.as_dict().unwrap(); // Get the decoded value in a dictionary form
    let info = treemap["info"].as_dict().unwrap(); // Get the info sub-dict

    // Converting the info sub_dict back to bencode byte array
    let mut info_bencode = Vec::with_capacity(234);
    info_bencode.push(b'd');
    for (k, v) in info.iter() {
        bencode_parser::encode(v, &mut info_bencode, Some(k));
    }
    info_bencode.push(b'e');

    let info_bencode = Arc::new(info_bencode);
    // "announce" is the torrent link
    let mut url = String::from_utf8(treemap["announce"].as_bytes().unwrap().to_vec())?;
    // Converting the bencode to hash and inserting in link
    url.push_str(&format!(
        "?info_hash={}",
        sha1_bytes_to_hex(Sha1::digest(&*info_bencode), true)?
    ));
    // The rest of the parameters; Not adding the info_hash along with these because parse_with_params double encodes
    let params = [
        ("peer_id", "hellomynameisgamer12".to_string()),
        ("port", 6881.to_string()),
        ("uploaded", 0.to_string()),
        ("downloaded", 0.to_string()),
        ("left", info["length"].as_int().unwrap().to_string()),
        ("compact", 1.to_string()),
    ];
    let url = reqwest::Url::parse_with_params(&url, &params)?;
    let response = reqwest::get(url).await?.bytes().await?;
    let decoded = bencode_parser::decode(&response)?;
    let decoded_dict = decoded.as_dict().unwrap();

    let torrent_pieces_hash = Arc::new(get_pieces(*&info["pieces"].as_bytes().unwrap())?);
    let piece_length = &info["piece length"].as_int().unwrap();
    let total_length = &info["length"].as_int().unwrap();
    let file_name = str::from_utf8(&info["name"].as_bytes().unwrap()).unwrap();

    let ips = get_peer_ips(decoded_dict["peers"].as_bytes().unwrap())?; // Extract <ip_addr:port> of peers
    let pieces_hashes = Arc::new(Mutex::new(Vec::with_capacity(torrent_pieces_hash.len())));
    let pieces_bytes = Arc::new(Mutex::new(Vec::with_capacity(torrent_pieces_hash.len())));

    let mut connections = Vec::with_capacity(ips.len());
    {
        // TODO: IMPLEMENT MULTI-PEER
        let info_hash = Arc::clone(&info_bencode);
        let torrent_pieces_hash = Arc::clone(&torrent_pieces_hash);
        let pieces_hashes = Arc::clone(&pieces_hashes);
        let pieces_bytes = Arc::clone(&pieces_bytes);

        let ip = ips[0].clone();
        let total_length = total_length.clone() as u32;
        let piece_length = piece_length.clone() as u32;

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
            for i in 0..torrent_pieces_hash.len() {
                let this_piece_length = if i == torrent_pieces_hash.len() - 1 {
                    total_length - piece_length as u32 * (torrent_pieces_hash.len() as u32 - 1)
                } else {
                    piece_length as u32
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
            for i in 0..torrent_pieces_hash.len() {
                let this_piece_length = if i == torrent_pieces_hash.len() - 1 {
                    total_length - piece_length as u32 * (torrent_pieces_hash.len() as u32 - 1)
                } else {
                    piece_length as u32
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
                pieces_hashes
                    .lock()
                    .await
                    .push(sha1_bytes_to_hex(Sha1::digest(&piece_buffer), false).unwrap());
                pieces_bytes.lock().await.extend(piece_buffer);
            }
        }));
    }

    for handle in connections {
        handle.await?;
    }

    assert_eq!(&*torrent_pieces_hash, &*pieces_hashes.lock().await); // TODO: IF NOT MATCH THEN RETRY WITH ANOTHER PEER?
    std::fs::write(file_name, &*pieces_bytes.lock().await).unwrap();

    Ok(())
}
