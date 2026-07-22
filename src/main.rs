mod bencode_parser;
#[cfg(test)]
mod bencode_tests;
use anyhow::Result;
use reqwest;
use sha1::{Digest, Sha1, digest::array::ArrayN};
use std::fmt::Write;

fn hash_bytes_to_percent_hex(hash: ArrayN<u8, 20>) -> Result<String> {
    let mut encode = String::new();
    for byte in hash {
        write!(encode, "%{:02x}", byte)?;
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

fn main() -> Result<()> {
    let bytes = std::fs::read("sample.torrent")?;
    let decoded_value = bencode_parser::decode_bencoded_value(&bytes)?;

    let treemap = decoded_value.as_dict().unwrap(); // Get the decoded value in a dictionary form
    let info = treemap["info"].as_dict().unwrap(); // Get the info sub-dict
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
        hash_bytes_to_percent_hex(Sha1::digest(info_bencode))?
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
    let response = reqwest::blocking::get(url)?.bytes()?;
    let decoded = bencode_parser::decode_bencoded_value(&response)?;
    let decoded_dict = decoded.as_dict().unwrap();
    let ips = get_peer_ips(decoded_dict["peers"].as_bytes().unwrap())?;
    println!("{:?}", ips);
    Ok(())
}
