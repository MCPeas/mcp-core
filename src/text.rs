// SPDX-FileCopyrightText: 2025-2026 Stefan Grönke <stefan@gronke.net>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Character-boundary-safe text slicing.
//!
//! MCP servers chunk large payloads by byte budget while keeping UTF-8 valid
//! and, where possible, breaking on a word boundary. These helpers give a
//! shared, tested implementation so consumers stop hand-rolling
//! `is_char_boundary` loops.

/// Largest char boundary of `s` at or below `index` (clamped to `s.len()`).
pub fn floor_char_boundary(s: &str, mut index: usize) -> usize {
    if index >= s.len() {
        s.len()
    } else {
        while index > 0 && !s.is_char_boundary(index) {
            index -= 1;
        }
        index
    }
}

/// Smallest char boundary of `s` at or above `index` (clamped to `s.len()`).
pub fn ceil_char_boundary(s: &str, mut index: usize) -> usize {
    while index < s.len() && !s.is_char_boundary(index) {
        index += 1;
    }
    index.min(s.len())
}

/// Take a char-boundary-safe slice of `text` starting at byte `offset` and
/// spanning about `budget` bytes.
///
/// `offset` is snapped down and the end snapped up to the nearest char
/// boundaries, so the returned `&str` is always valid UTF-8 and never splits a
/// character. The second element is the byte offset to continue from — `Some`
/// when more text remains, `None` when the slice reached the end. An `offset`
/// past the end yields an empty slice and `None`.
///
/// ```
/// use mcp_core::text::char_safe_chunk;
/// let (a, next) = char_safe_chunk("äbcd", 0, 3);
/// // "ä" is two bytes; the window rounds up to a boundary rather than splitting it.
/// assert_eq!(a, "äb");
/// let (b, end) = char_safe_chunk("äbcd", next.unwrap(), 3);
/// assert_eq!(b, "cd");
/// assert_eq!(end, None);
/// ```
pub fn char_safe_chunk(text: &str, offset: usize, budget: usize) -> (&str, Option<usize>) {
    let start = floor_char_boundary(text, offset.min(text.len()));
    let end = ceil_char_boundary(text, start.saturating_add(budget).min(text.len()));
    let next = if end < text.len() { Some(end) } else { None };
    (&text[start..end], next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_snap_around_multibyte_chars() {
        // "ä" occupies bytes 0..2.
        assert_eq!(floor_char_boundary("äbc", 1), 0);
        assert_eq!(ceil_char_boundary("äbc", 1), 2);
        // Indices at or past the end clamp to len.
        assert_eq!(floor_char_boundary("abc", 9), 3);
        assert_eq!(ceil_char_boundary("abc", 9), 3);
        // Already on a boundary: unchanged.
        assert_eq!(floor_char_boundary("abc", 2), 2);
        assert_eq!(ceil_char_boundary("abc", 2), 2);
    }

    #[test]
    fn chunk_never_splits_a_char_and_reports_continuation() {
        let text = "äöü12345"; // three 2-byte chars, then five 1-byte
        let (first, next) = char_safe_chunk(text, 0, 5);
        assert!(text.is_char_boundary(0));
        assert!(first.chars().count() >= 1);
        // The window (budget 5) lands mid-char and rounds up to a boundary.
        let n = next.expect("more text remains");
        assert!(text.is_char_boundary(n));
        // Continuing from `next` covers the rest without overlap or loss.
        let (second, _) = char_safe_chunk(text, n, text.len());
        assert_eq!(format!("{first}{second}"), text);
    }

    #[test]
    fn exact_fit_and_past_end() {
        let (all, next) = char_safe_chunk("abc", 0, 3);
        assert_eq!(all, "abc");
        assert_eq!(next, None);

        let (empty, next) = char_safe_chunk("abc", 10, 5);
        assert_eq!(empty, "");
        assert_eq!(next, None);
    }
}
