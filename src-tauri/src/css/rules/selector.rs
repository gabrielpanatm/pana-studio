use crate::source_graph::model::SourceRange;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssSelectorSourceTarget {
    pub selector: String,
    pub range: SourceRange,
    block_start: usize,
    block_end: usize,
}

pub fn locate_rule_block(css: &str, selector: &str) -> Option<(usize, usize, usize, usize, usize)> {
    let target = normalize_selector(selector);
    if target.is_empty() {
        return None;
    }

    locate_top_level_block(css, |raw_selector| {
        !raw_selector.starts_with('@') && selector_list_contains(raw_selector, &target)
    })
}

pub fn selector_source_target_at_offset(
    css: &str,
    offset: usize,
) -> Option<CssSelectorSourceTarget> {
    let offset = offset.min(css.len());
    let targets = selector_source_targets(css);
    if let Some(target) = targets
        .iter()
        .find(|target| target.range.start <= offset && offset <= target.range.end)
    {
        return Some(target.clone());
    }
    let innermost_start = targets
        .iter()
        .filter(|target| target.block_start <= offset && offset < target.block_end)
        .map(|target| target.block_start)
        .max()?;
    let mut containing = targets.into_iter().filter(|target| {
        target.block_start == innermost_start
            && target.block_start <= offset
            && offset < target.block_end
    });
    let target = containing.next()?;
    containing.next().is_none().then_some(target)
}

pub fn selector_source_target(css: &str, selector: &str) -> Option<CssSelectorSourceTarget> {
    let target = normalize_selector(selector);
    if target.is_empty() {
        return None;
    }
    selector_source_targets(css)
        .into_iter()
        .find(|candidate| normalize_selector(&candidate.selector) == target)
}

fn selector_source_targets(css: &str) -> Vec<CssSelectorSourceTarget> {
    let mut scanner = CssScanner::default();
    let mut statement_starts = vec![0usize];
    let mut targets = Vec::new();
    for (index, character) in css.char_indices() {
        if !scanner.consume(css, index, character) {
            continue;
        }
        match character {
            '{' => {
                let container_depth = scanner.depth.saturating_sub(1);
                let statement_start = statement_starts
                    .get(container_depth)
                    .copied()
                    .unwrap_or_default();
                let selector_start = selector_start_after_trivia(css, statement_start, index);
                let selector_end = trim_end_offset(css, selector_start, index);
                if selector_start < selector_end
                    && !normalize_selector(&css[selector_start..selector_end]).starts_with('@')
                {
                    let block_start = index + character.len_utf8();
                    let block_end = matching_block_end(css, block_start)
                        .map(|end| end + 1)
                        .unwrap_or(css.len());
                    for (part_start, part_end) in
                        top_level_selector_part_ranges(css, selector_start, selector_end)
                    {
                        let selector = normalize_selector(&css[part_start..part_end]);
                        if selector.starts_with('.') {
                            targets.push(CssSelectorSourceTarget {
                                selector,
                                range: source_range(css, part_start, part_end),
                                block_start,
                                block_end,
                            });
                        }
                    }
                }
                if statement_starts.len() <= scanner.depth {
                    statement_starts.resize(scanner.depth + 1, index + character.len_utf8());
                }
                statement_starts[scanner.depth] = index + character.len_utf8();
            }
            '}' | ';' => {
                if statement_starts.len() <= scanner.depth {
                    statement_starts.resize(scanner.depth + 1, index + character.len_utf8());
                }
                statement_starts[scanner.depth] = index + character.len_utf8();
            }
            _ => {}
        }
    }
    targets
}

