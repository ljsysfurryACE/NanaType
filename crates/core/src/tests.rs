use std::sync::Arc;

use nanatype_markup::parse;
use nanatype_model::{AfterLine, Document};
use nanatype_playback::{CueKind, Phase, QueuePolicy, WakeReason};
use nanatype_settings::{HistorySync, InputMode, InputPresence, Settings};
use nanatype_theme::SkinRegistry;

use crate::*;

fn skin() -> Arc<nanatype_theme::Skin> {
    SkinRegistry::new(None, "1.0.0")
        .load("nanatype.halo")
        .unwrap()
        .skin
}

fn core_with(f: impl FnOnce(&mut Settings)) -> Core {
    let mut s = Settings::default();
    s.playback.punctuation_pauses = false;
    s.general.speaker_name = "奈奈".into();
    f(&mut s);
    Core::new(s, skin())
}

fn type_and_send(core: &mut Core, markup: &str, now: Millis) -> Effects {
    core.handle(Cmd::DraftChanged(parse(markup).doc), now);
    core.handle(Cmd::Send, now)
}

/// Deliver every wake due by `until`, in order.
fn run_until(core: &mut Core, mut pending: Vec<Wake>, until: Millis) -> Vec<Effects> {
    let mut out = Vec::new();
    loop {
        pending.sort_by_key(|w| w.at_ms);
        let Some(pos) = pending.iter().position(|w| w.at_ms <= until) else {
            return out;
        };
        let w = pending.remove(pos);
        let fx = core.handle(
            Cmd::Wake {
                epoch: w.epoch,
                reason: w.reason,
            },
            w.at_ms,
        );
        pending.extend(fx.wakes.iter().copied());
        out.push(fx);
    }
}

#[test]
fn sending_starts_the_reveal_clears_the_draft_and_adds_a_following_row() {
    let mut core = core_with(|_| {});
    let fx = type_and_send(&mut core, "你好", 1_000);
    assert_eq!(fx.replace_draft, Some(Document::new()));
    assert!(core.draft().is_blank());
    let d = core.display();
    assert!(!d.empty);
    assert_eq!(d.speaker.name, "奈奈");
    assert_eq!(d.reveal.unit_at(0), Some(1_000));
    assert_eq!(d.reveal.unit_at(1), Some(1_100), "CJK waits two beats");
    assert_eq!(core.history().len(), 1);
    assert!(core.history()[0].reveal.is_some(), "row types along");
    assert!(fx.events.iter().any(|e| matches!(e, Event::LineStarted(_))));
    let fx = run_until(&mut core, fx.wakes, 10_000);
    assert!(
        fx.iter()
            .any(|f| f.events.iter().any(|e| matches!(e, Event::LineRevealed(_))))
    );
    assert!(
        core.history()[0]
            .reveal
            .as_ref()
            .is_none_or(|r| r.hidden_from == 2)
    );
    assert_eq!(core.phase(), Some(Phase::Done));
}

#[test]
fn blank_drafts_do_not_send() {
    let mut core = core_with(|_| {});
    let fx = type_and_send(&mut core, "  [wait=100]", 0);
    assert!(fx.replace_draft.is_none());
    assert!(core.display().empty);
    assert!(core.history().is_empty());
}

#[test]
fn continue_passes_a_pause_and_skips_a_segment() {
    let mut core = core_with(|_| {});
    let fx = type_and_send(&mut core, "abcdef[pause]gh", 0);
    // Mid-reveal: continue completes the segment and gates.
    let fx2 = core.handle(Cmd::Continue, 60);
    assert!(fx2.cancel_epochs.len() <= 1);
    assert_eq!(core.phase(), Some(Phase::Gated));
    assert!(core.display().continue_indicator);
    // Old wakes are now stale.
    run_until(&mut core, fx.wakes, 10_000);
    assert_eq!(core.phase(), Some(Phase::Gated));
    let fx3 = core.handle(Cmd::Continue, 5_000);
    assert_eq!(core.phase(), Some(Phase::Revealing));
    assert_eq!(core.display().reveal.unit_at(6), Some(5_000));
    run_until(&mut core, fx3.wakes, 20_000);
    assert_eq!(core.phase(), Some(Phase::Done));
}

