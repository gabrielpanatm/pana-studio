use std::collections::{BTreeMap, HashMap};

use pest::{iterators::Pair, Parser};
use pest_derive::Parser;
use serde::Serialize;

#[derive(Parser)]
#[grammar = "source_graph/zola_shortcode.pest"]
struct ZolaShortcodeParser;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZolaShortcodeDocument {
    source: String,
    pub(crate) content_start: usize,
    pub(crate) invocations: Vec<ZolaShortcodeInvocation>,
    pub(crate) ignored_ranges: Vec<ZolaShortcodeRange>,
    pub(crate) parse_error: Option<String>,
}

impl ZolaShortcodeDocument {
    pub(crate) fn reconstruct(&self) -> &str {
        &self.source
    }

    pub(crate) fn is_lossless(&self) -> bool {
        self.invocations
            .iter()
            .all(|invocation| invocation.is_valid_for(&self.source))
            && self
                .ignored_ranges
                .iter()
                .all(|range| range.is_valid_for(&self.source))
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ZolaShortcodeInvocation {
    pub name: String,
    pub arguments: BTreeMap<String, ZolaShortcodeValue>,
    pub range: ZolaShortcodeRange,
    pub call_range: ZolaShortcodeRange,
    pub body_range: Option<ZolaShortcodeRange>,
    pub nth: usize,
    pub inner: Vec<ZolaShortcodeInvocation>,
    pub source_node_id: Option<String>,
}

impl ZolaShortcodeInvocation {
    fn is_valid_for(&self, source: &str) -> bool {
        self.range.is_valid_for(source)
            && self.call_range.is_valid_for(source)
            && self
                .body_range
                .as_ref()
                .is_none_or(|range| range.is_valid_for(source))
            && self.inner.iter().all(|inner| {
                inner.is_valid_for(source)
                    && self.body_range.as_ref().is_some_and(|body| {
                        body.start <= inner.range.start && inner.range.end <= body.end
                    })
            })
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ZolaShortcodeRange {
    pub start: usize,
    pub end: usize,
}

impl ZolaShortcodeRange {
    fn is_valid_for(&self, source: &str) -> bool {
        self.start <= self.end
            && self.end <= source.len()
            && source.is_char_boundary(self.start)
            && source.is_char_boundary(self.end)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum ZolaShortcodeValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Array(Vec<ZolaShortcodeValue>),
}

pub(crate) fn parse_zola_shortcodes(source: &str) -> ZolaShortcodeDocument {
    let content_start = markdown_content_start(source);
    let content = &source[content_start..];
    let mut invocations = Vec::new();
    let mut ignored_ranges = Vec::new();
    let mut counts = HashMap::<String, usize>::new();
    let parse_error = match ZolaShortcodeParser::parse(Rule::page, content) {
        Ok(mut pairs) => {
            if let Some(page) = pairs.next() {
                project_pairs(
                    page.into_inner(),
                    content_start,
                    &mut counts,
                    &mut invocations,
                    &mut ignored_ranges,
                );
            }
            None
        }
        Err(error) => Some(error.renamed_rules(zola_rule_name).to_string()),
    };
    ZolaShortcodeDocument {
        source: source.to_string(),
        content_start,
        invocations,
        ignored_ranges,
        parse_error,
    }
}

fn project_pairs(
    pairs: pest::iterators::Pairs<'_, Rule>,
    base_offset: usize,
    counts: &mut HashMap<String, usize>,
    invocations: &mut Vec<ZolaShortcodeInvocation>,
    ignored_ranges: &mut Vec<ZolaShortcodeRange>,
) {
    for pair in pairs {
        match pair.as_rule() {
            Rule::inline_shortcode => {
                let range = absolute_range(&pair, base_offset);
                let call_range = range.clone();
                let (name, arguments) = parse_shortcode_call(pair);
                let nth = next_invocation(counts, &name);
                invocations.push(ZolaShortcodeInvocation {
                    name,
                    arguments,
                    range,
                    call_range,
                    body_range: None,
                    nth,
                    inner: Vec::new(),
                    source_node_id: None,
                });
            }
            Rule::shortcode_with_body => {
                let range = absolute_range(&pair, base_offset);
                let mut inner_pairs = pair.into_inner();
                let Some(call) = inner_pairs.next() else {
                    continue;
                };
                let call_range = absolute_range(&call, base_offset);
                let (name, arguments) = parse_shortcode_call(call);
                let nth = next_invocation(counts, &name);
                let body = inner_pairs.next();
                let body_range = body.as_ref().map(|body| absolute_range(body, base_offset));
                let mut inner = Vec::new();
                if let Some(body) = body {
                    let body_span = body.as_span();
                    let body_offset = base_offset + body_span.start();
                    if let Ok(mut nested_pairs) =
                        ZolaShortcodeParser::parse(Rule::page, body.as_str())
                    {
                        if let Some(page) = nested_pairs.next() {
                            project_pairs(
                                page.into_inner(),
                                body_offset,
                                counts,
                                &mut inner,
                                ignored_ranges,
                            );
                        }
                    }
                }
                invocations.push(ZolaShortcodeInvocation {
                    name,
                    arguments,
                    range,
                    call_range,
                    body_range,
                    nth,
                    inner,
                    source_node_id: None,
                });
            }
            Rule::ignored_inline_shortcode | Rule::ignored_shortcode_with_body => {
                ignored_ranges.push(absolute_range(&pair, base_offset));
            }
            Rule::EOI | Rule::text => {}
            _ => {}
        }
    }
}

fn parse_shortcode_call(pair: Pair<'_, Rule>) -> (String, BTreeMap<String, ZolaShortcodeValue>) {
    let mut name = None;
    let mut arguments = BTreeMap::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ident if name.is_none() => {
                name = Some(inner.as_str().to_string());
            }
            Rule::kwarg => {
                let mut keyword = None;
                let mut value = None;
                for item in inner.into_inner() {
                    match item.as_rule() {
                        Rule::ident => keyword = Some(item.as_str().to_string()),
                        Rule::literal => value = parse_literal(item),
                        _ => {}
                    }
                }
                if let (Some(keyword), Some(value)) = (keyword, value) {
                    arguments.insert(keyword, value);
                }
            }
            _ => {}
        }
    }
    (name.unwrap_or_default(), arguments)
}

fn parse_literal(pair: Pair<'_, Rule>) -> Option<ZolaShortcodeValue> {
    let inner = pair.into_inner().next()?;
    match inner.as_rule() {
        Rule::boolean => Some(ZolaShortcodeValue::Boolean(inner.as_str() == "true")),
        Rule::string => {
            let value = inner.as_str();
            let quote = value.chars().next()?;
            Some(ZolaShortcodeValue::String(value.replace(quote, "")))
        }
        Rule::float => inner
            .as_str()
            .parse::<f64>()
            .ok()
            .map(ZolaShortcodeValue::Float),
        Rule::int => inner
            .as_str()
            .parse::<i64>()
            .ok()
            .map(ZolaShortcodeValue::Integer),
        Rule::array => Some(ZolaShortcodeValue::Array(
            inner.into_inner().filter_map(parse_literal).collect(),
        )),
        _ => None,
    }
}

fn absolute_range(pair: &Pair<'_, Rule>, base_offset: usize) -> ZolaShortcodeRange {
    let span = pair.as_span();
    ZolaShortcodeRange {
        start: base_offset + span.start(),
        end: base_offset + span.end(),
    }
}

fn next_invocation(counts: &mut HashMap<String, usize>, name: &str) -> usize {
    let count = counts.entry(name.to_string()).or_default();
    *count += 1;
    *count
}

fn markdown_content_start(source: &str) -> usize {
    let bom_len = source
        .starts_with('\u{feff}')
        .then_some('\u{feff}'.len_utf8())
        .unwrap_or_default();
    let without_bom = &source[bom_len..];
    let Some(marker) = ["+++", "---"]
        .iter()
        .find(|marker| without_bom.starts_with(**marker))
    else {
        return 0;
    };
    let body_start = bom_len + marker.len();
    let rest = &source[body_start..];
    let close = format!("\n{marker}");
    let Some(close_relative) = rest.find(&close) else {
        return 0;
    };
    let mut content_start = body_start + close_relative + close.len();
    if source.get(content_start..content_start + 2) == Some("\r\n") {
        content_start += 2;
    } else if source.get(content_start..content_start + 1) == Some("\n") {
        content_start += 1;
    }
    content_start
}

fn zola_rule_name(rule: &Rule) -> String {
    match rule {
        Rule::int => "un număr întreg",
        Rule::float => "un număr zecimal",
        Rule::string => "un șir",
        Rule::literal => "un literal (număr, șir, boolean sau array)",
        Rule::array => "un array",
        Rule::kwarg => "un argument numit",
        Rule::ident => "un identificator",
        Rule::inline_shortcode => "un shortcode inline",
        Rule::ignored_inline_shortcode => "un shortcode inline ignorat",
        Rule::sc_body_start => "începutul unui shortcode cu body",
        Rule::ignored_sc_body_start => "începutul unui shortcode ignorat",
        Rule::text => "text",
        Rule::EOI => "sfârșitul conținutului",
        Rule::double_quoted_string => "un șir între ghilimele duble",
        Rule::single_quoted_string => "un șir între ghilimele simple",
        Rule::backquoted_quoted_string => "un șir între backticks",
        Rule::boolean => "un boolean",
        Rule::all_chars => "un caracter alfanumeric",
        Rule::kwargs => "o listă de argumente",
        Rule::sc_def => "o definiție shortcode",
        Rule::shortcode_with_body => "un shortcode cu body",
        Rule::ignored_shortcode_with_body => "un shortcode ignorat cu body",
        Rule::sc_body_end => "{% end %}",
        Rule::ignored_sc_body_end => "{%/* end */%}",
        Rule::text_in_body_sc => "conținutul shortcode-ului",
        Rule::text_in_ignored_body_sc => "conținut ignorat",
        Rule::content => "conținut",
        Rule::page => "pagina",
        Rule::WHITESPACE => "spațiu",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_is_lossless_and_matches_pinned_zola_shortcode_shapes() {
        let source = r#"+++
title = "Shortcodes"
+++
{{ video(id="abc", autoplay=true, sizes=[1, 2]) }}
{% quote(author='Ana') %}
Text {{ badge(label=`Nou`) }}
{% end %}
{{/* escaped() */}}
"#;
        let document = parse_zola_shortcodes(source);
        assert!(document.parse_error.is_none(), "{:?}", document.parse_error);
        assert!(document.is_lossless());
        assert_eq!(document.reconstruct(), source);
        assert_eq!(document.invocations.len(), 2);
        assert_eq!(document.invocations[0].name, "video");
        assert_eq!(document.invocations[0].nth, 1);
        assert_eq!(document.invocations[1].name, "quote");
        assert_eq!(document.invocations[1].inner.len(), 1);
        assert_eq!(document.invocations[1].inner[0].name, "badge");
        assert_eq!(document.ignored_ranges.len(), 1);
    }

    #[test]
    fn malformed_shortcode_is_reported_without_losing_source() {
        let source = "Text {% end %}";
        let document = parse_zola_shortcodes(source);
        assert!(document.parse_error.is_some());
        assert_eq!(document.reconstruct(), source);
    }
}
