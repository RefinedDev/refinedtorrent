mod bencode_parser;
#[cfg(test)]
mod bencode_tests;
use anyhow::Result;

use crate::bencode_parser::BencodeType;

fn main() -> Result<()> { 
    let bytes = std::fs::read("sample.torrent")?;
    let decoded_value = bencode_parser::decode_bencoded_value(&bytes)?;
    // if let BencodeType::Dict(something) = decoded_value {
    //     if let BencodeType::Dict(something2) = &something["info"] {
    //         println!("{}", something2["pieces"])
    //     }
    // }
    println!("{}", decoded_value);
    Ok(())
}
