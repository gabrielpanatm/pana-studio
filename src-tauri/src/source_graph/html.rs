const SKIP_TAGS: &[&str] = &[
    "html", "head", "body", "script", "style", "meta", "link", "base", "title", "br", "hr",
    "input", "area", "col", "embed", "param", "source", "track", "wbr", "noscript", "template",
];

#[derive(Clone)]
pub struct HtmlItem {
    pub tag: String,
    pub label: String,
    pub start: usize,
    pub end: usize,
}

pub fn parse_html_opening_tags(source: &str) -> Vec<HtmlItem> {
    let bytes = source.as_bytes();
    let mut items = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        if is_tera_start(bytes, index) {
            index = skip_tera_token(bytes, index).unwrap_or(index + 2);
            continue;
        }

        if bytes[index] != b'<' {
            index += 1;
            continue;
        }

        let after_lt = index + 1;
        if after_lt >= bytes.len() {
            break;
        }

        let next = bytes[after_lt];
        if next == b'/' || next == b'!' || next == b'?' || !next.is_ascii_alphabetic() {
            index += 1;
            continue;
        }

        let name_start = after_lt;
        let mut cursor = name_start;
        while cursor < bytes.len()
            && (bytes[cursor].is_ascii_alphanumeric()
                || bytes[cursor] == b'-'
                || bytes[cursor] == b':')
        {
            cursor += 1;
        }

        let tag = source[name_start..cursor].to_ascii_lowercase();
        if SKIP_TAGS.contains(&tag.as_str()) {
            index = cursor;
            continue;
        }

        let Some(end) = opening_tag_end(source, cursor) else {
            break;
        };
        let raw = &source[index..end];
        items.push(HtmlItem {
            label: html_label(&tag, raw),
            tag,
            start: index,
            end,
        });
        index = end;
    }

    items
}

fn opening_tag_end(source: &str, mut index: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut in_double_quote = false;
    let mut in_single_quote = false;

    while index < bytes.len() {
        if !in_double_quote && !in_single_quote && is_tera_start(bytes, index) {
            index = skip_tera_token(bytes, index).unwrap_or(index + 2);
            continue;
        }

        match bytes[index] {
            b'"' if !in_single_quote => in_double_quote = !in_double_quote,
            b'\'' if !in_double_quote => in_single_quote = !in_single_quote,
            b'>' if !in_double_quote && !in_single_quote => return Some(index + 1),
            _ => {}
        }
        index += 1;
    }

    None
}

fn is_tera_start(bytes: &[u8], index: usize) -> bool {
    index + 1 < bytes.len()
        && bytes[index] == b'{'
        && matches!(bytes[index + 1], b'%' | b'{' | b'#')
}

fn skip_tera_token(bytes: &[u8], index: usize) -> Option<usize> {
    let (close_a, close_b) = match bytes.get(index + 1).copied()? {
        b'%' => (b'%', b'}'),
        b'{' => (b'}', b'}'),
        b'#' => (b'#', b'}'),
        _ => return None,
    };
    let mut cursor = index + 2;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == close_a && bytes[cursor + 1] == close_b {
            return Some(cursor + 2);
        }
        cursor += 1;
    }
    None
}

fn html_label(tag: &str, raw: &str) -> String {
    if let Some(id) = attr_value(raw, "id") {
        return format!("<{tag} #{id}>");
    }
    if let Some(class_name) = attr_value(raw, "class")
        .and_then(|classes| classes.split_whitespace().next().map(str::to_string))
    {
        return format!("<{tag} .{class_name}>");
    }
    format!("<{tag}>")
}

fn attr_value(raw: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let mut search_start = 0;

    while let Some(relative) = raw[search_start..].find(&needle) {
        let index = search_start + relative;
        let before = raw[..index].chars().last();
        if before.is_some_and(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        }) {
            search_start = index + needle.len();
            continue;
        }

        let value_start = index + needle.len();
        let quote = raw.as_bytes().get(value_start).copied()?;
        if quote != b'"' && quote != b'\'' {
            return None;
        }
        let value_start = value_start + 1;
        let rest = &raw[value_start..];
        let end = rest.find(quote as char)?;
        return Some(rest[..end].to_string());
    }

    None
}
