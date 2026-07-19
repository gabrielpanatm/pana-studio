use crate::source_graph::model::SourceRange;

pub(super) fn source_range(source: &str, start: usize, end: usize) -> SourceRange {
    let (line, column) = line_column_at(source, start);
    let (end_line, end_column) = line_column_at(source, end);
    SourceRange {
        start,
        end,
        line,
        column,
        end_line,
        end_column,
    }
}

fn line_column_at(source: &str, offset: usize) -> (usize, usize) {
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
