use crate::model::{Segment, SubtitleFormat};

pub fn format_srt_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

pub fn format_vtt_timestamp(ms: i64) -> String {
    let ms = ms.max(0);
    let hours = ms / 3_600_000;
    let minutes = (ms % 3_600_000) / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let millis = ms % 1_000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

pub fn render_srt(segments: &[Segment]) -> String {
    let mut output = String::new();
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(&(index + 1).to_string());
        output.push('\n');
        output.push_str(&format_srt_timestamp(segment.start_ms));
        output.push_str(" --> ");
        output.push_str(&format_srt_timestamp(segment.end_ms));
        output.push('\n');
        output.push_str(segment.text.trim());
        output.push('\n');
    }
    output
}

pub fn render_vtt(segments: &[Segment]) -> String {
    let mut output = String::from("WEBVTT\n\n");
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(&format_vtt_timestamp(segment.start_ms));
        output.push_str(" --> ");
        output.push_str(&format_vtt_timestamp(segment.end_ms));
        output.push('\n');
        output.push_str(segment.text.trim());
        output.push('\n');
    }
    output
}

pub fn render_txt(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|segment| segment.text.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render(segments: &[Segment], format: SubtitleFormat) -> String {
    match format {
        SubtitleFormat::Srt => render_srt(segments),
        SubtitleFormat::Vtt => render_vtt(segments),
        SubtitleFormat::Txt => render_txt(segments),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Segment> {
        vec![
            Segment::new(0, 1_200, "你好"),
            Segment::new(1_200, 3_723_456, "世界"),
        ]
    }

    #[test]
    fn formats_srt_timestamp() {
        assert_eq!(format_srt_timestamp(0), "00:00:00,000");
        assert_eq!(format_srt_timestamp(3_723_456), "01:02:03,456");
        assert_eq!(format_srt_timestamp(-10), "00:00:00,000");
    }

    #[test]
    fn formats_vtt_timestamp() {
        assert_eq!(format_vtt_timestamp(3_723_456), "01:02:03.456");
    }

    #[test]
    fn renders_srt() {
        let rendered = render_srt(&sample());
        assert_eq!(
            rendered,
            "1\n00:00:00,000 --> 00:00:01,200\n你好\n\n2\n00:00:01,200 --> 01:02:03,456\n世界\n"
        );
    }

    #[test]
    fn renders_vtt() {
        let rendered = render_vtt(&sample());
        assert!(rendered.starts_with("WEBVTT\n\n"));
        assert!(rendered.contains("00:00:00.000 --> 00:00:01.200\n你好"));
    }

    #[test]
    fn renders_txt() {
        assert_eq!(render_txt(&sample()), "你好\n世界");
    }

    #[test]
    fn parses_formats() {
        assert_eq!(SubtitleFormat::parse("SRT"), Some(SubtitleFormat::Srt));
        assert_eq!(SubtitleFormat::parse("vtt"), Some(SubtitleFormat::Vtt));
        assert_eq!(SubtitleFormat::parse("ass"), None);
    }
}
