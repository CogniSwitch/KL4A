//! Unicode code-point (scalar) offset utilities. Every `start_pos`/`end_pos` the
//! bundle format stores is a Python `str`-index, i.e. a code-point offset into the
//! normalized Markdown text -- never a byte offset. Rust's `str`/regex APIs are
//! byte-offset based, so anything that records such offsets must convert through
//! here (used by `sopkb-core::normalize::extract_sections` in Phase 2).

/// Precomputed byte<->char offset table, built once per source string and reused
/// across many lookups (one per heading match in `extract_sections`, for example).
pub struct CharIndex {
    /// `byte_offsets[i]` = byte offset of the i-th Unicode scalar value; one extra
    /// sentinel entry equal to the total byte length represents "end of string".
    byte_offsets: Vec<usize>,
}

impl CharIndex {
    pub fn new(s: &str) -> Self {
        let mut byte_offsets: Vec<usize> = s.char_indices().map(|(b, _)| b).collect();
        byte_offsets.push(s.len());
        Self { byte_offsets }
    }

    /// Number of Unicode scalar values in the source string (Python `len(s)`-equivalent).
    pub fn char_len(&self) -> usize {
        self.byte_offsets.len() - 1
    }

    /// Convert a byte offset (must land on a char boundary, true for any offset
    /// derived from `str`/regex match positions) to a code-point offset.
    pub fn char_offset_at_byte(&self, byte_offset: usize) -> usize {
        self.byte_offsets.binary_search(&byte_offset).unwrap_or_else(|insert_at| insert_at)
    }

    /// Convert a code-point offset back to a byte offset, for slicing the original `&str`.
    pub fn byte_offset_at_char(&self, char_offset: usize) -> usize {
        self.byte_offsets[char_offset]
    }
}

/// One-shot byte->char conversion when only a single lookup is needed (avoids
/// building a `CharIndex`).
pub fn byte_to_char_offset(s: &str, byte_offset: usize) -> usize {
    s[..byte_offset].chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_offsets_are_identity() {
        let idx = CharIndex::new("hello world");
        assert_eq!(idx.char_len(), 11);
        assert_eq!(idx.char_offset_at_byte(6), 6);
        assert_eq!(idx.byte_offset_at_char(6), 6);
    }

    #[test]
    fn multibyte_chars_diverge_byte_from_char_offset() {
        // "café" -- 'é' is 2 bytes in UTF-8 but 1 char.
        let s = "café x";
        let idx = CharIndex::new(s);
        assert_eq!(idx.char_len(), 6); // c,a,f,é,space,x
        // byte offset of 'x' is 6 (c=1,a=1,f=1,é=2,space=1 -> 6), char offset of 'x' is 5.
        assert_eq!(idx.char_offset_at_byte(6), 5);
        assert_eq!(idx.byte_offset_at_char(5), 6);
        assert_eq!(byte_to_char_offset(s, 6), 5);
    }
}
