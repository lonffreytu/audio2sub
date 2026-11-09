use crate::model::Segment;
use regex::Regex;
use std::sync::OnceLock;

const MAX_CHARS: usize = 20;
const HARD_SPLIT: usize = 18;

fn sentence_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"[。！？!?\n]+").expect("valid regex"))
}

fn clause_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"[，,。！？!?\n]+").expect("valid regex"))
}

fn comma_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"[，,、；;]+").expect("valid regex"))
}

fn char_len(value: &str) -> usize {
    value.chars().count()
}

pub fn split_text_to_segments(text: &str, duration_ms: i64) -> Vec<Segment> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }

    let mut sentences = split_trimmed(sentence_pattern(), text);
    if sentences.len() <= 1 {
        sentences = split_trimmed(clause_pattern(), text);
    }

    let mut parts = Vec::new();
    for sentence in sentences {
        if char_len(&sentence) <= MAX_CHARS {
            parts.push(sentence);
            continue;
        }

        let clauses = split_trimmed(comma_pattern(), &sentence);
        if clauses.len() <= 1 {
            if sentence.chars().any(|ch| ch.is_ascii_alphabetic())
                && (sentence.contains(' ') || sentence.contains('-'))
            {
                merge_words(&sentence, &mut parts);
            } else {
                let chars: Vec<char> = sentence.chars().collect();
                for chunk in chars.chunks(HARD_SPLIT) {
                    parts.push(chunk.iter().collect());
                }
            }
        } else {
            merge_clauses(clauses, &mut parts);
        }
    }

    parts.retain(|part| !part.trim().is_empty());
    if parts.is_empty() {
        parts.push(text.to_string());
    }

    allocate_timeline(&parts, duration_ms)
}

fn split_trimmed(pattern: &Regex, text: &str) -> Vec<String> {
    pattern
        .split(text)
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn merge_words(sentence: &str, parts: &mut Vec<String>) {
    let mut buffer = String::new();
    for word in sentence.split_whitespace() {
        let separator = usize::from(!buffer.is_empty());
        if char_len(&buffer) + char_len(word) + separator <= MAX_CHARS {
            if !buffer.is_empty() {
                buffer.push(' ');
            }
            buffer.push_str(word);
        } else {
            if !buffer.is_empty() {
                parts.push(std::mem::take(&mut buffer));
            }
            buffer.push_str(word);
        }
    }
    if !buffer.is_empty() {
        parts.push(buffer);
    }
}

fn merge_clauses(clauses: Vec<String>, parts: &mut Vec<String>) {
    let mut buffer = String::new();
    for clause in clauses {
        let separator = usize::from(!buffer.is_empty());
        if char_len(&buffer) + char_len(&clause) + separator <= MAX_CHARS {
            if !buffer.is_empty() {
                buffer.push('，');
            }
            buffer.push_str(&clause);
        } else {
            if !buffer.is_empty() {
                parts.push(std::mem::take(&mut buffer));
            }
            buffer.push_str(&clause);
        }
    }
    if !buffer.is_empty() {
        parts.push(buffer);
    }
}

fn allocate_timeline(parts: &[String], duration_ms: i64) -> Vec<Segment> {
    let total_chars: usize = parts.iter().map(|part| char_len(part)).sum();
    let total_ms = duration_ms.max(0);
    let mut current_ms = 0;
    let mut segments = Vec::with_capacity(parts.len());

    for (index, part) in parts.iter().enumerate() {
        let start_ms = current_ms;
        let end_ms = if index + 1 == parts.len() {
            total_ms
        } else if total_chars == 0 {
            start_ms
        } else {
            start_ms + total_ms * char_len(part) as i64 / total_chars as i64
        }
        .max(start_ms);
        segments.push(Segment::new(start_ms, end_ms, part.clone()));
        current_ms = end_ms;
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_returns_empty() {
        assert!(split_text_to_segments("   ", 1_000).is_empty());
    }

    #[test]
    fn short_text_fills_duration() {
        let segments = split_text_to_segments("你好世界", 1_000);
        assert_eq!(segments, vec![Segment::new(0, 1_000, "你好世界")]);
    }

    #[test]
    fn splits_by_sentence_and_keeps_timeline_monotonic() {
        let segments = split_text_to_segments("你好。世界！", 2_000);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], Segment::new(0, 1_000, "你好"));
        assert_eq!(segments[1], Segment::new(1_000, 2_000, "世界"));
    }

    #[test]
    fn hard_splits_long_text_without_punctuation() {
        let text = "汉".repeat(40);
        let segments = split_text_to_segments(&text, 4_000);
        assert_eq!(segments.len(), 3);
        assert_eq!(char_len(&segments[0].text), 18);
        assert_eq!(char_len(&segments[1].text), 18);
        assert_eq!(char_len(&segments[2].text), 4);
        assert_eq!(segments.first().unwrap().start_ms, 0);
        assert_eq!(segments.last().unwrap().end_ms, 4_000);
    }

    #[test]
    fn merges_short_clauses() {
        let segments = split_text_to_segments(
            "第一句，第二句，第三句，第四句，第五句，第六句，第七句，第八句，第九句，第十句",
            10_000,
        );
        assert!(segments
            .iter()
            .all(|segment| char_len(&segment.text) <= MAX_CHARS));
        assert!(segments
            .windows(2)
            .all(|pair| pair[0].end_ms == pair[1].start_ms));
        assert_eq!(segments.last().unwrap().end_ms, 10_000);
    }

    #[test]
    fn splits_english_words() {
        let text = "this is a long english sentence that should be split into small subtitle lines by words";
        let segments = split_text_to_segments(text, 5_000);
        assert!(segments.len() > 1);
        assert!(segments
            .iter()
            .all(|segment| char_len(&segment.text) <= MAX_CHARS));
        assert!(segments
            .iter()
            .all(|segment| !segment.text.starts_with(' ')));
    }

    #[test]
    fn last_segment_reaches_total_duration() {
        let segments = split_text_to_segments("你好。世界。再见。", 3_000);
        assert_eq!(segments.last().unwrap().end_ms, 3_000);
        assert_eq!(segments.first().unwrap().start_ms, 0);
    }
}
