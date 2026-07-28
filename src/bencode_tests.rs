use crate::bencode_parser;

#[test]
fn string_test() {
    let decoded_value = bencode_parser::decode("10:helloworld".as_bytes()).unwrap();
    println!("{}", decoded_value);
}

#[test]
fn number_test() {
    let decoded_value = bencode_parser::decode("i69420e".as_bytes()).unwrap();
    println!("{}", decoded_value);
}

#[test]
fn list_test() {
    let decoded_value = bencode_parser::decode("l5:gamer4:girl3:xxxi12ee".as_bytes()).unwrap();
    println!("{}", decoded_value);
}

#[test]
fn nested_lists() {
    let decoded_value = bencode_parser::decode(
        "lli1ei2eli3ei4eeel5:alphal4:betai5ei6el7:charliei8eeee3:ende".as_bytes(),
    )
    .unwrap();
    println!("{}", decoded_value);
}

#[test]
fn dict_test() {
    let decoded_value = bencode_parser::decode("d5:gamer4:girl3:xxxi12ee".as_bytes()).unwrap();
    println!("{}", decoded_value);
}

#[test]
fn nested_dicts() {
    let decoded_value = bencode_parser::decode(
        "d4:rootd6:configd5:debugi1e7:version3:1.0e4:infod3:agei20e4:name3:Bobee5:valuei42eee"
            .as_bytes(),
    )
    .unwrap();
    println!("{}", decoded_value);
}

#[test]
fn bigbencode() {
    let decoded_value = bencode_parser::decode("d4:infod4:name8:test.txt6:lengthi12345e6:piecesl6:piece16:piece26:piece3ee5:ownerd3:agei30e7:contactd5:email14:alice@test.com5:phonel8:123456788:98765432ee4:name5:Alicee4:tagsl4:rust7:bencode4:jsonl5:inneri42ed1:xi1e1:yl5:hello5:worldeeeee".as_bytes()).unwrap();
    println!("{}", decoded_value);
}
