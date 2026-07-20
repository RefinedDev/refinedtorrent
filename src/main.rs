mod bencode_parser;
#[cfg(test)]
mod bencode_tests;
use anyhow::Result;

fn main() -> Result<()> { 
    let bytes = std::fs::read("sample.torrent")?;
    // println!("{}", String::from_utf8_lossy(&bytes));
    let decoded_value = bencode_parser::decode_bencoded_value(&bytes);
    Ok(())
}
