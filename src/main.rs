use std::env;

use serde_json::{Map, Value};

fn decode_str(encoded: &str) -> (usize, usize) {
    let colon_index = encoded.find(':').unwrap();
    return (colon_index, encoded[..colon_index].parse::<usize>().unwrap())
}

fn decode_number(encoded: &str) -> (i64, usize) {
    let number_string = &encoded[1..encoded.find("e").unwrap()];
    return (number_string.parse::<i64>().unwrap(), number_string.len());
}

fn decode_list(mut rest: &str) -> (Vec<Value>, Option<&str>) {
    let mut list: Vec<Value> = Vec::new();
    while !rest.is_empty() {
        let first_char = rest.chars().next().unwrap();
        if first_char.is_ascii_digit() { // string
            let (colon_idx, strlen) = decode_str(rest);
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx+strlen+1..];
            list.push(string.into());
        } else if first_char == 'i' { // number
            let (number, len) = decode_number(rest);
            rest = &rest[len+2..];
            list.push(number.into());
        } else if first_char == 'l' { // list
            rest = rest.strip_prefix("l").unwrap();
            let recursed = decode_list(rest);
            list.push(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'd' { // dict
            rest = rest.strip_prefix("d").unwrap();
            let recursed = decode_dict(rest);
            list.push(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'e' { // only gonna during recursion or if the bencode is faulty
            rest = rest.strip_prefix("e").unwrap();
            return (list, Some(rest))
        }
    }
    (list, None)
}

fn decode_dict(mut rest: &str) -> (Map<String,Value>, Option<&str>) {
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
        let first_char = rest.chars().next().unwrap();
        if first_char.is_ascii_digit() { // string
            let (colon_idx, strlen) = decode_str(rest);
            let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
            rest = &rest[colon_idx+strlen+1..];
            insert_if_can(string.into());
        } else if first_char == 'i' { // number
            let (number, len) = decode_number(rest);
            rest = &rest[len+2..];
            insert_if_can(number.into());
        } else if first_char == 'l' { // list
            rest = rest.strip_prefix("l").unwrap();
            let recursed = decode_list(rest);
            insert_if_can(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'd' { // dict
            rest = rest.strip_prefix("d").unwrap();
            let recursed = decode_dict(rest);
            insert_if_can(recursed.0.into());
            rest = recursed.1.unwrap()
        } else if first_char == 'e' { // only gonna during recursion or if the bencode is faulty
            rest = rest.strip_prefix("e").unwrap();
            return (dict, Some(rest))
        }
    }

    (dict, None)
}

fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    let first_char = encoded_value.chars().next().unwrap();
    if first_char.is_ascii_digit() { // string
        let (colon_idx, strlen) = decode_str(encoded_value);
        let string = &encoded_value[colon_idx + 1..colon_idx + 1 + strlen];
        return string.into()
    } else if first_char == 'i' { // number
        return decode_number(encoded_value).0.into()
    } else if first_char == 'l' { // list
        let rest = encoded_value.strip_prefix("l").and_then(|r| r.strip_suffix("e")).unwrap();
        return decode_list(rest).0.into()
    } else if first_char == 'd' { // dictionary
        let rest = encoded_value.strip_prefix("d").and_then(|r| r.strip_suffix("e")).unwrap();
        return decode_dict(rest).0.into()
    } else {
        panic!("Unhandled encoded value: {}", encoded_value)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let command = &args[1];

    if command == "decode" {
        let encoded_value = &args[2];
        let decoded_value = decode_bencoded_value(encoded_value);
        println!("{}", decoded_value.to_string());
    } else {
        println!("unknown command: {}", args[1])
    }
}
