pub fn extract_data_anims(content: &str) -> Vec<String> {
    let needle = "data-anim=\"";
    let mut values: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut start = 0;

    while let Some(pos) = content[start..].find(needle) {
        let abs = start + pos + needle.len();
        if let Some(end) = content[abs..].find('"') {
            let value = &content[abs..abs + end];
            if !value.is_empty() && seen.insert(value.to_string()) {
                values.push(value.to_string());
            }
        }
        start = abs + 1;
        if start >= content.len() {
            break;
        }
    }

    values
}
