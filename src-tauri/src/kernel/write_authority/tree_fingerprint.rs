use crate::kernel::file_buffer_store::hash_bytes;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct TreeFingerprintRecord {
    pub relative_path: String,
    pub kind: u8,
    pub version_token: String,
}

/// Hashes the canonical descendant inventory captured by the descriptor-based
/// WriteAuthority capability. No path-side duplicate scanner exists.
pub(crate) fn tree_fingerprint_from_records(mut records: Vec<TreeFingerprintRecord>) -> String {
    records.sort();
    let estimated = records.iter().fold(0_usize, |total, record| {
        total
            .saturating_add(record.relative_path.len())
            .saturating_add(record.version_token.len())
            .saturating_add(4)
    });
    let mut canonical = Vec::with_capacity(estimated);
    for record in records {
        canonical.push(record.kind);
        canonical.push(0);
        canonical.extend_from_slice(record.relative_path.as_bytes());
        canonical.push(0);
        canonical.extend_from_slice(record.version_token.as_bytes());
        canonical.push(b'\n');
    }
    hash_bytes(&canonical)
}

#[cfg(test)]
mod tests {
    use super::{tree_fingerprint_from_records, TreeFingerprintRecord};

    #[test]
    fn fingerprint_is_order_independent_but_identity_sensitive() {
        let first = TreeFingerprintRecord {
            relative_path: "a/file.txt".to_string(),
            kind: b'f',
            version_token: "v1".to_string(),
        };
        let second = TreeFingerprintRecord {
            relative_path: "b".to_string(),
            kind: b'd',
            version_token: "v2".to_string(),
        };
        assert_eq!(
            tree_fingerprint_from_records(vec![first.clone(), second.clone()]),
            tree_fingerprint_from_records(vec![second.clone(), first.clone()])
        );
        assert_ne!(
            tree_fingerprint_from_records(vec![first, second.clone()]),
            tree_fingerprint_from_records(vec![TreeFingerprintRecord {
                version_token: "changed".to_string(),
                ..second
            }])
        );
    }
}
