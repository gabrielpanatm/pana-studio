use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use serde::{Deserialize, Serialize};

use super::rules::CssProperty;

pub const CSS_GRID_SCHEMA_VERSION: u32 = 1;

const GRID_LONGHANDS: [&str; 17] = [
    "display",
    "grid-template-columns",
    "grid-template-rows",
    "grid-template-areas",
    "grid-auto-columns",
    "grid-auto-rows",
    "grid-auto-flow",
    "column-gap",
    "row-gap",
    "justify-content",
    "align-content",
    "justify-items",
    "align-items",
    "grid-column",
    "grid-row",
    "grid-area",
    "gap",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssGrid {
    pub schema_version: u32,
    pub display: Option<String>,
    pub template_columns: CssGridTrackList,
    pub template_rows: CssGridTrackList,
    pub template_areas: CssGridAreas,
    pub auto_columns: Option<String>,
    pub auto_rows: Option<String>,
    pub auto_flow: Option<String>,
    pub column_gap: Option<String>,
    pub row_gap: Option<String>,
    pub justify_content: Option<String>,
    pub align_content: Option<String>,
    pub justify_items: Option<String>,
    pub align_items: Option<String>,
    pub item_column: Option<String>,
    pub item_row: Option<String>,
    pub item_area: Option<String>,
    pub opaque_properties: BTreeMap<String, String>,
    pub structurally_editable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CssGridTrackListMode {
    None,
    Tracks,
    Subgrid,
    Masonry,
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssGridTrackList {
    pub raw: Option<String>,
    pub mode: CssGridTrackListMode,
    pub tracks: Vec<CssGridTrack>,
    pub structurally_editable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CssGridTrackKind {
    Keyword,
    Flex,
    Length,
    Minmax,
    FitContent,
    Repeat,
    LineNames,
    Dynamic,
    Opaque,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssGridTrack {
    pub id: String,
    pub kind: CssGridTrackKind,
    pub raw: String,
    pub repeat_count: Option<String>,
    pub repeat_tracks: Vec<CssGridTrack>,
    pub structurally_editable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CssGridAreas {
    pub raw: Option<String>,
    pub rows: Vec<Vec<String>>,
    pub valid: bool,
    pub error: Option<String>,
    pub structurally_editable: bool,
}

impl CssGrid {
    pub fn from_rules(rules: &[CssProperty]) -> Self {
        let declarations = rules
            .iter()
            .fold(BTreeMap::new(), |mut values, declaration| {
                values.insert(
                    declaration.property.trim().to_ascii_lowercase(),
                    declaration.value.trim().to_string(),
                );
                values
            });
        Self::from_declarations(&declarations)
    }

    pub fn from_declarations(declarations: &BTreeMap<String, String>) -> Self {
        let value = |property: &str| meaningful_value(declarations.get(property));
        let template_columns = CssGridTrackList::parse(value("grid-template-columns").as_deref());
        let template_rows = CssGridTrackList::parse(value("grid-template-rows").as_deref());
        let template_areas = CssGridAreas::parse(value("grid-template-areas").as_deref());

        let gap = value("gap").and_then(|raw| split_top_level_whitespace(&raw));
        let row_gap =
            value("row-gap").or_else(|| gap.as_ref().and_then(|parts| parts.first().cloned()));
        let column_gap = value("column-gap").or_else(|| {
            gap.as_ref()
                .and_then(|parts| parts.get(1).or_else(|| parts.first()).cloned())
        });

        let opaque_properties = ["grid", "grid-template", "place-content", "place-items"]
            .into_iter()
            .filter_map(|property| value(property).map(|raw| (property.to_string(), raw)))
            .collect::<BTreeMap<_, _>>();
        let structurally_editable = opaque_properties.is_empty()
            && template_columns.structurally_editable
            && template_rows.structurally_editable
            && template_areas.valid;

        Self {
            schema_version: CSS_GRID_SCHEMA_VERSION,
            display: value("display"),
            template_columns,
            template_rows,
            template_areas,
            auto_columns: value("grid-auto-columns"),
            auto_rows: value("grid-auto-rows"),
            auto_flow: value("grid-auto-flow"),
            column_gap,
            row_gap,
            justify_content: value("justify-content"),
            align_content: value("align-content"),
            justify_items: value("justify-items"),
            align_items: value("align-items"),
            item_column: value("grid-column"),
            item_row: value("grid-row"),
            item_area: value("grid-area"),
            opaque_properties,
            structurally_editable,
        }
    }

    pub fn to_longhands(&self) -> BTreeMap<String, String> {
        let mut values = BTreeMap::new();
        let mut insert = |property: &str, value: Option<String>| {
            values.insert(property.to_string(), value.unwrap_or_default());
        };
        insert("display", self.display.clone());
        insert("grid-template-columns", self.template_columns.to_css());
        insert("grid-template-rows", self.template_rows.to_css());
        insert("grid-template-areas", self.template_areas.to_css());
        insert("grid-auto-columns", self.auto_columns.clone());
        insert("grid-auto-rows", self.auto_rows.clone());
        insert("grid-auto-flow", self.auto_flow.clone());
        insert("column-gap", self.column_gap.clone());
        insert("row-gap", self.row_gap.clone());
        insert("justify-content", self.justify_content.clone());
        insert("align-content", self.align_content.clone());
        insert("justify-items", self.justify_items.clone());
        insert("align-items", self.align_items.clone());
        insert("grid-column", self.item_column.clone());
        insert("grid-row", self.item_row.clone());
        insert("grid-area", self.item_area.clone());
        for (property, value) in &self.opaque_properties {
            values.insert(property.clone(), value.clone());
        }
        values
    }
}

impl CssGridTrackList {
    pub fn parse(raw: Option<&str>) -> Self {
        let raw = raw
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let Some(value) = raw.as_deref() else {
            return Self {
                raw: None,
                mode: CssGridTrackListMode::None,
                tracks: Vec::new(),
                structurally_editable: true,
            };
        };
        let normalized = value.to_ascii_lowercase();
        if normalized == "none" {
            return Self {
                raw,
                mode: CssGridTrackListMode::None,
                tracks: Vec::new(),
                structurally_editable: true,
            };
        }
        if normalized == "subgrid" || normalized.starts_with("subgrid ") {
            return Self {
                raw,
                mode: CssGridTrackListMode::Subgrid,
                tracks: Vec::new(),
                structurally_editable: false,
            };
        }
        if normalized == "masonry" {
            return Self {
                raw,
                mode: CssGridTrackListMode::Masonry,
                tracks: Vec::new(),
                structurally_editable: false,
            };
        }
        let Some(parts) = split_top_level_whitespace(value) else {
            return Self {
                raw,
                mode: CssGridTrackListMode::Opaque,
                tracks: Vec::new(),
                structurally_editable: false,
            };
        };
        let tracks = parts
            .iter()
            .enumerate()
            .map(|(index, part)| parse_track(part, index))
            .collect::<Vec<_>>();
        let structurally_editable = tracks.iter().all(|track| track.structurally_editable);
        Self {
            raw,
            mode: CssGridTrackListMode::Tracks,
            tracks,
            structurally_editable,
        }
    }

    pub fn to_css(&self) -> Option<String> {
        match self.mode {
            CssGridTrackListMode::None => self.raw.clone().or_else(|| Some("none".to_string())),
            CssGridTrackListMode::Subgrid
            | CssGridTrackListMode::Masonry
            | CssGridTrackListMode::Opaque => self.raw.clone(),
            CssGridTrackListMode::Tracks => Some(
                self.tracks
                    .iter()
                    .map(serialize_track)
                    .collect::<Vec<_>>()
                    .join(" "),
            ),
        }
    }
}

impl CssGridAreas {
    pub fn parse(raw: Option<&str>) -> Self {
        let raw = raw
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let Some(value) = raw.as_deref() else {
            return empty_areas(raw);
        };
        if value.eq_ignore_ascii_case("none") {
            return empty_areas(raw);
        }
        if is_dynamic(value) {
            return Self {
                raw,
                rows: Vec::new(),
                valid: true,
                error: None,
                structurally_editable: false,
            };
        }
        let rows = match parse_quoted_rows(value) {
            Ok(rows) => rows,
            Err(error) => {
                return Self {
                    raw,
                    rows: Vec::new(),
                    valid: false,
                    error: Some(error),
                    structurally_editable: false,
                }
            }
        };
        if let Err(error) = validate_area_rows(&rows) {
            return Self {
                raw,
                rows,
                valid: false,
                error: Some(error),
                structurally_editable: false,
            };
        }
        Self {
            raw,
            rows,
            valid: true,
            error: None,
            structurally_editable: true,
        }
    }

    pub fn to_css(&self) -> Option<String> {
        if self.rows.is_empty() {
            return self.raw.clone();
        }
        Some(
            self.rows
                .iter()
                .map(|row| format!("\"{}\"", row.join(" ")))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

pub fn normalize_grid_properties(
    properties: &HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    let mut normalized = properties.clone();
    for property in GRID_LONGHANDS {
        let Some(value) = properties.get(property) else {
            continue;
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            normalized.insert(property.to_string(), String::new());
            continue;
        }
        match property {
            "grid-template-columns"
            | "grid-template-rows"
            | "grid-auto-columns"
            | "grid-auto-rows" => {
                if has_top_level_comma(trimmed) {
                    return Err(format!("[css_grid_invalid_tracks] {property} conține o virgulă în afara unei funcții CSS."));
                }
                let tracks = CssGridTrackList::parse(Some(trimmed));
                if matches!(tracks.mode, CssGridTrackListMode::Opaque) {
                    return Err(format!(
                        "[css_grid_invalid_tracks] Expresia {property} este incompletă."
                    ));
                }
                if tracks.tracks.iter().any(known_invalid_track) {
                    return Err(format!(
                        "[css_grid_invalid_tracks] {property} conține o funcție Grid incompletă sau un repeat() invalid."
                    ));
                }
                normalized.insert(property.to_string(), tracks.to_css().unwrap_or_default());
            }
            "grid-template-areas" => {
                let areas = CssGridAreas::parse(Some(trimmed));
                if !areas.valid {
                    return Err(format!(
                        "[css_grid_invalid_areas] {}",
                        areas
                            .error
                            .unwrap_or_else(|| "Zonele grilei nu sunt valide.".to_string())
                    ));
                }
                normalized.insert(property.to_string(), areas.to_css().unwrap_or_default());
            }
            "grid-auto-flow" if !is_dynamic(trimmed) => {
                let tokens = split_top_level_whitespace(trimmed).unwrap_or_default();
                let valid = matches!(tokens.as_slice(), [flow] if matches!(flow.as_str(), "row" | "column" | "dense"))
                    || matches!(tokens.as_slice(), [flow, dense] if matches!(flow.as_str(), "row" | "column") && dense == "dense");
                if !valid {
                    return Err("[css_grid_invalid_auto_flow] grid-auto-flow acceptă row, column și opțional dense.".to_string());
                }
                normalized.insert(property.to_string(), tokens.join(" "));
            }
            _ => {
                normalized.insert(property.to_string(), trimmed.to_string());
            }
        }
    }
    Ok(normalized)
}

fn parse_track(raw: &str, index: usize) -> CssGridTrack {
    let raw = raw.trim().to_string();
    let normalized = raw.to_ascii_lowercase();
    let mut track = CssGridTrack {
        id: format!("grid-track-{index}"),
        kind: CssGridTrackKind::Opaque,
        raw: raw.clone(),
        repeat_count: None,
        repeat_tracks: Vec::new(),
        structurally_editable: true,
    };
    if raw.starts_with('[') && raw.ends_with(']') {
        track.kind = CssGridTrackKind::LineNames;
        return track;
    }
    if is_dynamic(&raw) {
        track.kind = CssGridTrackKind::Dynamic;
        track.structurally_editable = false;
        return track;
    }
    if matches!(normalized.as_str(), "auto" | "min-content" | "max-content") {
        track.kind = CssGridTrackKind::Keyword;
        return track;
    }
    if numeric_unit(&normalized, "fr") {
        track.kind = CssGridTrackKind::Flex;
        return track;
    }
    if is_length(&normalized) {
        track.kind = CssGridTrackKind::Length;
        return track;
    }
    if let Some(body) = function_body(&raw, "minmax") {
        track.kind = CssGridTrackKind::Minmax;
        track.structurally_editable =
            split_top_level_commas(body).is_some_and(|parts| parts.len() == 2);
        return track;
    }
    if function_body(&raw, "fit-content").is_some() {
        track.kind = CssGridTrackKind::FitContent;
        track.structurally_editable =
            function_body(&raw, "fit-content").is_some_and(|body| !body.trim().is_empty());
        return track;
    }
    if let Some(body) = function_body(&raw, "repeat") {
        track.kind = CssGridTrackKind::Repeat;
        if let Some(parts) = split_top_level_commas(body).filter(|parts| parts.len() == 2) {
            track.repeat_count = Some(parts[0].clone());
            if let Some(children) = split_top_level_whitespace(&parts[1]) {
                track.repeat_tracks = children
                    .iter()
                    .enumerate()
                    .map(|(child_index, child)| parse_track(child, child_index))
                    .collect();
            }
            track.structurally_editable = !track.repeat_tracks.is_empty()
                && track
                    .repeat_tracks
                    .iter()
                    .all(|child| child.structurally_editable);
        } else {
            track.structurally_editable = false;
        }
        return track;
    }
    track.kind = CssGridTrackKind::Opaque;
    track.structurally_editable = false;
    track
}

fn serialize_track(track: &CssGridTrack) -> String {
    if matches!(track.kind, CssGridTrackKind::Repeat)
        && track.repeat_count.is_some()
        && !track.repeat_tracks.is_empty()
    {
        return format!(
            "repeat({}, {})",
            track.repeat_count.as_deref().unwrap_or("2"),
            track
                .repeat_tracks
                .iter()
                .map(serialize_track)
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    track.raw.trim().to_string()
}

fn known_invalid_track(track: &CssGridTrack) -> bool {
    match track.kind {
        CssGridTrackKind::Minmax | CssGridTrackKind::FitContent => !track.structurally_editable,
        CssGridTrackKind::Repeat => {
            let count = track.repeat_count.as_deref().unwrap_or("").trim();
            let valid_count = matches!(count, "auto-fit" | "auto-fill")
                || count.parse::<u32>().is_ok_and(|value| value > 0)
                || is_dynamic(count);
            !valid_count
                || track.repeat_tracks.is_empty()
                || track.repeat_tracks.iter().any(known_invalid_track)
        }
        _ => false,
    }
}

fn empty_areas(raw: Option<String>) -> CssGridAreas {
    CssGridAreas {
        raw,
        rows: Vec::new(),
        valid: true,
        error: None,
        structurally_editable: true,
    }
}

fn parse_quoted_rows(value: &str) -> Result<Vec<Vec<String>>, String> {
    let mut rows = Vec::new();
    let mut chars = value.char_indices().peekable();
    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        if ch != '\'' && ch != '"' {
            return Err(
                "Fiecare rând din grid-template-areas trebuie să fie între ghilimele.".to_string(),
            );
        }
        let (_, quote) = chars.next().unwrap();
        let mut row = String::new();
        let mut escaped = false;
        let mut closed = false;
        while let Some((_, current)) = chars.next() {
            if escaped {
                row.push(current);
                escaped = false;
            } else if current == '\\' {
                escaped = true;
            } else if current == quote {
                closed = true;
                break;
            } else {
                row.push(current);
            }
        }
        if !closed {
            return Err(
                "Un rând din grid-template-areas nu are ghilimeaua de închidere.".to_string(),
            );
        }
        let cells = row
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        if cells.is_empty() {
            return Err("Un rând din grid-template-areas este gol.".to_string());
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err("grid-template-areas nu conține niciun rând.".to_string());
    }
    Ok(rows)
}

fn validate_area_rows(rows: &[Vec<String>]) -> Result<(), String> {
    let width = rows.first().map(Vec::len).unwrap_or(0);
    if width == 0 || rows.iter().any(|row| row.len() != width) {
        return Err(
            "Toate rândurile din grid-template-areas trebuie să aibă același număr de celule."
                .to_string(),
        );
    }
    let names = rows
        .iter()
        .flatten()
        .filter(|cell| !is_empty_area_cell(cell))
        .cloned()
        .collect::<BTreeSet<_>>();
    for name in names {
        if !valid_area_name(&name) {
            return Err(format!("Numele zonei „{name}” nu este valid."));
        }
        let mut cells = Vec::new();
        for (row, values) in rows.iter().enumerate() {
            for (column, value) in values.iter().enumerate() {
                if value == &name {
                    cells.push((row, column));
                }
            }
        }
        let min_row = cells.iter().map(|cell| cell.0).min().unwrap_or(0);
        let max_row = cells.iter().map(|cell| cell.0).max().unwrap_or(0);
        let min_column = cells.iter().map(|cell| cell.1).min().unwrap_or(0);
        let max_column = cells.iter().map(|cell| cell.1).max().unwrap_or(0);
        for row in min_row..=max_row {
            for column in min_column..=max_column {
                if rows[row][column] != name {
                    return Err(format!(
                        "Zona „{name}” trebuie să formeze un dreptunghi continuu."
                    ));
                }
            }
        }
    }
    Ok(())
}

fn is_empty_area_cell(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character == '.')
}

fn valid_area_name(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '-')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn meaningful_value(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn is_dynamic(value: &str) -> bool {
    let value = value.trim();
    value.starts_with('$') || value.starts_with("#{") || value.starts_with("var(")
}

fn is_length(value: &str) -> bool {
    value == "0"
        || [
            "px", "em", "rem", "%", "ch", "ex", "vw", "vh", "vmin", "vmax", "cm", "mm", "q", "in",
            "pc", "pt",
        ]
        .iter()
        .any(|unit| numeric_unit(value, unit))
        || ["calc(", "min(", "max(", "clamp(", "env("]
            .iter()
            .any(|prefix| value.starts_with(prefix))
}

fn numeric_unit(value: &str, unit: &str) -> bool {
    value
        .strip_suffix(unit)
        .is_some_and(|number| !number.is_empty() && number.parse::<f64>().is_ok())
}

fn function_body<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    let trimmed = value.trim();
    let prefix = format!("{name}(");
    trimmed.strip_prefix(&prefix)?.strip_suffix(')')
}

fn split_top_level_commas(value: &str) -> Option<Vec<String>> {
    split_top_level(value, true)
}
fn split_top_level_whitespace(value: &str) -> Option<Vec<String>> {
    split_top_level(value, false)
}

fn split_top_level(value: &str, commas: bool) -> Option<Vec<String>> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut stack = VecDeque::new();
    let mut quote = None;
    let mut escaped = false;
    for ch in value.chars() {
        if let Some(active) = quote {
            current.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            current.push(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => {
                stack.push_back(ch);
                current.push(ch);
            }
            ')' | ']' | '}' => {
                let expected = match ch {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                if stack.pop_back() != Some(expected) {
                    return None;
                }
                current.push(ch);
            }
            ',' if commas && stack.is_empty() => push_part(&mut result, &mut current, true)?,
            whitespace if !commas && whitespace.is_whitespace() && stack.is_empty() => {
                push_part(&mut result, &mut current, false)?;
            }
            _ => current.push(ch),
        }
    }
    if quote.is_some() || !stack.is_empty() {
        return None;
    }
    push_part(&mut result, &mut current, commas)?;
    (!result.is_empty()).then_some(result)
}

fn push_part(result: &mut Vec<String>, current: &mut String, require: bool) -> Option<()> {
    let part = current.trim();
    if !part.is_empty() {
        result.push(part.to_string());
    } else if require {
        return None;
    }
    current.clear();
    Some(())
}

fn has_top_level_comma(value: &str) -> bool {
    match split_top_level_commas(value) {
        Some(parts) => parts.len() > 1,
        None => value.contains(','),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn property(property: &str, value: &str) -> CssProperty {
        CssProperty {
            property: property.to_string(),
            value: value.to_string(),
        }
    }

    #[test]
    fn parses_and_serializes_advanced_track_lists_without_expanding_repeat() {
        let list = CssGridTrackList::parse(Some(
            "[start] minmax(0, 1fr) repeat(auto-fit, minmax(14rem, 1fr)) [end]",
        ));
        assert_eq!(list.mode, CssGridTrackListMode::Tracks);
        assert_eq!(list.tracks.len(), 4);
        assert_eq!(list.tracks[2].kind, CssGridTrackKind::Repeat);
        assert_eq!(list.tracks[2].repeat_count.as_deref(), Some("auto-fit"));
        assert_eq!(
            list.to_css().as_deref(),
            Some("[start] minmax(0, 1fr) repeat(auto-fit, minmax(14rem, 1fr)) [end]")
        );
    }

    #[test]
    fn preserves_dynamic_subgrid_and_unknown_values_as_non_destructive_modes() {
        let dynamic = CssGridTrackList::parse(Some("$coloane-proiect"));
        assert!(!dynamic.structurally_editable);
        assert_eq!(dynamic.to_css().as_deref(), Some("$coloane-proiect"));
        assert_eq!(
            CssGridTrackList::parse(Some("subgrid [a] [b]")).mode,
            CssGridTrackListMode::Subgrid
        );
    }

    #[test]
    fn areas_require_rectangular_rows_and_contiguous_named_rectangles() {
        let areas = CssGridAreas::parse(Some("\"hero hero side\" \"main main side\""));
        assert!(areas.valid);
        assert_eq!(areas.rows.len(), 2);
        assert_eq!(
            areas.to_css().as_deref(),
            Some("\"hero hero side\" \"main main side\"")
        );
        assert!(!CssGridAreas::parse(Some("\"a a\" \"a .\"")).valid);
        assert!(!CssGridAreas::parse(Some("\"a a\" \"b\"")).valid);
    }

    #[test]
    fn rule_projection_cascades_gap_and_round_trips_longhands() {
        let model = CssGrid::from_rules(&[
            property("display", "grid"),
            property("grid-template-columns", "repeat(3, 1fr)"),
            property("gap", "$space-m 2rem"),
            property("grid-template-areas", "\"a b c\""),
        ]);
        assert_eq!(model.row_gap.as_deref(), Some("$space-m"));
        assert_eq!(model.column_gap.as_deref(), Some("2rem"));
        let reparsed = CssGrid::from_declarations(&model.to_longhands());
        assert_eq!(
            reparsed.template_columns.to_css(),
            model.template_columns.to_css()
        );
    }

    #[test]
    fn mutation_normalization_rejects_invalid_areas_and_top_level_track_commas() {
        let invalid_areas = HashMap::from([(
            "grid-template-areas".to_string(),
            "\"a a\" \"a .\"".to_string(),
        )]);
        assert!(normalize_grid_properties(&invalid_areas).is_err());
        let invalid_tracks =
            HashMap::from([("grid-template-columns".to_string(), "1fr, 2fr".to_string())]);
        assert!(normalize_grid_properties(&invalid_tracks).is_err());
        for value in [
            "minmax(1fr)",
            "fit-content()",
            "repeat(0, 1fr)",
            "repeat(2, )",
        ] {
            let invalid = HashMap::from([("grid-template-columns".to_string(), value.to_string())]);
            assert!(normalize_grid_properties(&invalid).is_err(), "{value}");
        }
    }
}
