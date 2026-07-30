use crate::bencode_parser::{self, BencodeType};
use anyhow::Result;
use sha1::{Digest, Sha1, digest::array::ArrayN};
use std::fmt::Write;

pub fn generate_key() -> String {
    (0..8)
        .map(|_| format!("{:x}", rand::random_range(0..100)))
        .collect()
}

pub fn sha1_bytes_to_hex(hash: ArrayN<u8, 20>, percent: bool) -> Result<String> {
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

pub fn hash_bytes_to_hex(hash: &[u8]) -> Result<String> {
    let mut encode = String::new();
    for byte in hash {
        write!(encode, "{:02x}", byte)?;
    }
    Ok(encode)
}

pub struct Torrent<'a> {
    pub data: BencodeType<'a>,
    peer_id: &'a str,
    port: &'a str,

    pub piece_length: i64,
    pub total_length: i64,
    pub save_file_name: &'a str,
}

impl<'a> Torrent<'a> {
    pub fn new(data: BencodeType<'a>, peer_id: &'a str, port: &'a str) -> Self {
        let data_dict = data.as_dict().unwrap();
        let info = data_dict["info"].as_dict().unwrap();

        Torrent {
            piece_length: info["piece length"].as_int().unwrap(),
            total_length: info["length"].as_int().unwrap(),
            save_file_name: str::from_utf8(info["name"].as_bytes().unwrap()).unwrap(),

            data: data,
            peer_id,
            port,
        }
    }

    pub fn get_info_bencode(&self) -> Vec<u8> {
        let data = self.data.as_dict().unwrap();
        let info = data["info"].as_dict().unwrap();

        let mut info_bencode = Vec::with_capacity(234);
        info_bencode.push(b'd');
        for (k, v) in info.iter() {
            bencode_parser::encode(v, &mut info_bencode, Some(k));
        }
        info_bencode.push(b'e');
        info_bencode
    }

    pub fn get_piece_hashes(&self) -> Result<Vec<String>> {
        let data = self.data.as_dict().unwrap();
        let info = data["info"].as_dict().unwrap();
        let bytes = info["pieces"].as_bytes().unwrap();
        let end = bytes.len() / 20;
        let mut pieces = Vec::with_capacity(end);
        for i in 0..end {
            let slice = &bytes[20 * i..20 * (i + 1)];
            pieces.push(hash_bytes_to_hex(slice)?);
        }
        Ok(pieces)
    }

    pub async fn get_peer_ips(&self, info_bencode: &Vec<u8>) -> Result<Vec<String>> {
        let data = self.data.as_dict().unwrap();
        let info = data["info"].as_dict().unwrap();

        // "announce" is the torrent link (ONLY HTTP(S) IS SUPPORTED NOT UDP)
        let mut url = String::from_utf8(data["announce"].as_bytes().unwrap().to_vec())?;
        url.push_str(&format!(
            "?info_hash={}",
            sha1_bytes_to_hex(Sha1::digest(info_bencode), true)?
        ));
        // The rest of the parameters; Not adding the info_hash along with these because parse_with_params double encodes
        let params = [
            ("peer_id", self.peer_id),
            ("port", self.port),
            ("uploaded", "0"),
            ("downloaded", "0"),
            ("left", &info["length"].as_int().unwrap().to_string()),
            ("compact", "1"),
            ("event", "started"),
            ("key", &generate_key())
        ];
        let url = reqwest::Url::parse_with_params(&url, &params)?;
        
        let response = reqwest::Client::new()
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36")
            .send()
            .await?
            .bytes()
            .await?;

        let decoded = bencode_parser::decode(&response)?;
        let decoded_dict = decoded.as_dict().unwrap();

        let bytes = decoded_dict["peers"].as_bytes().unwrap();
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
}
