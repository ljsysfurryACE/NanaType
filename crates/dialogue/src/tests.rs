use crate::{
    AfterLine, DialogueDocument, HISTORY_CAP, HistoryEntry, ImageAtom, ImageData, ImageFrame,
    ImageKind, Motion, Phase, Playback, PlaybackConfig, SessionHistory, TextStyle, is_cjk_grapheme,
};
use std::sync::Arc;

fn typed(text: &str) -> DialogueDocument {
    let mut document = DialogueDocument::new();
    document.insert_text(text);
    document
}

fn hold_config() -> PlaybackConfig {
    PlaybackConfig {
        simulate_pop: false,
        ..PlaybackConfig::default()
    }
}

#[test]
fn document_edits_split_graphemes_style_and_undo() {
    let mut document = DialogueDocument::new();
    document.insert_text("你a");
    assert_eq!(document.len(), 2);
    assert_eq!(document.plain_text(), "你a");
    assert!(is_cjk_grapheme("你"));
    assert!(!is_cjk_grapheme("a"));

    document.select_all();
    document.map_style(|style| style.bold = true);
    assert!(document.pieces().iter().all(|piece| match piece {
        crate::Piece::Grapheme { style, .. } => style.bold,
        _ => false,
    }));

    document.set_caret(document.len(), false);
    document.insert_newline();
    document.insert_text("b");
    assert_eq!(document.plain_text(), "你a\nb");

    document.insert_wait(400);
    document.insert_speed(20);
    document.insert_instant();
    document.insert_pause();
    assert_eq!(document.plain_text(), "你a\nb");
    assert!(document.pieces().iter().any(|piece| piece.is_marker()));

    assert!(document.undo());
    assert!(
        !document
            .pieces()
            .iter()
            .any(|piece| matches!(piece, crate::Piece::Pause))
    );
    while document.undo() {}
    assert!(document.is_empty());
    assert!(document.redo());
    assert!(!document.plain_text().is_empty());
}

#[test]
fn selection_delete_and_pending_style_apply_to_later_typing() {
    let mut document = typed("abcd");
    document.set_caret(1, false);
    document.set_caret(3, true);
    document.backspace();
    assert_eq!(document.plain_text(), "ad");

    document.map_style(|style| {
        style.italic = true;
        style.motion = Motion::Wave;
    });
    document.insert_text("Z");
    assert_eq!(document.plain_text(), "aZd");
    match &document.pieces()[1] {
        crate::Piece::Grapheme { cluster, style } => {
            assert_eq!(cluster, "Z");
            assert!(style.italic);
            assert_eq!(style.motion, Motion::Wave);
        }
        _ => panic!("expected grapheme"),
    }
}

#[test]
fn image_is_not_copied_as_text() {
    let mut document = typed("hi");
    document.insert_image(ImageAtom {
        kind: ImageKind::Clipboard,
        label: "剪贴板".into(),
        height_em: 1.5,
        data: Arc::new(ImageData {
            width: 1,
            height: 1,
            frames: vec![ImageFrame {
                rgba: vec![255, 0, 0, 255],
                delay_ms: 0,
            }],
        }),
    });
    assert_eq!(document.plain_text(), "hi");
    assert_eq!(document.selected_plain_text(), "hi");
}

#[test]
fn reveal_clock_waits_a_beat_and_cjk_waits_two() {
    let mut play = Playback::start(typed("a你b"), "", hold_config(), 0);
    assert_eq!(play.tick(0), 1);
    assert_eq!(play.cursor(), 1);
    assert_eq!(play.tick(49), 0);
    assert_eq!(play.tick(50), 1);
    assert!(matches!(
        &play.revealed()[1],
        crate::Piece::Grapheme { cluster, .. } if cluster == "你"
    ));
    assert_eq!(play.tick(149), 0);
    assert_eq!(play.tick(150), 1);
    assert_eq!(play.phase(), Phase::Revealing);
    assert_eq!(play.tick(200), 0);
    assert_eq!(play.phase(), Phase::Holding);
}

#[test]
fn speed_wait_and_instant_change_the_clock() {
    let mut document = DialogueDocument::new();
    document.insert_text("A");
    document.insert_wait(200);
    document.insert_text("B");
    document.insert_speed(10);
    document.insert_text("C");
    document.insert_instant();
    document.insert_text("DE");

    let mut play = Playback::start(document, "", hold_config(), 0);
    assert_eq!(play.tick(0), 1, "A");
    assert_eq!(play.tick(50), 0, "wait starts");
    assert_eq!(play.tick(249), 0);
    assert_eq!(play.tick(250), 1, "B");
    assert_eq!(play.tick(300), 0, "speed marker arms the next gap");
    assert_eq!(play.tick(310), 1, "C at the new speed");
    assert_eq!(play.tick(320), 2, "instant opens D and E together");
    assert_eq!(play.phase(), Phase::Holding);
    assert_eq!(play.document().plain_text(), "ABCDE");
}

