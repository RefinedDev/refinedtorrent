mod bencode_parser;
#[cfg(test)]
mod bencode_tests;

use std::env;
use anyhow::Result;

fn main() -> Result<()> { 
    // let args: Vec<String> = env::args().collect();
    // let command = &args[1];

    // if command == "decode" {
    //     let encoded_value = &args[2];
    //     let decoded_value =  bencode_parser::decode_bencoded_value(encoded_value)?;
    //     println!("{}", decoded_value.to_string());
    // } else {
    //     println!("unknown command: {}", args[1])
    // }

    let bytes = std::fs::read("sample.torrent")?;
    // println!("{}", String::from_utf8_lossy(&bytes));
    let decoded_value = bencode_parser::decode_bencoded_value(&bytes);
    Ok(())
}
