use serde_json::{Map, Value};
use anyhow::{Result, anyhow};

// Strings are of the form "<len>:<string>"
// This returns the colon index and length respectively
fn decode_str(encoded: &str) -> Result<(usize, usize)> {
    let colon_index = encoded.find(':').ok_or_else(|| anyhow!("No colon found in beginning of {}", encoded))?;
    return Ok((colon_index, encoded[..colon_index].parse::<usize>()?))
}

// Numbers are of the form "i<number>e"
// This returns the number and length of the number in string form 
// For example "120" has a length of 3
fn decode_number(encoded: &str) -> Result<(i64, usize)> {
    let number_string = &encoded[1..encoded.find("e").ok_or_else(|| anyhow!("No number found in beginning of {}", encoded))?];
    return Ok((number_string.parse::<i64>()?, number_string.len()));
}

// Lists are of the form "l<item1><item2>...<itemN>e"
// This returns the decoded list and an optional string which is only Some() during recursion (it is the rest string)
fn decode_list(mut rest: &str) -> Result<(Vec<Value>, Option<&str>)> {
    let mut list: Vec<Value> = Vec::new();
    while !rest.is_empty() {
        let first_char = rest.chars().next().ok_or_else(|| anyhow!("bencode string is empty!"))?;
        if first_char.is_ascii_digit() { // string
            let (colon_idx, strlen) = decode_str(rest)?;
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx+strlen+1..];
            list.push(string.into());
        } else if first_char == 'i' { // number
            let (number, len) = decode_number(rest)?;
            rest = &rest[len+2..];
            list.push(number.into());
        } else if first_char == 'l' { // list
            rest = rest.strip_prefix("l").unwrap();
            let recursed = decode_list(rest)?;
            list.push(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'd' { // dict
            rest = rest.strip_prefix("d").unwrap();
            let recursed = decode_dict(rest)?;
            list.push(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'e' { // only gonna happen during recursion or if the bencode is faulty
            rest = rest.strip_prefix("e").unwrap();
            return Ok((list, Some(rest)))
        }
    }
    Ok((list, None))
}

// Dictionaries are of the form d<item1><key1><item2><key2>....<itemN><keyN>e
// This returns the decoded dictionary and an optional string which is only Some during recursion (it is the rest string)
fn decode_dict(mut rest: &str) -> Result<(Map<String,Value>, Option<&str>)> {
    let mut dict: Map<String, Value> = Map::new();
    let mut key: Option<String> = None;
    let mut insert_if_can = |item: Value| {
        if key.is_none() {
            key = Some(item.as_str().unwrap().to_owned())
        } else {
            dict.insert(key.take().unwrap(), item);
        }
    };

    while !rest.is_empty() {
        let first_char = rest.chars().next().ok_or_else(|| anyhow!("bencode string is empty!"))?;
        if first_char.is_ascii_digit() { // string
            let (colon_idx, strlen) = decode_str(rest)?;
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx+strlen+1..];
            insert_if_can(string.into());
        } else if first_char == 'i' { // number
            let (number, len) = decode_number(rest)?;
            rest = &rest[len+2..];
            insert_if_can(number.into());
        } else if first_char == 'l' { // list
            rest = rest.strip_prefix("l").unwrap();
            let recursed = decode_list(rest)?;
            insert_if_can(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'd' { // dict
            rest = rest.strip_prefix("d").unwrap();
            let recursed = decode_dict(rest)?;
            insert_if_can(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'e' { // only gonna during recursion or if the bencode is faulty
            rest = rest.strip_prefix("e").unwrap();
            return Ok((dict, Some(rest)))
        }
    }

    Ok((dict, None))
}

pub fn decode_bencoded_value(encoded_value: &str) -> Result<serde_json::Value> {
    let first_char = encoded_value.chars().next().ok_or_else(|| anyhow!("bencode string is empty!"))?;
    if first_char.is_ascii_digit() { // string
        let (colon_idx, strlen) = decode_str(encoded_value)?;
        let string = &encoded_value[colon_idx + 1..colon_idx + 1 + strlen];
        return Ok(string.into())
    } else if first_char == 'i' { // number
        return Ok(decode_number(encoded_value)?.0.into())
    } else if first_char == 'l' { // list
        let rest = encoded_value.strip_prefix("l").and_then(|r| r.strip_suffix("e")).ok_or_else(|| anyhow!("Invalid bencode {}", encoded_value))?;
        return Ok(decode_list(rest)?.0.into())
    } else if first_char == 'd' { // dictionary
        let rest = encoded_value.strip_prefix("d").and_then(|r| r.strip_suffix("e")).ok_or_else(|| anyhow!("Invalid bencode {}", encoded_value))?;
        return Ok(decode_dict(rest)?.0.into())
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}