#[test]
fn pause_waits_for_the_continue_key_and_does_not_submit() {
    let mut document = typed("A");
    document.insert_pause();
    document.insert_text("B");
    let mut play = Playback::start(document, "", hold_config(), 0);
    assert_eq!(play.tick(0), 1);
    assert!(
        !play.continue_key(0),
        "continue does nothing before the pause"
    );
    assert_eq!(play.tick(50), 0);
    assert_eq!(play.phase(), Phase::WaitingForKey);
    assert!(play.continue_key(80));
    assert_eq!(play.phase(), Phase::Revealing);
    assert_eq!(play.tick(80), 1);
    assert_eq!(play.document().plain_text(), "AB");
}

#[test]
fn hold_stays_and_fade_uses_base_plus_per_character() {
    let mut held = Playback::start(typed("ab"), "名", hold_config(), 0);
    held.tick(0);
    held.tick(50);
    held.tick(100);
    assert_eq!(held.phase(), Phase::Holding);
    assert!((held.opacity(10_000) - 1.0).abs() < f32::EPSILON);

    let mut fading = Playback::start(
        typed("ab"),
        "",
        PlaybackConfig {
            simulate_pop: false,
            after: AfterLine::Fade,
            fade_base_ms: 100,
            fade_per_char_ms: 10,
            ..PlaybackConfig::default()
        },
        0,
    );
    fading.tick(0);
    fading.tick(50);
    fading.tick(100);
    assert_eq!(fading.phase(), Phase::Fading);
    let mid = fading.opacity(100 + 60);
    assert!(mid < 1.0 && mid > 0.0, "mid fade {mid}");
    fading.tick(100 + 120);
    assert_eq!(fading.phase(), Phase::Finished);
    assert_eq!(fading.opacity(100 + 120), 0.0);
}

#[test]
fn typing_simulation_off_skips_to_hold_or_fade() {
    let mut play = Playback::start(
        typed("abcd"),
        "",
        PlaybackConfig {
            simulate_typing: false,
            simulate_pop: false,
            ..PlaybackConfig::default()
        },
        0,
    );
    assert_eq!(play.cursor(), 4);
    assert_eq!(play.phase(), Phase::Holding);
    assert_eq!(play.tick(0), 0);

    let fade = Playback::start(
        typed("ab"),
        "",
        PlaybackConfig {
            simulate_typing: false,
            simulate_pop: false,
            after: AfterLine::Fade,
            fade_base_ms: 50,
            fade_per_char_ms: 0,
            ..PlaybackConfig::default()
        },
        5,
    );
    assert_eq!(fade.phase(), Phase::Fading);
    assert_eq!(fade.cursor(), 2);
}

#[test]
fn history_caps_at_200_and_keeps_the_name_snapshot() {
    let mut history = SessionHistory::default();
    let mut document = typed("第一句");
    history.push(document.snapshot(), "初音");
    document.clear();
    document.insert_text("后来");
    for index in 0..HISTORY_CAP {
        history.push(typed(&format!("句{index}")), format!("名{index}"));
    }
    assert_eq!(history.len(), HISTORY_CAP);
    assert_eq!(history.entries().front().unwrap().name, "名0");
    assert_ne!(history.entries().front().unwrap().name, "初音");
    let recalled: HistoryEntry = history.entries().back().unwrap().clone();
    assert!(recalled.name.starts_with('名'));
    document.replace_with(recalled.document.clone());
    assert_eq!(document.plain_text(), recalled.document.plain_text());
    assert_eq!(recalled.name, history.entries().back().unwrap().name);
}

#[test]
fn clear_drops_the_undo_stack() {
    let mut document = typed("abc");
    assert!(document.undo());
    document.insert_text("d");
    document.clear();
    assert!(document.is_empty());
    assert!(!document.undo());
}

#[test]
fn pending_color_survives_in_the_snapshot_name_is_separate() {
    let mut document = typed("字");
    document.map_style(|style| style.color = Some([1, 2, 3, 255]));
    let _ = TextStyle::default();
    let snap = document.snapshot();
    assert_eq!(snap.pending_style().color, Some([1, 2, 3, 255]));
    let mut history = SessionHistory::default();
    history.push(document.clone(), "当时的名字");
    assert_eq!(history.get(0).unwrap().name, "当时的名字");
}
