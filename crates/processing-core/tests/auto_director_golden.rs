use std::path::PathBuf;

use grabme_processing_core::auto_zoom::AutoZoomAnalyzer;
use grabme_processing_core::vertical::{generate_vertical_timeline, VerticalConfig};
use grabme_project_model::event::parse_events;

fn load_fixture_events() -> Vec<grabme_project_model::event::InputEvent> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("sample-project")
        .join("meta")
        .join("events.jsonl");

    let content = std::fs::read_to_string(path).expect("fixture events should be readable");
    let jsonl = content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })
        .collect::<Vec<_>>()
        .join("\n");

    parse_events(&jsonl).expect("fixture events should parse")
}

fn fnv1a_64(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[test]
fn auto_director_default_fixture_signature_is_stable() {
    let events = load_fixture_events();
    let analyzer = AutoZoomAnalyzer::with_defaults();
    let timeline = analyzer.analyze(&events);

    let signature = timeline
        .keyframes
        .iter()
        .map(|kf| {
            format!(
                "{:.3}|{:.6}|{:.6}|{:.6}|{:.6}",
                kf.time_secs, kf.viewport.x, kf.viewport.y, kf.viewport.w, kf.viewport.h
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(timeline.keyframes.len(), 201);
    assert_eq!(fnv1a_64(&signature), 0xbc5e0d057ef2da2f);
}

#[test]
fn vertical_mode_fixture_keeps_nine_by_sixteen() {
    let events = load_fixture_events();
    let keyframes = generate_vertical_timeline(&events, &VerticalConfig::default());

    assert!(!keyframes.is_empty());
    for keyframe in keyframes {
        let ratio = keyframe.viewport.w / keyframe.viewport.h;
        assert!((ratio - 9.0 / 16.0).abs() < 0.001);
    }
}