#[test]
fn summon_mode_hides_the_input_after_sending() {
    let mut core = core_with(|s| s.general.input_presence = InputPresence::Summon);
    assert!(type_and_send(&mut core, "x", 0).hide_input);
}

#[test]
fn fade_after_line_blanks_the_display() {
    let mut core = core_with(|s| {
        s.playback.after = AfterLine::Fade {
            delay_ms: 500,
            per_unit_ms: 0,
        }
    });
    let fx = type_and_send(&mut core, "a", 0);
    let all = run_until(&mut core, fx.wakes, 100_000);
    assert!(
        all.iter()
            .any(|f| f.events.iter().any(|e| matches!(e, Event::LineFinished(_))))
    );
    assert!(core.display().empty);
    assert_eq!(core.history().len(), 1, "history keeps the line");
}

#[test]
fn queue_policy_holds_lines_until_continue() {
    let mut core = core_with(|s| s.playback.queue = QueuePolicy::Queue);
    let fx = type_and_send(&mut core, "first", 0);
    let fx2 = type_and_send(&mut core, "second", 10);
    assert!(
        fx2.events
            .iter()
            .any(|e| matches!(e, Event::Queued { position: 1 }))
    );
    assert_eq!(
        core.history().len(),
        1,
        "queued lines are not in history yet"
    );
    run_until(&mut core, fx.wakes, 10_000);
    assert_eq!(core.phase(), Some(Phase::Done));
    core.handle(Cmd::Continue, 20_000);
    assert_eq!(core.display().doc.plain_text(), "second");
    assert_eq!(core.history().len(), 2);
}

#[test]
fn replace_policy_interrupts_and_completes_the_old_row() {
    let mut core = core_with(|_| {});
    type_and_send(&mut core, "long line here", 0);
    let fx = type_and_send(&mut core, "next", 10);
    assert!(!fx.cancel_epochs.is_empty() || fx.cues.is_empty());
    assert_eq!(core.display().doc.plain_text(), "next");
    let first = &core.history()[0];
    assert_eq!(
        first.reveal.as_ref().map(|r| r.hidden_from),
        first.reveal.as_ref().map(|_| 14),
        "the interrupted row shows its whole text"
    );
    let reveal = first.reveal.as_ref().unwrap();
    assert!(reveal.unit_at(13).is_some(), "the tail is not hidden");
}

#[test]
fn an_interrupted_multi_segment_line_shows_its_tail_in_history() {
    let mut core = core_with(|_| {});
    type_and_send(&mut core, "ab[pause]cd", 0);
    type_and_send(&mut core, "next", 10);
    let reveal = core.history()[0].reveal.clone();
    if let Some(r) = reveal {
        assert!(r.unit_at(3).is_some());
    }
}

#[test]
fn history_sync_on_submit_and_on_complete() {
    let mut core = core_with(|s| s.history.sync = HistorySync::OnSubmit);
    type_and_send(&mut core, "x", 0);
    assert!(core.history()[0].reveal.is_none());

    let mut core = core_with(|s| s.history.sync = HistorySync::OnComplete);
    let fx = type_and_send(&mut core, "xy", 0);
    assert!(core.history().is_empty());
    run_until(&mut core, fx.wakes, 10_000);
    assert_eq!(core.history().len(), 1);
}

#[test]
fn mirror_mode_follows_typing_including_repeats_and_commits_on_send() {
    let mut core = core_with(|s| s.general.input_mode = InputMode::Mirror);
    core.handle(Cmd::DraftChanged(Document::plain("哈")), 0);
    assert_eq!(core.display().reveal.unit_at(0), Some(0));
    core.handle(Cmd::DraftChanged(Document::plain("哈哈")), 100);
    let r = &core.display().reveal;
    assert_eq!(r.shown, 1, "first character stays");
    assert_eq!(r.unit_at(1), Some(100), "the repeat reveals");
    let fx = core.handle(Cmd::Send, 200);
    assert_eq!(fx.replace_draft, Some(Document::new()));
    assert_eq!(core.history().len(), 1);
    // The editor echoes the cleared draft; the committed line stays.
    core.handle(Cmd::DraftChanged(Document::new()), 210);
    assert_eq!(core.display().doc.plain_text(), "哈哈");
    // New typing replaces it.
    core.handle(Cmd::DraftChanged(Document::plain("嗯")), 300);
    assert_eq!(core.display().doc.plain_text(), "嗯");
}

