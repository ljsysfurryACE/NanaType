use std::fs;

use nanatype_model::AfterLine;

use crate::*;

const V0: &str = r#"{
  "theme": "Night",
  "speaker_name": "奈奈",
  "after_line": "Fade",
  "fade_base_ms": 1500,
  "fade_per_char_ms": 30,
  "speed_ms": 25,
  "simulate_typing": true,
  "simulate_pop": false,
  "per_character": true,
  "typewriter_sound": true,
  "input_presence": "Hotkey",
  "summon_hotkey": { "ctrl": false, "shift": true, "alt": false, "win": false, "key": "F8" },
  "continue_hotkey": { "ctrl": false, "shift": false, "alt": false, "win": false, "key": "F10" },
  "layout_adjust": false,
  "show_input": true,
  "show_typewriter": true,
  "show_history": false,
  "spout_typewriter": true,
  "spout_history": false,
  "spout_name_typewriter": "My/Typewriter",
  "spout_name_history": "NanaType/History",
  "input_width": 760, "input_height": 304,
  "typewriter_width": 760, "typewriter_height": 200,
  "history_width": 400, "history_height": 600,
  "sticker_dir": "D:/stickers",
  "geometry": { "input": { "65537": { "x": 1, "y": 2, "width": 3, "height": 4 } } }
}"#;

#[test]
fn chat_panel_style_defaults_and_round_trips_without_a_schema_change() {
    let old: Settings =
        serde_json::from_str(r#"{"schema":1,"integrations":{"blivechat":{"panel_open":true}}}"#)
            .unwrap();
    assert_eq!(
        old.integrations.blivechat.panel_style,
        ChatPanelStyle::Compact
    );
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    for (style, wire) in [
        (ChatPanelStyle::Compact, "compact"),
        (ChatPanelStyle::History, "history"),
    ] {
        let mut settings = old.clone();
        settings.integrations.blivechat.panel_style = style;
        save_json(&path, &settings).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(json["integrations"]["blivechat"]["panel_style"], wire);
        let loaded = load_settings(&path, None).value;
        assert_eq!(loaded.integrations.blivechat.panel_style, style);
        assert!(loaded.integrations.blivechat.panel_open);
        assert_eq!(loaded.schema, SETTINGS_SCHEMA);
    }
}

#[test]
fn v0_settings_migrate_into_the_new_schema() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = tmp.path().join("legacy.json");
    fs::write(&legacy, V0).unwrap();
    let path = tmp.path().join("new/settings.json");
    let loaded = load_settings(&path, Some(&legacy));
    assert!(matches!(loaded.note, LoadNote::Migrated { from: 0 }));
    let s = loaded.value;
    assert_eq!(s.schema, SETTINGS_SCHEMA);
    assert_eq!(s.skin.id, "nanatype.night");
    assert_eq!(s.general.speaker_name, "奈奈");
    assert_eq!(s.general.input_mode, InputMode::Mirror);
    assert_eq!(s.general.input_presence, InputPresence::Summon);
    assert_eq!(
        s.playback.after,
        AfterLine::Fade {
            delay_ms: 1500,
            per_unit_ms: 30
        }
    );
    assert_eq!(s.playback.speed_scale, 0.5);
    assert_eq!(
        s.playback.entrance,
        Some(nanatype_model::EntranceKind::None)
    );
    assert!(s.sound.typing);
    assert!(s.hotkeys.summon.as_ref().unwrap().shift);
    assert_eq!(
        s.hotkeys.continue_,
        HotkeySettings::default().continue_,
        "F10 moves to F9"
    );
    assert!(!s.overlays.history.visible);
    assert!(s.output.display.enabled);
    assert_eq!(s.output.display.name, "My/Typewriter");
    assert_eq!(s.overlays.history.size, Some([400.0, 600.0]));
    assert_eq!(s.assets.sticker_dirs, vec!["D:/stickers".to_string()]);
    assert!(s.unknown.is_empty(), "{:?}", s.unknown);
}

