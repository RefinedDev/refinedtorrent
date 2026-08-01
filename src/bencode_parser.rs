use anyhow::{Context, Result, anyhow};
use std::collections::BTreeMap;

type Dict<'a> = BTreeMap<String, BencodeType<'a>>;

#[derive(Debug)]
pub enum BencodeType<'a> {
    String(&'a [u8]),
    Integer(i64),
    List(Vec<BencodeType<'a>>),
    Dict(Dict<'a>),
}

impl<'a> BencodeType<'a> {
    pub fn as_bytes(&self) -> Option<&'a [u8]> {
        match self {
            BencodeType::String(arr) => Some(*arr),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            BencodeType::Integer(int) => Some(*int),
            _ => None,
        }
    }

    pub fn as_dict(&self) -> Option<&BTreeMap<String, BencodeType<'a>>> {
        match self {
            BencodeType::Dict(tree) => Some(tree),
            _ => None,
        }
    }
}

impl<'a> std::fmt::Display for BencodeType<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BencodeType::String(arr) => match String::from_utf8(arr.to_vec()) {
                Ok(something) => write!(f, "{}", something),
                Err(_) => write!(f, "{:?}", arr),
            },
            BencodeType::Integer(int) => write!(f, "{}", int),
            BencodeType::List(list) => {
                write!(f, "[")?;
                for (i, item) in list.iter().enumerate() {
                    if i != 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{item}")?; // Recurses if nested
                }
                write!(f, "]")
            }
            BencodeType::Dict(dict) => {
                write!(f, "{{")?;
                for (i, (k, v)) in dict.iter().enumerate() {
                    if i != 0 {
                        write!(f, ",")?;
                    }
                    write!(f, "{}:{}", k, v)?; // Recurses if nested
                }
                write!(f, "}}")
            }
        }
    }
}

// Strings are of the form "<len>:<string>"
// This returns the colon index and length of string respectively
fn decode_str(encoded: &[u8]) -> Result<(usize, usize)> {
    let colon_index = encoded.iter().position(|b| b == &b':').ok_or_else(|| {
        anyhow!(
            "No colon found in beginning of {}",
            String::from_utf8_lossy(encoded)
        )
    })?;
    Ok((
        colon_index,
        str::from_utf8(&encoded[..colon_index])?.parse::<usize>()?,
    ))
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
    Ok((number_string.parse::<i64>()?, number_string.len()))
}

// Lists are of the form "l<item1><item2>...<itemN>e"
// This returns the decoded list and an optional string which is only Some() during recursion (it is the rest of the rest string after decoding the nested list)
fn decode_list<'a>(mut rest: &'a [u8]) -> Result<(Vec<BencodeType<'a>>, Option<&'a [u8]>)> {
    let mut list: Vec<BencodeType> = Vec::new();
    while !rest.is_empty() {
        let first_char = rest.first()
            .ok_or_else(|| anyhow!("bencode string is empty!"))?;
        if first_char.is_ascii_digit() {
            // string
            let (colon_idx, strlen) = decode_str(rest)?;
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx + strlen + 1..];
            list.push(BencodeType::String(string));
        } else if first_char == &b'i' {
            // number
            let (number, len) = decode_number(rest)?;
            rest = &rest[len + 2..]; // +2 to account for i and e
            list.push(BencodeType::Integer(number));
        } else if first_char == &b'l' {
            // list
            rest = rest
                .strip_prefix(b"l")
                .context("slice did not begin with 'l'")?;
            let recursed = decode_list(rest)?;
            list.push(BencodeType::List(recursed.0));
            rest = recursed.1.unwrap()
        } else if first_char == &b'd' {
            // dict
            rest = rest
                .strip_prefix(b"d")
                .context("slice did not begin with 'd'")?;
            let recursed = decode_dict(rest)?;
            list.push(BencodeType::Dict(recursed.0));
            rest = recursed.1.unwrap()
        } else if first_char == &b'e' {
            // only gonna happen during recursion or if the bencode is faulty
            rest = rest
                .strip_prefix(b"e")
                .context("slice did not begin with 'e'")?;
            return Ok((list, Some(rest)));
        }
    }
    Ok((list, None))
}

