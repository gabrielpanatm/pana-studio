use crate::source_graph::model::SourceRange;

pub(super) fn source_range(source: &str, start: usize, end: usize) -> SourceRange {
    let start = start.min(source.len());
    let end = end.min(source.len());
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
