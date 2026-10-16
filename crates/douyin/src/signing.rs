//! Rust adaptation of the compact webcast signer and HTTP signing byte layout in
//! jwwsjlm/douyinLive, commit 0e506f3cf935beede946296602af2e52d57ecea2,
//! Copyright (c) 2025 今晚打打, MIT. See ../LICENSE-DOUYINLIVE.
//! No JavaScript SDK is copied or executed.
use base64::{
    Engine,
    alphabet::Alphabet,
    engine::{GeneralPurpose, general_purpose::NO_PAD},
};
use md5::{Digest, Md5};
use sm3::Sm3;

const WS_ALPHABET: &str = "Dkdpgh4ZKsQB80/Mfvw36XI1R25+WUAlEi7NLboqYTOPuzmFjJnryx9HVGcaStCe";
fn encode(bytes: &[u8], alphabet: &str) -> String {
    GeneralPurpose::new(
        &Alphabet::new(alphabet).expect("fixed 64-character alphabet"),
        NO_PAD,
    )
    .encode(bytes)
}
fn rc4(data: &[u8], key: &[u8]) -> Vec<u8> {
    let mut state = std::array::from_fn::<_, 256, _>(|n| n as u8);
    let mut j = 0usize;
    for i in 0..256 {
        j = (j + usize::from(state[i]) + usize::from(key[i % key.len()])) & 255;
        state.swap(i, j);
    }
    let (mut i, mut j) = (0, 0);
    data.iter()
        .map(|b| {
            i = (i + 1) & 255;
            j = (j + usize::from(state[i])) & 255;
            state.swap(i, j);
            b ^ state[(usize::from(state[i]) + usize::from(state[j])) & 255]
        })
        .collect()
}
pub fn stub(value: &str) -> [u8; 16] {
    Md5::digest(value.as_bytes()).into()
}
pub fn websocket(stub: [u8; 16], counter: u8, flag: bool, random: u8, key: u8) -> String {
    let digest = Md5::digest(stub);
    let empty = Md5::digest(Md5::digest([]));
    let mut payload = [
        counter & 63,
        0,
        1,
        14,
        empty[14],
        empty[15],
        digest[14],
        digest[15],
        random,
        0,
    ];
    payload[9] = payload[..9].iter().fold(0, |a, b| a ^ b);
    let mut result = vec![64 | if flag { 16 } else { 0 }, key];
    result.extend(rc4(&payload, &[key]));
    encode(&result, WS_ALPHABET)
}

/// Sign the *exact encoded* query which will be transmitted. Time is explicit for tests.
pub fn http(query: &str, user_agent: &str, millis: u64) -> String {
    let hash = |b: &[u8]| -> Vec<u8> { Sm3::digest(b).to_vec() };
    let query_hash = hash(&hash(format!("{query}cus").as_bytes()));
    let suffix = hash(&hash(b"cus"));
    let ua = hash(
        encode(
            &rc4(user_agent.as_bytes(), &[0, 1, 14]),
            "ckdp1h4ZKsUB80/Mfvw36XIgR25+WQAlEi7NLboqYTOPuzmFjJnryx9HVGDaStCe",
        )
        .as_bytes(),
    );
    let mut b = [0u8; 73];
    let start = millis as u32;
    b[18] = 44;
    b[20..24].copy_from_slice(&start.to_be_bytes());
    b[30..32].copy_from_slice(&1u16.to_be_bytes());
    b[34..38].copy_from_slice(&14u32.to_be_bytes());
    b[38..40].copy_from_slice(&query_hash[21..23]);
    b[40..42].copy_from_slice(&suffix[21..23]);
    b[42..44].copy_from_slice(&ua[23..25]);
    b[44..48].copy_from_slice(&start.wrapping_add(100).to_be_bytes());
    b[48] = 3;
    b[52..56].copy_from_slice(&110624u32.to_be_bytes());
    b[57..61].copy_from_slice(&6383u32.to_le_bytes());
    let environment = b"1920|1080|1920|1040|0|30|0|0|1872|92|1920|1040|1857|92|1|24|Win32";
    b[65..67].copy_from_slice(&(environment.len() as u16).to_le_bytes());
    let order = [
        18, 20, 52, 26, 30, 34, 58, 38, 40, 53, 42, 21, 27, 54, 55, 31, 35, 57, 39, 41, 43, 22, 28,
        32, 60, 36, 23, 29, 33, 37, 44, 45, 59, 46, 47, 48, 49, 50, 24, 25, 65, 66, 70, 71,
    ];
    // The upstream checksum intentionally excludes index 34 (the high byte of arg2).
    let check = order
        .iter()
        .filter(|&&i| i != 34)
        .fold(0, |sum, &i| sum ^ b[i]);
    let mut payload: Vec<_> = order.into_iter().map(|i| b[i]).collect();
    payload.extend(environment);
    payload.push(check);
    let mut prefix = Vec::new();
    for (random, options) in [(1234u16, [3u8, 45]), (9876, [1, 0]), (5555, [1, 5])] {
        let [lo, hi] = random.to_le_bytes();
        prefix.extend([
            (lo & 170) | (options[0] & 85),
            (lo & 85) | (options[0] & 170),
            (hi & 170) | (options[1] & 85),
            (hi & 85) | (options[1] & 170),
        ]);
    }
    prefix.extend(rc4(&payload, &[121]));
    format!(
        "{}=",
        encode(
            &prefix,
            "Dkdpgh2ZmsQB80/MfvV36XI1R45-WUAlEixNLwoqYTOPuzKFjJnry79HbGcaStCe"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn webcast_matches_published_independent_vectors() {
        let input = [
            0x70, 0x4f, 0x43, 0x6b, 0x25, 0x58, 0xb0, 0xd7, 0xa1, 0xc7, 0xe7, 0x58, 0x52, 0x7d,
            0xd8, 0xf1,
        ];
        assert_eq!(websocket(input, 1, true, 63, 63), "6pt0VC0aRZDP7n07");
        assert_eq!(websocket(input, 0, false, 0, 6), "fD+evFuzasjOC8Uu");
        assert_ne!(
            websocket(input, 2, true, 63, 63),
            websocket(input, 1, true, 63, 63)
        );
    }
    #[test]
    fn query_bytes_and_fingerprint_are_not_interchangeable() {
        // Independently executed pinned Go reference with its clock fixed to 123456789 ms.
        assert_eq!(
            http(
                "aid=6383&app_name=douyin_web&room_id=123456789",
                "Mozilla/5.0",
                123456789
            ),
            "E7mhBmg6mEVNgf6X5R/LfY3q6333YILY0HViMD2fJVfL/y39HMYX9exoLDkvSeYjZsmfIFYjy4hbO3xprQAjM36UHWwEUdQ2mgWkKl5Q5I0j53iruyRDntmF4vj3SFlm5XNAEOk0y75rKb70Woqe-vIlO62-zo0/9X8="
        );
        assert_ne!(http("a=%20", "UA", 1), http("a=+", "UA", 1));
        assert_ne!(http("a=1", "UA", 1), http("a=1", "other", 1));
        assert_ne!(http("a=1", "UA", 1), http("a=1", "UA", 2));
    }
}
