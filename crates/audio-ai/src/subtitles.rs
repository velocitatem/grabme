//! Subtitle generation in SRT and VTT formats.

use crate::transcription::TranscriptionSegment;
use grabme_common::error::GrabmeResult;

/// Generate SRT subtitle content from transcription segments.
pub fn generate_srt(segments: &[TranscriptionSegment]) -> String {
    let mut output = String::new();

    for (i, segment) in segments.iter().enumerate() {
        output.push_str(&format!("{}\n", i + 1));
        output.push_str(&format!(
            "{} --> {}\n",
            format_srt_time(segment.start_secs),
            format_srt_time(segment.end_secs),
        ));
        output.push_str(&segment.text);
        output.push_str("\n\n");
    }

    output
}

/// Generate WebVTT subtitle content from transcription segments.
pub fn generate_vtt(segments: &[TranscriptionSegment]) -> String {
    let mut output = String::from("WEBVTT\n\n");

    for segment in segments {
        output.push_str(&format!(
            "{} --> {}\n",
            format_vtt_time(segment.start_secs),
            format_vtt_time(segment.end_secs),
        ));
        output.push_str(&segment.text);
        output.push_str("\n\n");
    }

    output
}

/// Format seconds as SRT timestamp: HH:MM:SS,mmm
fn format_srt_time(secs: f64) -> String {
    let total_ms = (secs * 1000.0) as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let seconds = (total_ms % 60_000) / 1000;
    let millis = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

/// Format seconds as VTT timestamp: HH:MM:SS.mmm
fn format_vtt_time(secs: f64) -> String {
    let total_ms = (secs * 1000.0) as u64;
    let hours = total_ms / 3_600_000;
    let minutes = (total_ms % 3_600_000) / 60_000;
    let seconds = (total_ms % 60_000) / 1000;
    let millis = total_ms % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

/// Save subtitles to a file.
pub fn save_subtitles(
    segments: &[TranscriptionSegment],
    path: &std::path::Path,
) -> GrabmeResult<()> {
    let content = match path.extension().and_then(|e| e.to_str()) {
        Some("vtt") => generate_vtt(segments),
        _ => generate_srt(segments), // default to SRT
    };
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srt_generation() {
        let segments = vec![
            TranscriptionSegment {
                start_secs: 0.0,
                end_secs: 2.5,
                text: "Hello world".to_string(),
                confidence: Some(0.95),
            },
            TranscriptionSegment {
                start_secs: 3.0,
                end_secs: 5.0,
                text: "This is a test".to_string(),
                confidence: None,
            },
        ];

        let srt = generate_srt(&segments);
        assert!(srt.contains("1\n00:00:00,000 --> 00:00:02,500\nHello world"));
        assert!(srt.contains("2\n00:00:03,000 --> 00:00:05,000\nThis is a test"));
    }

    #[test]
    fn test_vtt_generation() {
        let segments = vec![TranscriptionSegment {
            start_secs: 61.5,
            end_secs: 63.0,
            text: "One minute in".to_string(),
            confidence: None,
        }];

        let vtt = generate_vtt(&segments);
        assert!(vtt.starts_with("WEBVTT\n"));
        assert!(vtt.contains("00:01:01.500 --> 00:01:03.000"));
    }

    #[test]
    fn test_time_formatting() {
        assert_eq!(format_srt_time(0.0), "00:00:00,000");
        assert_eq!(format_srt_time(3661.5), "01:01:01,500");
        assert_eq!(format_vtt_time(3661.5), "01:01:01.500");
    }
}
