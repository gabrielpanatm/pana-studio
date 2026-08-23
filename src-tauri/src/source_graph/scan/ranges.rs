use crate::source_graph::model::SourceRange;

pub(crate) struct SourceRangeIndex<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceRangeIndex<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        let mut line_starts = Vec::with_capacity(source.lines().count().saturating_add(1));
        line_starts.push(0);
        line_starts.extend(source.match_indices('\n').map(|(index, _)| index + 1));
        Self {
            source,
            line_starts,
        }
    }

    pub(crate) fn range(&self, start: usize, end: usize) -> SourceRange {
        let start = start.min(self.source.len());
        let end = end.min(self.source.len());
        let (line, column) = self.line_column(start);
        let (end_line, end_column) = self.line_column(end);
        SourceRange {
            start,
            end,
            line,
            column,
            end_line,
            end_column,
        }
    }

    fn line_column(&self, offset: usize) -> (usize, usize) {
        let line_index = self
            .line_starts
            .partition_point(|line_start| *line_start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line_index];
        let column = self
            .source
            .get(line_start..offset)
            .map(|line| line.chars().count() + 1)
            .unwrap_or_else(|| line_column(self.source, offset).1);
        (line_index + 1, column)
    }
}

pub(crate) fn source_range(source: &str, start: usize, end: usize) -> SourceRange {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_ranges_preserve_unicode_line_and_column_semantics() {
        let source = "Acasă\nȘir UTF-8\nfinal";
        let index = SourceRangeIndex::new(source);
        for offset in source
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(source.len()))
        {
            assert_eq!(
                index.range(offset, offset),
                source_range(source, offset, offset)
            );
        }
    }
}
