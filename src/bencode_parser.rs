use anyhow::{Result, anyhow};
use serde_json::{Map, Value};

// Strings are of the form "<len>:<string>"
// This returns the colon index and length of string respectively
fn decode_str(encoded: &[u8]) -> Result<(usize, usize)> {
    let colon_index = encoded.iter().position(|b| b == &b':').ok_or_else(|| {
        anyhow!(
            "No colon found in beginning of {}",
            String::from_utf8_lossy(encoded)
        )
    })?;
    return Ok((
        colon_index,
        str::from_utf8(&encoded[..colon_index])?.parse::<usize>()?,
    ));
}

// Numbers are of the form "i<number>e"
// This returns the number and length of the number in string form
// For example "120" has a length of 3
fn decode_number(encoded: &[u8]) -> Result<(i64, usize)> {
    let number_string = str::from_utf8(
        &encoded[1..encoded.iter().position(|b| b == &b'e').ok_or_else(|| {
            anyhow!(
                "No number found in beginning of {}",
                String::from_utf8_lossy(encoded)
            )
        })?],
    )?;
    return Ok((number_string.parse::<i64>()?, number_string.len()));
}

// Lists are of the form "l<item1><item2>...<itemN>e"
// This returns the decoded list and an optional string which is only Some() during recursion (it is the rest string)
fn decode_list(mut rest: &[u8]) -> Result<(Vec<Value>, Option<&[u8]>)> {
    let mut list: Vec<Value> = Vec::new();
    while !rest.is_empty() {
        let first_char = rest
            .get(0)
            .ok_or_else(|| anyhow!("bencode string is empty!"))?;
        if first_char.is_ascii_digit() {
            // string
            let (colon_idx, strlen) = decode_str(rest)?;
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx + strlen + 1..];
            list.push(string.into());
        } else if first_char == &b'i' {
            // number
            let (number, len) = decode_number(rest)?;
            rest = &rest[len + 2..];
            list.push(number.into());
        } else if first_char == &b'l' {
            // list
            rest = rest.strip_prefix(b"l").unwrap();
            let recursed = decode_list(rest)?;
            list.push(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == &b'd' {
            // dict
            rest = rest.strip_prefix(b"d").unwrap();
            let recursed = decode_dict(rest)?;
            list.push(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == &b'e' {
            // only gonna happen during recursion or if the bencode is faulty
            rest = rest.strip_prefix(b"e").unwrap();
            return Ok((list, Some(rest)));
        }
    }
    Ok((list, None))
}

// Dictionaries are of the form d<item1><key1><item2><key2>....<itemN><keyN>e
// This returns the decoded dictionary and an optional string which is only Some during recursion (it is the rest string)
// The official BitTorrent specification says that Dict keys are ALWAYS valid utf-8 byte strings
// Therefore I am taking the liberty of checking and converting byte strings to dict keys
fn decode_dict(mut rest: &[u8]) -> Result<(Map<String, Value>, Option<&[u8]>)> {
    let mut dict: Map<String, Value> = Map::new();
    let mut key: Option<String> = None;
   
    while !rest.is_empty() {
        let first_char = rest
            .get(0)
            .ok_or_else(|| anyhow!("bencode string is empty!"))?;
        if first_char.is_ascii_digit() {
            // string
            let (colon_idx, strlen) = decode_str(rest)?;
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx + strlen + 1..];
            // Check if there exists a dictionary key; if yes then this string is a Value
            if key.is_none() {
                key = Some(String::from_utf8(string.to_vec()).unwrap())
            } else {
                dict.insert(key.take().unwrap(), string.into());
            }
        } else if first_char == &b'i' {
            // number
            let (number, len) = decode_number(rest)?;
            rest = &rest[len + 2..];
            dict.insert(key.take().unwrap(), number.into());
        } else if first_char == &b'l' {
            // list
            rest = rest.strip_prefix(b"l").unwrap();
            let recursed = decode_list(rest)?;
            dict.insert(key.take().unwrap(), recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == &b'd' {
            // dict
            rest = rest.strip_prefix(b"d").unwrap();
            let recursed = decode_dict(rest)?;
            dict.insert(key.take().unwrap(), recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == &b'e' {
            // only gonna during recursion or if the bencode is faulty
            rest = rest.strip_prefix(b"e").unwrap();
            return Ok((dict, Some(rest)));
        }
    }

    Ok((dict, None))
}

pub fn decode_bencoded_value(encoded_value: &[u8]) -> Result<serde_json::Value> {
    let first_char = encoded_value
        .get(0)
        .ok_or_else(|| anyhow!("bencode string is empty!"))?;
    if first_char.is_ascii_digit() {
        // string
        let (colon_idx, strlen) = decode_str(&encoded_value)?;
        let string = &encoded_value[colon_idx + 1..colon_idx + 1 + strlen];
        return Ok(string.into());
    } else if first_char == &b'i' {
        // number
        return Ok(decode_number(&encoded_value)?.0.into());
    } else if first_char == &b'l' {
        // list
        let rest = encoded_value
            .strip_prefix(b"l")
            .and_then(|r| r.strip_suffix(b"e"))
            .ok_or_else(|| {
                anyhow!(
                    "Invalid bencode {}",
                    String::from_utf8_lossy(&encoded_value)
                )
            })?;
        return Ok(decode_list(rest)?.0.into());
    } else if first_char == &b'd' {
        // dictionary
        let rest = encoded_value
            .strip_prefix(b"d")
            .and_then(|r| r.strip_suffix(b"e"))
            .ok_or_else(|| {
                anyhow!(
                    "Invalid bencode {}",
                    String::from_utf8_lossy(&encoded_value)
                )
            })?;
        return Ok(decode_dict(rest)?.0.into());
    } else {
        panic!(
            "Unhandled encoded value: {}",
            String::from_utf8_lossy(&encoded_value)
        )
    }
}
