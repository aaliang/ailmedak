pub fn as_hex_string(inpt: &[u8]) -> String {
    inpt.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join("")
}