// Dictionaries are of the form d<item1><key1><item2><key2>....<itemN><keyN>e
// This returns the decoded dictionary and an optional string which is only Some during recursion (it is the rest of the rest string after decoding the nested dictionary)
// The official BitTorrent specification says that Dict keys are ALWAYS valid utf-8 byte strings
// Therefore I am taking the liberty of checking and converting byte strings to dict keys
fn decode_dict<'a>(
    mut rest: &'a [u8],
) -> Result<(Dict<'a>, Option<&'a [u8]>)> {
    let mut dict: BTreeMap<String, BencodeType> = BTreeMap::new();
    let mut key: Option<String> = None;

    while !rest.is_empty() {
        let first_char = rest.first()
            .ok_or_else(|| anyhow!("bencode string is empty!"))?;
        if first_char.is_ascii_digit() {
            // string
            let (colon_idx, strlen) = decode_str(rest)?;
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx + strlen + 1..];
            // Check if there exists a dictionary key; if yes then this string is a Value
            if key.is_none() {
                key = Some(String::from_utf8(string.to_vec()).context("key is not valid utf-8")?)
            } else {
                dict.insert(key.take().unwrap(), BencodeType::String(string));
            }
        } else if first_char == &b'i' {
            // number
            let (number, len) = decode_number(rest)?;
            rest = &rest[len + 2..];
            dict.insert(key.take().unwrap(), BencodeType::Integer(number));
        } else if first_char == &b'l' {
            // list
            rest = rest
                .strip_prefix(b"l")
                .context("slice did not begin with 'l'")?;
            let recursed = decode_list(rest)?;
            dict.insert(key.take().unwrap(), BencodeType::List(recursed.0));
            rest = recursed.1.unwrap()
        } else if first_char == &b'd' {
            // dict
            rest = rest
                .strip_prefix(b"d")
                .context("slice did not begin with 'd'")?;
            let recursed = decode_dict(rest)?;
            dict.insert(key.take().unwrap(), BencodeType::Dict(recursed.0));
            rest = recursed.1.unwrap()
        } else if first_char == &b'e' {
            // only gonna during recursion or if the bencode is faulty
            rest = rest
                .strip_prefix(b"e")
                .context("slice did not begin with 'e'")?;
            return Ok((dict, Some(rest)));
        }
    }

    Ok((dict, None))
}

pub fn decode<'a>(encoded_value: &'a [u8]) -> Result<BencodeType<'a>> {
    let first_char = encoded_value.first()
        .ok_or_else(|| anyhow!("bencode string is empty!"))?;
    if first_char.is_ascii_digit() {
        // string
        let (colon_idx, strlen) = decode_str(encoded_value)?;
        let string = &encoded_value[colon_idx + 1..colon_idx + 1 + strlen];
        Ok(BencodeType::String(string))
    } else if first_char == &b'i' {
        // number
        Ok(BencodeType::Integer(decode_number(encoded_value)?.0))
    } else if first_char == &b'l' {
        // list
        let rest = encoded_value
            .strip_prefix(b"l")
            .and_then(|r| r.strip_suffix(b"e"))
            .ok_or_else(|| {
                anyhow!(
                    "Invalid bencode {}",
                    String::from_utf8_lossy(encoded_value)
                )
            })?;
        return Ok(BencodeType::List(decode_list(rest)?.0));
    } else if first_char == &b'd' {
        // dictionary
        let rest = encoded_value
            .strip_prefix(b"d")
            .and_then(|r| r.strip_suffix(b"e"))
            .ok_or_else(|| {
                anyhow!(
                    "Invalid bencode {}",
                    String::from_utf8_lossy(encoded_value)
                )
            })?;
        return Ok(BencodeType::Dict(decode_dict(rest)?.0));
    } else {
        panic!(
            "Unhandled encoded value: {}",
            String::from_utf8_lossy(encoded_value)
        )
    }
}

pub fn encode(value: &BencodeType, bencoded: &mut Vec<u8>, key: Option<&str>) {
    if let Some(k) = key {
        let len_colon_str = format!("{}:{}", k.len(), k);
        bencoded.extend_from_slice(len_colon_str.as_bytes());
    }
    match value {
        BencodeType::String(str_bytes) => {
            bencoded.extend_from_slice(str_bytes.len().to_string().as_bytes());
            bencoded.push(b':');
            bencoded.extend_from_slice(str_bytes);
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
