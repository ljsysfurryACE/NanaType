use std::sync::Arc;

use nanatype_markup::parse;
use nanatype_model::{AfterLine, Document, Line, LineId, Speaker};

use crate::*;

fn no_punct() -> PacingParams {
    PacingParams {
        punctuation_pauses: false,
        ..PacingParams::default()
    }
}

fn quiet() -> TypingSoundRule {
    TypingSoundRule {
        enabled: false,
        ..TypingSoundRule::default()
    }
}

fn tl(markup: &str, pacing: &PacingParams) -> Arc<Timeline> {
    Arc::new(compile(&parse(markup).doc, pacing, &quiet()))
}

fn wakes(effects: &[PlayerEffect]) -> Vec<(u64, WakeReason)> {
    effects
        .iter()
        .filter_map(|e| match e {
            PlayerEffect::WakeAt { at_ms, reason, .. } => Some((*at_ms, *reason)),
            _ => None,
        })
        .collect()
}

#[test]
fn reveal_waits_a_beat_and_cjk_waits_two() {
    let t = tl("a你b", &no_punct());
    let s = &t.segments[0];
    assert_eq!(&*s.reveal_ms, &[0, 50, 150]);
    assert_eq!(s.duration_ms, 200);
    assert_eq!(s.end, SegmentEnd::LineEnd);
}

#[test]
fn punctuation_adds_its_pause() {
    let t = tl("a，b。c", &PacingParams::default());
    assert_eq!(&*t.segments[0].reveal_ms, &[0, 50, 220, 270, 580]);
}

#[test]
fn wait_speed_and_instant_change_the_clock() {
    let t = tl(
        "a[wait=400]b[speed=2]cd[/speed][instant]efg[/instant]h",
        &no_punct(),
    );
    let r = &t.segments[0].reveal_ms;
    assert_eq!(r[0], 0);
    assert_eq!(r[1], 450, "a's beat plus the wait");
    assert_eq!(r[2], 500);
    assert_eq!(r[3], 600, "half speed doubles the beat");
    assert_eq!(r[4], 700);
    assert_eq!(r[5], 700, "instant span appears together");
    assert_eq!(r[6], 700);
    assert_eq!(r[7], 700);
    let fixed = tl("[speed=10ms]你好[/speed]", &no_punct());
    assert_eq!(&*fixed.segments[0].reveal_ms, &[0, 10]);
}

#[test]
fn global_speed_scale_and_typing_off() {
    let fast = PacingParams {
        speed_scale: 0.5,
        ..no_punct()
    };
    assert_eq!(&*tl("ab", &fast).segments[0].reveal_ms, &[0, 25]);
    let off = PacingParams {
        typing: false,
        ..no_punct()
    };
    let t = tl("ab[wait=900]c", &off);
    assert_eq!(&*t.segments[0].reveal_ms, &[0, 0, 0]);
    assert_eq!(t.segments[0].duration_ms, 0);
}

#[test]
fn pauses_split_segments_and_wait_for_continue() {
    let t = tl("ab[pause]cd", &no_punct());
    assert_eq!(t.segments.len(), 2);
    assert_eq!(t.segments[0].units, 0..2);
    assert_eq!(t.segments[1].units, 2..4);
    let (mut p, fx) = Player::start(LineId(1), t, AfterLine::Hold, None, 1_000);
    assert_eq!(wakes(&fx), vec![(1_100, WakeReason::SegmentEnd)]);
    let fx = p.on_wake(p.epoch(), WakeReason::SegmentEnd, 1_100);
    assert!(fx.contains(&PlayerEffect::ContinueIndicator(true)));
    assert_eq!(p.phase(), Phase::Gated);
    assert_eq!(p.snapshot(5_000).revealed, 2);
    let (did, fx) = p.advance(6_000);
    assert!(did);
    assert!(matches!(
        fx.iter()
            .find(|e| matches!(e, PlayerEffect::ShowSegment { .. })),
        Some(PlayerEffect::ShowSegment {
            seg: 1,
            origin_ms: 6_000,
            ..
        })
    ));
    assert_eq!(p.snapshot(6_000).revealed, 3);
    assert_eq!(p.snapshot(6_050).revealed, 4);
}

