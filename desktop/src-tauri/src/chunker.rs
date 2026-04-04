pub const CHUNK_SIZE: usize = 256;
pub const CHUNK_OVERLAP: usize = 32;

pub struct Chunk {
    pub start_token: usize,
    pub end_token: usize,
    pub text: String,
}

/// Split `text` into overlapping windows.
///
/// - `chunk_size`: number of whitespace-delimited tokens per window.
/// - `overlap`: how many tokens the next window re-uses from the current one.
///
/// Panics if `overlap >= chunk_size` — that is a programming error.
pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<Chunk> {
    assert!(overlap < chunk_size, "overlap must be less than chunk_size");

    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }

    let step = chunk_size - overlap;
    let mut chunks = Vec::new();
    let mut start = 0;

    loop {
        let end = (start + chunk_size).min(tokens.len());
        chunks.push(Chunk {
            start_token: start,
            end_token: end,
            text: tokens[start..end].join(" "),
        });
        if end == tokens.len() {
            break;
        }
        start += step;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(text: &str) -> Vec<String> {
        text.split_whitespace().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_empty_text_returns_empty() {
        assert!(chunk_text("", 8, 2).is_empty());
        assert!(chunk_text("   \t\n  ", 8, 2).is_empty());
    }

    #[test]
    fn test_single_chunk_when_text_shorter_than_window() {
        let chunks = chunk_text("hello world foo", 8, 2);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start_token, 0);
        assert_eq!(chunks[0].end_token, 3);
        assert_eq!(chunks[0].text, "hello world foo");
    }

    #[test]
    fn test_overlap_applied_correctly() {
        // 10 tokens, size=4, overlap=2 → step=2
        // chunk 0: tokens 0-4
        // chunk 1: tokens 2-6
        // chunk 2: tokens 4-8
        // chunk 3: tokens 6-10
        let input = "a b c d e f g h i j"; // 10 tokens
        let chunks = chunk_text(input, 4, 2);
        assert_eq!(chunks.len(), 4);

        assert_eq!(chunks[0].start_token, 0);
        assert_eq!(chunks[0].end_token, 4);
        assert_eq!(chunks[0].text, "a b c d");

        assert_eq!(chunks[1].start_token, 2);
        assert_eq!(chunks[1].end_token, 6);
        assert_eq!(chunks[1].text, "c d e f");

        // Adjacent chunks share `overlap` tokens
        let t = tokens(input);
        let end_of_0 = &t[chunks[0].start_token..chunks[0].end_token];
        let start_of_1 = &t[chunks[1].start_token..chunks[1].end_token];
        let shared: Vec<_> = end_of_0
            .iter()
            .rev()
            .take(2)
            .collect();
        let leading: Vec<_> = start_of_1.iter().take(2).collect();
        assert_eq!(shared.into_iter().rev().collect::<Vec<_>>(), leading);
    }

    #[test]
    fn test_last_chunk_covers_remainder() {
        // 7 tokens, size=4, overlap=1 → step=3
        // chunk 0: 0-4
        // chunk 1: 3-7 (last, covers up to end)
        let input = "a b c d e f g";
        let chunks = chunk_text(input, 4, 1);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[1].end_token, 7);
        assert_eq!(chunks[1].text, "d e f g");
    }

    #[test]
    fn test_whitespace_normalization() {
        // tabs and newlines are treated identically to spaces
        let input = "foo\tbar\nbaz  qux";
        let chunks = chunk_text(input, 8, 2);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "foo bar baz qux");
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(CHUNK_SIZE, 256);
        assert_eq!(CHUNK_OVERLAP, 32);
        assert!(CHUNK_OVERLAP < CHUNK_SIZE);
    }

    #[test]
    #[should_panic(expected = "overlap must be less than chunk_size")]
    fn test_panic_when_overlap_equals_chunk_size() {
        chunk_text("a b c", 4, 4);
    }

    #[test]
    fn test_exactly_one_token() {
        let chunks = chunk_text("hello", 256, 32);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello");
        assert_eq!(chunks[0].start_token, 0);
        assert_eq!(chunks[0].end_token, 1);
    }
}