#[test]
fn unknown_fields_survive_a_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    fs::write(
        &path,
        r#"{ "schema": 1, "future_feature": { "x": 1 }, "general": { "speaker_name": "a", "later": true } }"#,
    )
    .unwrap();
    let loaded = load_settings(&path, None);
    assert!(matches!(loaded.note, LoadNote::Loaded));
    assert_eq!(loaded.value.general.speaker_name, "a");
    assert!(loaded.value.unknown.contains_key("future_feature"));
    save_json(&path, &loaded.value).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("future_feature"));
}

#[test]
fn missing_fields_take_defaults_instead_of_resetting_everything() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    fs::write(
        &path,
        r#"{ "schema": 1, "skin": { "id": "nanatype.rpg" } }"#,
    )
    .unwrap();
    let s = load_settings(&path, None).value;
    assert_eq!(s.skin.id, "nanatype.rpg");
    assert_eq!(s.history.max_items, 200);
}

#[test]
fn files_from_before_blivechat_get_its_defaults() {
    let json = r#"{"schema": 1, "integrations": {"osc": {"enabled": true}}}"#;
    let s: Settings = serde_json::from_str(json).unwrap();
    let b = &s.integrations.blivechat;
    assert!(!b.enabled);
    assert_eq!((b.host.as_str(), b.port), ("127.0.0.1", 12450));
    assert_eq!(b.room_kind, BliveRoomKind::AuthCode);
    assert!(b.quote_on_display && b.panel.text && b.panel.gift);
    assert!(
        !s.history.chat.enabled,
        "chat stays out of history until asked"
    );
    assert_eq!(
        s.history.chat.filter.min_gift_coin, 1,
        "no free gifts in history"
    );
    assert!(s.history.show_quote);
}

#[test]
fn blivechat_values_are_clamped() {
    let mut s = Settings::default();
    let b = &mut s.integrations.blivechat;
    b.host = "  ".into();
    b.port = 0;
    b.room = " 123 ".into();
    assert!(s.sanitize());
    let b = &s.integrations.blivechat;
    assert_eq!((b.host.as_str(), b.port), ("127.0.0.1", 12450));
    assert_eq!(b.room, "123");
}

#[test]
fn corrupt_files_are_quarantined_not_overwritten() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    fs::write(&path, "{ not json").unwrap();
    let loaded = load_settings(&path, None);
    match loaded.note {
        LoadNote::Quarantined { moved_to, .. } => {
            assert!(moved_to.exists());
            assert_eq!(fs::read_to_string(moved_to).unwrap(), "{ not json");
        }
        other => panic!("{other:?}"),
    }
    assert!(!path.exists());
}

#[test]
fn newer_schemas_load_read_only() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    fs::write(
        &path,
        r#"{ "schema": 7, "general": { "speaker_name": "z" } }"#,
    )
    .unwrap();
    let loaded = load_settings(&path, None);
    assert!(matches!(loaded.note, LoadNote::TooNew { schema: 7 }));
    assert_eq!(loaded.value.general.speaker_name, "z");
}

#[test]
fn atomic_writes_keep_a_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    save_json(&path, &Settings::default()).unwrap();
    let mut s = Settings::default();
    s.general.speaker_name = "b".into();
    save_json(&path, &s).unwrap();
    assert!(path.with_extension("json.bak").exists());
    assert_eq!(load_settings(&path, None).value.general.speaker_name, "b");
}

#[test]
fn sanitize_clamps_hand_edited_values() {
    let mut s = Settings::default();
    s.playback.speed_scale = f32::NAN;
    s.sound.master = 7.0;
    s.history.visible_items = 99_999;
    assert!(s.sanitize());
    assert_eq!(s.playback.speed_scale, 1.0);
    assert_eq!(s.sound.master, 1.0);
    assert_eq!(s.history.visible_items, s.history.max_items);
    assert!(!s.sanitize());
}