#[test]
fn auto_pause_continues_on_its_own() {
    let t = tl("a[pause auto=300]b", &no_punct());
    let (mut p, _) = Player::start(LineId(1), t, AfterLine::Hold, None, 0);
    let fx = p.on_wake(p.epoch(), WakeReason::SegmentEnd, 50);
    assert_eq!(wakes(&fx), vec![(350, WakeReason::AutoContinue)]);
    let fx = p.on_wake(p.epoch(), WakeReason::AutoContinue, 350);
    assert!(
        fx.iter()
            .any(|e| matches!(e, PlayerEffect::ShowSegment { seg: 1, .. }))
    );
}

#[test]
fn page_clears_what_came_before() {
    let t = tl("ab[page]cd", &no_punct());
    assert_eq!(t.segments[0].end, SegmentEnd::Page);
    assert_eq!(t.segments[1].visible_from, 2);
    let (mut p, _) = Player::start(LineId(1), t, AfterLine::Hold, None, 0);
    p.on_wake(p.epoch(), WakeReason::SegmentEnd, 100);
    p.advance(200);
    assert_eq!(p.snapshot(200).visible_from, 2);
}

#[test]
fn hold_stays_and_fade_uses_delay_plus_per_unit() {
    let t = tl("abc", &no_punct());
    let (mut p, _) = Player::start(LineId(7), t.clone(), AfterLine::Hold, None, 0);
    let fx = p.on_wake(p.epoch(), WakeReason::SegmentEnd, 150);
    assert!(fx.contains(&PlayerEffect::LineRevealed(LineId(7))));
    assert!(wakes(&fx).is_empty());
    assert_eq!(p.phase(), Phase::Done);

    let fade = AfterLine::Fade {
        delay_ms: 1_000,
        per_unit_ms: 40,
    };
    let (mut p, _) = Player::start(LineId(8), t, fade, None, 0);
    let fx = p.on_wake(p.epoch(), WakeReason::SegmentEnd, 150);
    assert_eq!(wakes(&fx), vec![(150 + 1_000 + 120, WakeReason::AfterLine)]);
    let fx = p.on_wake(p.epoch(), WakeReason::AfterLine, 1_270);
    assert!(matches!(fx[0], PlayerEffect::FadeOut { at_ms: 1_270, .. }));
    assert!(p.snapshot(1_270 + 225).opacity < 0.6);
    let fx = p.on_wake(p.epoch(), WakeReason::FadeDone, 1_270 + 450);
    assert_eq!(fx, vec![PlayerEffect::LineFinished(LineId(8))]);
    assert_eq!(p.snapshot(9_999).opacity, 0.0);
}

#[test]
fn stale_wakes_are_ignored_after_a_skip() {
    let t = tl("abcdef", &no_punct());
    let (mut p, fx) = Player::start(LineId(1), t, AfterLine::Hold, None, 0);
    let old = match &fx[1] {
        PlayerEffect::ShowSegment { epoch, .. } => *epoch,
        other => panic!("{other:?}"),
    };
    let (_, fx) = p.advance(60);
    assert!(
        fx.iter()
            .any(|e| matches!(e, PlayerEffect::LineRevealed(_)))
    );
    assert_eq!(p.snapshot(60).revealed, 6);
    assert!(p.on_wake(old, WakeReason::SegmentEnd, 300).is_empty());
}