fn top_level_selector_part_ranges(source: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut part_start = start;
    let mut quote = None;
    let mut escaped = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (relative_index, character) in source[start..end].char_indices() {
        let index = start + relative_index;
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ',' if paren_depth == 0 && bracket_depth == 0 => {
                if let Some(range) = trimmed_range(source, part_start, index) {
                    result.push(range);
                }
                part_start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if let Some(range) = trimmed_range(source, part_start, end) {
        result.push(range);
    }
    result
}

fn trimmed_range(source: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let value = &source[start..end];
    let leading = value.len().saturating_sub(value.trim_start().len());
    let trailing = value.len().saturating_sub(value.trim_end().len());
    let start = start + leading;
    let end = end.saturating_sub(trailing);
    (start < end).then_some((start, end))
}

fn trim_end_offset(source: &str, start: usize, end: usize) -> usize {
    end.saturating_sub(
        source[start..end]
            .len()
            .saturating_sub(source[start..end].trim_end().len()),
    )
}

fn source_range(source: &str, start: usize, end: usize) -> SourceRange {
    let (line, column) = line_column(source, start);
    let (end_line, end_column) = line_column(source, end);
    SourceRange {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}

fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (index, character) in source.char_indices() {
        if index >= offset {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

pub fn locate_exact_rule_block(
    css: &str,
    selector: &str,
) -> Option<(usize, usize, usize, usize, usize)> {
    let target = normalize_selector_list(selector);
    if target.is_empty() {
        return None;
    }

    locate_top_level_block(css, |raw_selector| {
        !raw_selector.starts_with('@') && normalize_selector_list(raw_selector) == target
    })
}

pub(super) fn locate_top_level_block(
    css: &str,
    predicate: impl Fn(&str) -> bool,
) -> Option<(usize, usize, usize, usize, usize)> {
    let mut scanner = CssScanner::default();
    let mut statement_start = 0usize;
    for (index, character) in css.char_indices() {
        if !scanner.consume(css, index, character) {
            continue;
        }
        match character {
            '{' if scanner.depth == 1 => {
                let selector_start = selector_start_after_trivia(css, statement_start, index);
                let raw_selector = css[selector_start..index].trim();
                if predicate(raw_selector) {
                    let content_start = index + character.len_utf8();
                    let content_end = matching_block_end(css, content_start)?;
                    return Some((
                        selector_start,
                        index,
                        content_start,
                        content_end,
                        content_end + 1,
                    ));
                }
            }
            '}' if scanner.depth == 0 => statement_start = index + character.len_utf8(),
            ';' if scanner.depth == 0 => statement_start = index + character.len_utf8(),
            _ => {}
        }
    }
    None
}

fn selector_list_contains(raw_selector: &str, target: &str) -> bool {
    split_top_level_selector_list(raw_selector)
        .into_iter()
        .any(|candidate| normalize_selector(candidate) == target)
}

fn split_top_level_selector_list(selector: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (index, character) in selector.char_indices() {
        if let Some(active_quote) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active_quote {
                quote = None;
            }
            continue;
        }
        if matches!(character, '\'' | '"') {
            quote = Some(character);
            continue;
        }
        match character {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            ',' if paren_depth == 0 && bracket_depth == 0 => {
                result.push(&selector[start..index]);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    result.push(&selector[start..]);
    result
}

fn normalize_selector_list(selector: &str) -> String {
    split_top_level_selector_list(selector)
        .into_iter()
        .map(normalize_selector)
        .collect::<Vec<_>>()
        .join(", ")
}

fn matching_block_end(css: &str, content_start: usize) -> Option<usize> {
    let mut scanner = CssScanner {
        depth: 1,
        ..CssScanner::default()
    };
    for (relative_index, character) in css[content_start..].char_indices() {
        let index = content_start + relative_index;
        if !scanner.consume(css, index, character) {
            continue;
        }
        if character == '}' && scanner.depth == 0 {
            return Some(index);
        }
    }
    None
}

fn selector_start_after_trivia(css: &str, start: usize, end: usize) -> usize {
    let mut cursor = start;
    loop {
        while cursor < end {
            let Some(character) = css[cursor..end].chars().next() else {
                break;
            };
            if !character.is_whitespace() {
                break;
            }
            cursor += character.len_utf8();
        }
        if css[cursor..end].starts_with("/*") {
            let Some(close) = css[cursor + 2..end].find("*/") else {
                return cursor;
            };
            cursor += 2 + close + 2;
            continue;
        }
        if css[cursor..end].starts_with("//") {
            let Some(line_end) = css[cursor + 2..end].find('\n') else {
                return cursor;
            };
            cursor += 2 + line_end + 1;
            continue;
        }
        return cursor;
    }
}

#[derive(Default)]
struct CssScanner {
    depth: usize,
    quote: Option<char>,
    escaped: bool,
    line_comment: bool,
    block_comment: bool,
    skip_next: bool,
}

impl CssScanner {
    /// Returns true only when the character belongs to CSS structure rather
    /// than a quote/comment. `depth` is updated before the caller observes it.
    fn consume(&mut self, source: &str, index: usize, character: char) -> bool {
        if self.skip_next {
            self.skip_next = false;
            return false;
        }
        let next = source[index + character.len_utf8()..].chars().next();
        if self.line_comment {
            if character == '\n' {
                self.line_comment = false;
            }
            return false;
        }
        if self.block_comment {
            if character == '*' && next == Some('/') {
                self.block_comment = false;
                self.skip_next = true;
            }
            return false;
        }
        if let Some(active_quote) = self.quote {
            if self.escaped {
                self.escaped = false;
            } else if character == '\\' {
                self.escaped = true;
            } else if character == active_quote {
                self.quote = None;
            }
            return false;
        }
        if character == '/' && next == Some('/') {
            self.line_comment = true;
            self.skip_next = true;
            return false;
        }
        if character == '/' && next == Some('*') {
            self.block_comment = true;
            self.skip_next = true;
            return false;
        }
        if matches!(character, '\'' | '"') {
            self.quote = Some(character);
            return false;
        }
        match character {
            '{' => self.depth += 1,
            '}' => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
        true
    }
}

pub fn normalize_selector(selector: &str) -> String {
    selector
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

pub(super) fn base_selector_of(selector: &str) -> Option<&str> {
    let s = selector.trim();
    if !s.starts_with('.') {
        return None;
    }
    if let Some(pos) = s.find(':') {
        if pos > 0 {
            return Some(&s[..pos]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_selector_matches_an_individual_panel_target() {
        let source = ".card,\n.hero:is(.large, .wide) { color: red; }";
        let block = locate_rule_block(source, ".hero:is(.large, .wide)").unwrap();
        assert_eq!(&source[block.2..block.3], " color: red; ");
    }

    #[test]
    fn braces_in_strings_and_comments_do_not_end_the_rule() {
        let source = r#"/* } */ .hero { content: "}"; background: url("data:image/svg+xml,{x}"); color: red; }"#;
        let block = locate_rule_block(source, ".hero").unwrap();
        assert!(source[block.2..block.3].contains("color: red"));
    }

    #[test]
    fn does_not_match_nested_rules_as_top_level_panel_rules() {
        let source = ".parent { .child { color: red; } }\n.child { color: blue; }";
        let block = locate_rule_block(source, ".child").unwrap();
        assert!(source[block.2..block.3].contains("blue"));
    }
}
