pub(crate) fn find_first_string_literal(
    source: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize, String)> {
    let bytes = source.as_bytes();
    let mut index = start;
    while index < end && index < bytes.len() {
        let quote = bytes[index];
        if quote != b'"' && quote != b'\'' {
            index += 1;
            continue;
        }

        let content_start = index + 1;
        index = content_start;
        while index < end && index < bytes.len() {
            if bytes[index] == quote && !is_escaped(bytes, index) {
                let value = source.get(content_start..index)?.to_string();
                return Some((content_start, index, value));
            }
            index += 1;
        }
        return None;
    }
    None
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    if index == 0 {
        return false;
    }
    let mut slash_count = 0;
    let mut cursor = index - 1;
    loop {
        if bytes[cursor] != b'\\' {
            break;
        }
        slash_count += 1;
        if cursor == 0 {
            break;
        }
        cursor -= 1;
    }
    slash_count % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_first_string_literal_and_ignores_escaped_quotes() {
        let source = r#"value = "old \"quoted\" value" and 'next'"#;

        let literal = find_first_string_literal(source, 0, source.len());

        assert_eq!(
            literal,
            Some((9, 29, r#"old \"quoted\" value"#.to_string()))
        );
    }
}