#[test]
fn skip_fires_pending_expressions_but_not_clicks() {
    let doc = parse("ab[expr=smile]cd[se=coin]").doc;
    let t = Arc::new(compile(&doc, &no_punct(), &TypingSoundRule::default()));
    let (mut p, _) = Player::start(LineId(1), t, AfterLine::Hold, None, 0);
    let (_, fx) = p.advance(10);
    let fired = fx
        .iter()
        .find_map(|e| match e {
            PlayerEffect::FireNow { cues } => Some(cues.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(fired.len(), 2);
    assert!(fired.iter().all(|c| c.kind.survives_skip()));
}

#[test]
fn auto_advance_fires_after_the_reveal() {
    let t = tl("a", &no_punct());
    let (mut p, _) = Player::start(LineId(3), t, AfterLine::Hold, Some(800), 0);
    let fx = p.on_wake(p.epoch(), WakeReason::SegmentEnd, 50);
    assert_eq!(wakes(&fx), vec![(850, WakeReason::AutoAdvance)]);
    let fx = p.on_wake(p.epoch(), WakeReason::AutoAdvance, 850);
    assert_eq!(fx, vec![PlayerEffect::AutoAdvance(LineId(3))]);
}

#[test]
fn typing_sounds_skip_spaces_punctuation_and_collapse_instant_spans() {
    let doc = parse("a b。[instant]cdef[/instant]").doc;
    let t = compile(&doc, &no_punct(), &TypingSoundRule::default());
    let clicks: Vec<u32> = t.segments[0]
        .cues
        .iter()
        .filter_map(|c| match c.kind {
            CueKind::TypeSound { unit, .. } => Some(unit),
            _ => None,
        })
        .collect();
    assert_eq!(clicks, vec![0, 2, 4]);
    let every_other = TypingSoundRule {
        every_n: 2,
        ..TypingSoundRule::default()
    };
    let t = compile(&parse("abcd").doc, &no_punct(), &every_other);
    let n = t.segments[0]
        .cues
        .iter()
        .filter(|c| matches!(c.kind, CueKind::TypeSound { .. }))
        .count();
    assert_eq!(n, 2);
}

#[test]
fn speaking_cues_bracket_each_segment() {
    let t = tl("ab[pause]c", &no_punct());
    for s in &t.segments {
        assert_eq!(s.cues.first().unwrap().kind, CueKind::SpeakingStart);
        assert_eq!(s.cues.last().unwrap().kind, CueKind::SpeakingStop);
    }
}

#[test]
fn markers_cue_at_the_moment_they_are_reached() {
    let t = tl("ab[expr=smile]c[screenshake=4,200]", &no_punct());
    let at = |pred: fn(&CueKind) -> bool| {
        t.segments[0]
            .cues
            .iter()
            .find(|c| pred(&c.kind))
            .unwrap()
            .at_ms
    };
    assert_eq!(at(|k| matches!(k, CueKind::Expression { .. })), 100);
    assert_eq!(at(|k| matches!(k, CueKind::Shake { .. })), 150);
}

#[test]
fn mirror_appends_repeated_characters() {
    let a = Document::plain("哈");
    let b = Document::plain("哈哈");
    assert_eq!(
        mirror_delta(&a, &b),
        MirrorDelta {
            common_prefix: 1,
            removed: 0,
            appended: 1
        }
    );
    let c = Document::plain("哈呵哈");
    assert_eq!(
        mirror_delta(&b, &c),
        MirrorDelta {
            common_prefix: 1,
            removed: 1,
            appended: 2
        }
    );
    let styled = parse("[b]哈[/b]哈").doc;
    assert_eq!(mirror_delta(&b, &styled).common_prefix, 0);
    assert_eq!(mirror_offsets(3, 30, 60), vec![0, 20, 40]);
}

#[test]
fn queue_policies() {
    let line = || Line::new(Document::plain("x"), Speaker::default());
    let mut q = LineQueue::new(QueuePolicy::Queue);
    assert!(matches!(
        q.submit(line(), false),
        QueueDecision::StartNow(_)
    ));
    assert!(matches!(
        q.submit(line(), true),
        QueueDecision::Enqueued { position: 1 }
    ));
    assert!(q.pop_next().is_some());
    let mut q = LineQueue::new(QueuePolicy::FinishThenNext);
    assert!(matches!(
        q.submit(line(), true),
        QueueDecision::FinishCurrentThenStart(_)
    ));
    let mut q = LineQueue::new(QueuePolicy::Replace);
    assert!(matches!(q.submit(line(), true), QueueDecision::StartNow(_)));
}
