//! Integration tests through the public library API (one test binary, as the rulebook asks).

use noname::img;
use ratatui::style::Style;

#[test]
fn mime_and_ext_agree_for_known_images() {
    let png = [0x89, b'P', b'N', b'G', 0, 0];
    let gif = *b"GIF89a";
    assert_eq!(img::mime(&png), Some("image/png"));
    assert_eq!(img::ext(img::mime(&png).unwrap()), "png");
    assert_eq!(img::mime(&gif), Some("image/gif"));
    assert_eq!(img::ext(img::mime(&gif).unwrap()), "gif");
    assert_eq!(img::mime(b"not an image"), None);
}

#[test]
fn base64_pads_like_the_standard() {
    assert_eq!(img::b64(b""), "");
    assert_eq!(img::b64(b"M"), "TQ==");
    assert_eq!(img::b64(b"Ma"), "TWE=");
    assert_eq!(img::b64(b"Man"), "TWFu");
}

#[test]
fn wrapped_lines_never_exceed_the_width() {
    let text = "the quick brown fox jumps over the lazy dog".to_string();
    let lines = noname::wsegs(&[(text.clone(), Style::default())], 10);
    assert!(lines.len() > 1);
    for l in &lines {
        let w: usize = l.iter().map(|s| s.content.chars().count()).sum();
        assert!(w <= 10, "line too wide: {w}");
    }
    // wrapping may drop the spaces at a break, but never a letter
    let joined: String = lines
        .iter()
        .flatten()
        .flat_map(|s| s.content.chars())
        .filter(|c| !c.is_whitespace())
        .collect();
    assert_eq!(joined, text.replace(' ', ""));
}