#[test]
fn hotkey_labels_and_global_safety() {
    let h = Hotkey {
        ctrl: true,
        shift: true,
        ..Hotkey::bare("KeyN")
    };
    assert_eq!(h.label("Win"), "Ctrl+Shift+N");
    assert!(h.is_safe_global());
    assert!(Hotkey::bare("F9").is_safe_global());
    assert!(!Hotkey::bare("Space").is_safe_global());
    assert!(!Hotkey::bare("KeyA").is_safe_global());
}

fn monitor(key: &str, x: f64, primary: bool) -> MonitorInfo {
    MonitorInfo {
        key: MonitorKey(key.into()),
        work_area: [x, 0.0, 1920.0, 1040.0],
        primary,
    }
}

#[test]
fn geometry_restores_per_layout_and_falls_back_to_a_connected_monitor() {
    let desk = [monitor("A", 0.0, true), monitor("B", 1920.0, false)];
    let mut store = GeometryStore::default();
    let rect = Placement {
        x: 1920.0 + 960.0,
        y: 520.0,
        w: 760.0,
        h: 200.0,
    };
    store.remember(&desk, "display", rect);
    assert_eq!(store.restore(&desk, "display"), Some(rect));

    // Laptop alone: B is gone, so there is nothing to restore.
    let laptop = [monitor("A", 0.0, true)];
    assert_eq!(store.restore(&laptop, "display"), None);

    // B reconnected on the left: same relative spot on B.
    let moved = [monitor("B", -1920.0, false), monitor("A", 0.0, true)];
    let p = store.restore(&moved, "display").unwrap();
    assert_eq!(p.x, -1920.0 + 960.0);

    // A frame partly off its monitor is pulled back inside.
    let mut store = GeometryStore::default();
    store.remember(
        &laptop,
        "history",
        Placement {
            x: 1800.0,
            y: 900.0,
            w: 400.0,
            h: 600.0,
        },
    );
    let p = store.restore(&laptop, "history").unwrap();
    assert!(p.x + p.w <= 1920.0 && p.y + p.h <= 1040.0);
}

#[test]
fn only_a_new_installation_enables_chat_history() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("settings.json");
    assert!(load_settings(&path, None).value.history.chat.enabled);
    for json in [
        r#"{"schema":1}"#,
        r#"{"schema":1,"history":{"chat":{"enabled":false}}}"#,
        "broken json",
    ] {
        fs::write(&path, json).unwrap();
        assert!(!load_settings(&path, None).value.history.chat.enabled);
    }
    fs::write(&path, r#"{"schema":1,"history":{"chat":{"enabled":true}}}"#).unwrap();
    assert!(load_settings(&path, None).value.history.chat.enabled);
}

#[test]
fn old_settings_preserve_local_output_and_add_disabled_partner_output() {
    let s: Settings = serde_json::from_str(
        r#"{
        "schema":1,
        "output":{"display":{"enabled":true,"name":"My Display","size":[1280,720]}},
        "overlays":{"display":{"size":[900,300]}}
    }"#,
    )
    .unwrap();
    assert!(s.output.display.enabled);
    assert_eq!(s.output.display.name, "My Display");
    assert_eq!(s.output.display.size, Some([1280, 720]));
    assert_eq!(s.overlays.display.size, Some([900.0, 300.0]));
    assert!(!s.output.peer_display.enabled);
    assert_eq!(s.output.peer_display.name, "NanaType Partner");
    assert_eq!(s.overlays.peer_display.size, None);
}

#[test]
fn partner_settings_round_trip_and_sanitize() {
    let mut s = Settings::default();
    s.output.peer_display.enabled = true;
    s.output.peer_display.name = "  Partner  ".into();
    s.output.peer_display.size = Some([0, 99999]);
    s.overlays.peer_display.size = Some([0.0, 99999.0]);
    s.overlays.peer_display.lock = Lock::EditLayout;
    assert!(s.sanitize());
    assert_eq!(s.output.peer_display.name, "Partner");
    assert_eq!(s.output.peer_display.size, Some([16, 8192]));
    assert_eq!(s.overlays.peer_display.size, Some([160.0, 8192.0]));
    let loaded: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert_eq!(loaded, s);
    assert!(!s.output.display.enabled);
    assert_eq!(s.overlays.display.size, None);
}
