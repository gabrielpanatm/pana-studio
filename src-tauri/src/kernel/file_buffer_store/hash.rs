pub fn hash_text(value: &str) -> String {
    hash_bytes(value.as_bytes())
}

pub fn hash_bytes(value: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}