#[test]
fn recall_walks_sent_lines_and_restores_the_stash() {
    let mut core = core_with(|_| {});
    type_and_send(&mut core, "one", 0);
    type_and_send(&mut core, "two", 10);
    core.handle(Cmd::DraftChanged(Document::plain("draft")), 20);
    let fx = core.handle(Cmd::Recall(-1), 30);
    assert_eq!(fx.replace_draft.unwrap().plain_text(), "two");
    let fx = core.handle(Cmd::Recall(-1), 31);
    assert_eq!(fx.replace_draft.unwrap().plain_text(), "one");
    let fx = core.handle(Cmd::Recall(-1), 32);
    assert_eq!(
        fx.replace_draft.unwrap().plain_text(),
        "one",
        "stays at the oldest"
    );
    core.handle(Cmd::Recall(1), 33);
    let fx = core.handle(Cmd::Recall(1), 34);
    assert_eq!(fx.replace_draft.unwrap().plain_text(), "draft");
}

#[test]
fn expression_cues_switch_portraits_and_shakes_show() {
    let mut core = core_with(|_| {});
    type_and_send(&mut core, "a", 0);
    let fx = core.handle(
        Cmd::CueFired(CueKind::Expression {
            name: "smile".into(),
            intensity: None,
        }),
        10,
    );
    assert!(
        fx.events
            .iter()
            .any(|e| matches!(e, Event::Expression { .. }))
    );
    assert_eq!(
        core.display().portrait,
        None,
        "halo has no portrait variants"
    );
    core.handle(
        Cmd::CueFired(CueKind::Portrait {
            variant: "happy".into(),
        }),
        20,
    );
    assert_eq!(core.display().portrait.as_deref(), Some("happy"));
    core.handle(CmdShake::cmd(), 30);
    assert!(core.display().shake.is_some());
}

struct CmdShake;
impl CmdShake {
    fn cmd() -> Cmd {
        Cmd::CueFired(CueKind::Shake { amp: 4.0, ms: 200 })
    }
}

#[test]
fn skip_reveals_everything_and_clear_blanks() {
    let mut core = core_with(|_| {});
    type_and_send(&mut core, "abc[pause]def", 0);
    core.handle(Cmd::Skip, 5);
    assert_eq!(core.phase(), Some(Phase::Done));
    assert_eq!(core.display().reveal.unit_at(5), Some(5));
    core.handle(Cmd::ClearDisplay, 6);
    assert!(core.display().empty);
    assert_eq!(core.phase(), None);
}

#[test]
fn quick_phrases_send_markup_with_line_meta() {
    let mut core = core_with(|_| {});
    core.handle(Cmd::DraftChanged(Document::plain("keep me")), 0);
    core.handle(Cmd::SendMarkup("[@after=clear delay=100]谢谢！".into()), 1);
    assert_eq!(core.display().doc.plain_text(), "谢谢！");
    assert_eq!(core.draft().plain_text(), "keep me");
}

#[test]
fn switching_modes_clears_the_queue_and_display() {
    let mut core = core_with(|_| {});
    type_and_send(&mut core, "x", 0);
    let mut s = core.settings().clone();
    s.general.input_mode = InputMode::Mirror;
    core.handle(Cmd::SetSettings(Box::new(s)), 5);
    assert_eq!(core.phase(), None);
    assert!(core.display().empty);
}

#[test]
fn stale_wakes_from_an_older_line_are_ignored() {
    let mut core = core_with(|_| {});
    let fx = type_and_send(&mut core, "aaaa", 0);
    type_and_send(&mut core, "bbbbbbbbbbbbbbbbbbb", 10);
    for w in fx.wakes {
        let out = core.handle(
            Cmd::Wake {
                epoch: w.epoch,
                reason: WakeReason::SegmentEnd,
            },
            w.at_ms,
        );
        assert!(out == Effects::default());
    }
    assert_eq!(core.phase(), Some(Phase::Revealing));
}
