use std::env;

use serde_json::{Map, Value};

fn get_str_data(encoded: &str) -> (usize, usize) {
    let colon_index = encoded.find(':').unwrap();
    return (colon_index, encoded[..colon_index].parse::<usize>().unwrap())
}

fn get_number_data(encoded: &str) -> (i64, usize) {
    let number_string = &encoded[1..encoded.find("e").unwrap()];
    return (number_string.parse::<i64>().unwrap(), number_string.len());
}

fn decode_bencoded_value(encoded_value: &str) -> serde_json::Value {
    let first_char = encoded_value.chars().next().unwrap();
    if first_char.is_ascii_digit() { // string
        let (colon_idx, strlen) = get_str_data(encoded_value);
        let string = &encoded_value[colon_idx + 1..colon_idx + 1 + strlen];
        return string.into()
    } else if first_char.is_ascii_alphabetic() { // can be a number or a list or dictionary
        if first_char == 'i' { // number
            return get_number_data(encoded_value).0.into()
        } else if first_char == 'l' { // list
            let mut rest = encoded_value.strip_prefix("l").and_then(|r| r.strip_suffix("e")).unwrap();
            let mut list: Vec<serde_json::Value> = Vec::new();
            while !rest.is_empty() {
                if rest.chars().next().unwrap().is_ascii_digit() {
                    let (colon_idx, strlen) = get_str_data(rest);
                    let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
                    rest = &rest[colon_idx+strlen+1..];
                    list.push(string.into());
                } else {
                    let (number, len) = get_number_data(rest);
                    rest = &rest[len+2..];
                    list.push(number.into());
                }
            }
            return list.into();
        } else { // dictionary
            let mut key: Option<String> = None;
            let mut rest = encoded_value.strip_prefix("d").and_then(|r| r.strip_suffix("e")).unwrap();
            let mut dict: Map<String, Value> = Map::new();
            let mut insert_if_can = |item: Value| {
                if key.is_none() {
                    key = Some(item.to_string())
                } else {
                    dict.insert(key.take().unwrap(), item);
                }
            };
            while !rest.is_empty() {
                if rest.chars().next().unwrap().is_ascii_digit() {
                    let (colon_idx, strlen) = get_str_data(rest);
                    let string = &rest[colon_idx + 1..colon_idx + 1 + strlen];
                    rest = &rest[colon_idx+strlen+1..];
                    insert_if_can(string.into());
                } else {
                    let (number, len) = get_number_data(rest);
                    rest = &rest[len+2..];
                    insert_if_can(number.into());
                }

            }
            return dict.into()
        }
    }
    else {
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
