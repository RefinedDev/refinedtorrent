#[cfg(test)]
mod bencode_tests;
mod bencode_parser;

use anyhow::Result;
use reqwest;
use sha1::{Digest, Sha1, digest::array::ArrayN};
use std::fmt::Write;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn hash_bytes_to_percent_hex(hash: ArrayN<u8, 20>) -> Result<String> {
    let mut encode = String::new();
    for byte in hash {
        write!(encode, "%{:02x}", byte)?;
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
    let piece_length = &info["piece length"].as_int().unwrap();
    let pieces = get_pieces(*&info["pieces"].as_bytes().unwrap())?;
    
    // Converting the info sub_dict back to bencode byte array
    let mut info_bencode = Vec::with_capacity(234);
    info_bencode.push(b'd');
    for (k, v) in info.iter() {
        bencode_parser::encode(v, &mut info_bencode, Some(k));
    }
    info_bencode.push(b'e');

    // "announce" is the torrent link
    let mut url = String::from_utf8(treemap["announce"].as_bytes().unwrap().to_vec())?;
    // Converting the bencode to hash and inserting in link
    url.push_str(&format!(
        "?info_hash={}",
        hash_bytes_to_percent_hex(Sha1::digest(info_bencode.clone()))?
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

    let ips = get_peer_ips(decoded_dict["peers"].as_bytes().unwrap())?; // Extract <ip_addr:port> of peers
    let mut connections = Vec::with_capacity(ips.len());
    for i in 0..ips.len() {
        let info_hash = info_bencode.clone();
        let ip = ips[i].clone();
        connections.push(tokio::spawn(async move {
            let mut stream = TcpStream::connect(ip).await.unwrap();
            
            let mut h1 = [0u8; 68]; // To establish a connection we need to do a handshake
            h1[0] = 19;
            h1[1..20].copy_from_slice(b"BitTorrent protocol");
            h1[20..28].fill(0);
            h1[28..48].copy_from_slice(&Sha1::digest(info_hash.clone()));
            h1[48..68].copy_from_slice(b"hellomynameisgamer12");
            stream.write_all(&h1).await.unwrap();
            let mut h2 = [0u8; 68]; // The peer returns something similar in return
            stream.read_exact(&mut h2).await.unwrap();
            
            // Get 'bitfield' payload
            let mut length_bytes = [0u8;4];
            let mut message_id = [0u8;1];
            stream.read_exact(&mut length_bytes).await.unwrap();
            stream.read_exact(&mut message_id).await.unwrap(); // message_id for bitfield is 5
            let mut bitfield_payload_bytes = vec![0u8; (u32::from_be_bytes(length_bytes) - 1) as usize]; // Message takes 1 length
            stream.read_exact(&mut bitfield_payload_bytes).await.unwrap();

            // Send 'interested' Message
            let length_bytes = (1 as u32).to_be_bytes();
            let message_id: [u8; 1] = [2];
            stream.write_all(&length_bytes).await.unwrap();
            stream.write_all(&message_id).await.unwrap();

            // Get 'unchoke' message
            let mut length_bytes = [0u8;4];
            let mut message_id = [0u8;1];
            stream.read_exact(&mut length_bytes).await.unwrap();
            stream.read_exact(&mut message_id).await.unwrap(); // message id for unchoke is 1
        
            // Send 'request' message
            // let length_bytes = (13 as u32).to_be_bytes();
            // let message_id: [u8; 1] = [6];
            // stream.write_all(&length_bytes).await.unwrap();
            // stream.write_all(&message_id).await.unwrap();
            
        }));
    }
    
    for handle in connections {
        handle.await?;
    }
    
    Ok(())
}
