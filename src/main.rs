mod bencode_parser;
#[cfg(test)]
mod bencode_tests;
use anyhow::Result;
use sha1::{Digest, Sha1};

use crate::bencode_parser::BencodeType;

fn encode(value: &BencodeType, bencoded: &mut Vec<u8>, key: Option<&str>) {
    if let Some(k) = key {
        let len_colon_str = format!("{}:{}", k.len(), k);
        bencoded.extend_from_slice(len_colon_str.as_bytes());
    }
    match value {
        BencodeType::String(str_bytes) => {
            bencoded.extend_from_slice(str_bytes.len().to_string().as_bytes());
            bencoded.push(b':');
            bencoded.extend_from_slice(*str_bytes);
        }
        BencodeType::Integer(int) => {
            bencoded.push(b'i');
            bencoded.extend_from_slice(int.to_string().as_bytes());
            bencoded.push(b'e');
        }
        BencodeType::List(list) => {
            bencoded.push(b'l');
            for item in list.iter() {
                encode(item, bencoded, None);
            }
            bencoded.push(b'e');
        }
        BencodeType::Dict(dict) => {
            bencoded.push(b'd');
            for (k, v) in dict.iter() {
                encode(v, bencoded, Some(k));
            }
            bencoded.push(b'e');
        }
    }
}

fn main() -> Result<()> {
    let bytes = std::fs::read("sample.torrent")?;
    let decoded_value = bencode_parser::decode_bencoded_value(&bytes)?;

    if let BencodeType::Dict(treemap) = decoded_value {
        if let BencodeType::Dict(info) = &treemap["info"] {
            let mut bencode = Vec::with_capacity(234);
            bencode.push(b'd');
            for (k, v) in info.iter() {
                encode(v, &mut bencode, Some(k));
            }
            bencode.push(b'e');
            let hash = Sha1::digest(bencode);
            let mut hash_str = String::new();
            for byte in hash {
                hash_str.push_str(&format!("{:02x}", byte));
            }
            assert_eq!(hash_str, "d69f91e6b2ae4c542468d1073a71d4ea13879a7f");
            
        }
    }

    Ok(())
}
