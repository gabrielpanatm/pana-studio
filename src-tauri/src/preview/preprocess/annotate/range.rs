pub(super) struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub(super) fn new(source: &str) -> Self {
        let mut line_starts = Vec::with_capacity(source.lines().count().saturating_add(1));
        line_starts.push(0);
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self { line_starts }
    }

    pub(super) fn line_column(&self, source: &str, offset: usize) -> (usize, usize) {
        let offset = offset.min(source.len());
        let line_index = self.line_starts.partition_point(|start| *start <= offset);
        let line_index = line_index.saturating_sub(1);
        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        let column = source
            .get(line_start..offset)
            .map(|segment| segment.chars().count().saturating_add(1))
            .unwrap_or(1);
        (line_index.saturating_add(1), column)
    }
}

#[cfg(test)]
mod tests {
    use super::LineIndex;

    #[test]
    fn line_index_preserves_unicode_line_and_column_coordinates() {
        let source = "unu\nțară\nfinal";
        let index = LineIndex::new(source);

        assert_eq!(index.line_column(source, 0), (1, 1));
        assert_eq!(index.line_column(source, 4), (2, 1));
        assert_eq!(index.line_column(source, 4 + "ța".len()), (2, 3));
        assert_eq!(index.line_column(source, source.len()), (3, 6));
    }
}
