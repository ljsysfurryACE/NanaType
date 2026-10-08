//! Live mirror: the typewriter follows the draft as it is typed.
//!
//! Units are compared by text and attributes, never deduplicated, so typing
//! the same character twice reveals it twice.

use nanatype_model::{Document, Inline, Unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirrorDelta {
    /// Units unchanged at the start; they stay revealed.
    pub common_prefix: u32,
    /// Units of `prev` after the prefix that disappeared.
    pub removed: u32,
    /// Units of `next` after the prefix that should reveal.
    pub appended: u32,
}

fn key<'a>(doc: &'a Document, unit: &Unit) -> (&'a str, Option<&'a Inline>) {
    let inline = doc
        .paragraphs
        .get(unit.para)
        .and_then(|p| p.inlines.get(unit.inline));
    (doc.unit_text(unit), inline)
}

fn same(prev: &Document, a: &Unit, next: &Document, b: &Unit) -> bool {
    if a.para != b.para {
        return false;
    }
    let (ta, ia) = key(prev, a);
    let (tb, ib) = key(next, b);
    if ta != tb {
        return false;
    }
    match (ia, ib) {
        (Some(Inline::Text(x)), Some(Inline::Text(y))) => x.same_attrs(y),
        (Some(Inline::Ruby(x)), Some(Inline::Ruby(y))) => {
            x.annotation == y.annotation && x.base.same_attrs(&y.base)
        }
        (Some(Inline::Atom(x)), Some(Inline::Atom(y))) => x == y,
        _ => false,
    }
}

pub fn mirror_delta(prev: &Document, next: &Document) -> MirrorDelta {
    let a = prev.units();
    let b = next.units();
    let common = a
        .iter()
        .zip(&b)
        .take_while(|(x, y)| same(prev, x, next, y))
        .count();
    MirrorDelta {
        common_prefix: common as u32,
        removed: (a.len() - common) as u32,
        appended: (b.len() - common) as u32,
    }
}

/// Reveal offsets for the appended units: a short stagger so a paste or an
/// IME commit ripples in instead of popping, capped so a long paste does not
/// lag behind typing.
pub fn mirror_offsets(appended: u32, stagger_ms: u32, cap_ms: u32) -> Vec<u32> {
    let step = if appended > 1 {
        stagger_ms.min(cap_ms / appended.max(1))
    } else {
        stagger_ms
    };
    (0..appended).map(|i| i * step).collect()
}
