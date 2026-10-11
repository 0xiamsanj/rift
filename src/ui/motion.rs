use std::time::Duration;

use gpui::*;

/// Small, restrained motion helpers (Zed-style: feedback, not decoration).
///
/// This GPUI version only animates `opacity` through the styled API — there
/// are no scale/rotate/translate transforms or hover transitions — so every
/// effect below is a fade, a fade-and-rise (via margin), or an opacity pulse.
/// One-shots run ~240 ms with gentle easing so motion reads smooth rather
/// than snappy; repeating pulses render only while their activity is live.
///
/// Deliberately absent: a blinking caret. Repeating animations repaint the
/// whole window every frame, and a caret would do that nonstop while typing.
/// Static carets keep keystrokes cheap.
///
/// All helpers are no-ops when `enabled` is false, backing the
/// "Toggle Interface Animations" palette command for reduced-motion users
/// (the toolkit exposes no OS reduced-motion query).
const ENTRANCE_MS: u64 = 240;
const ENTRANCE_RISE_PX: f32 = 8.0;
const PULSE_MS: u64 = 1400;

/// One-shot fade-and-rise for newly mounted surfaces (dialogs, menus).
/// Replays whenever the element remounts under a new `key`.
pub fn entrance<E>(enabled: bool, el: E, key: ElementId) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if !enabled {
        return el.into_any_element();
    }
    el.with_animation(
        key,
        Animation::new(Duration::from_millis(ENTRANCE_MS)).with_easing(ease_in_out),
        |el, delta| el.opacity(delta).mt(px(-ENTRANCE_RISE_PX * (1.0 - delta))),
    )
    .into_any_element()
}

/// Repeating breathing pulse for ongoing activity (loading, connecting,
/// running transfers). Render only while the activity is live; the pulse
/// itself never stops on its own.
pub fn pulse<E>(enabled: bool, el: E, key: ElementId) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if !enabled {
        return el.into_any_element();
    }
    el.with_animation(
        key,
        Animation::new(Duration::from_millis(PULSE_MS))
            .repeat()
            .with_easing(pulsating_between(0.45, 1.0)),
        |el, value| el.opacity(value),
    )
    .into_any_element()
}

/// One-shot fade for changed content (status messages). Key it by the
/// content so every change replays the fade.
pub fn flash<E>(enabled: bool, el: E, key: ElementId) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    if !enabled {
        return el.into_any_element();
    }
    el.with_animation(
        key,
        Animation::new(Duration::from_millis(150)).with_easing(ease_out_quint()),
        |el, delta| el.opacity(delta),
    )
    .into_any_element()
}

/// Staggered fade for list rows (palette, menus, bookmarks, transfers).
/// `index` sets the cascade position; `max_stagger` caps the total delay
/// so long lists don't feel laggy. Keys must be stable per logical row so
/// re-renders don't replay.
pub fn staggered<E>(enabled: bool, el: E, key: ElementId, index: usize, max_stagger: usize) -> AnyElement
where
    E: IntoElement + Styled + 'static,
{
    const STEP_MS: u64 = 28;
    const FADE_MS: u64 = 160;
    if !enabled {
        return el.into_any_element();
    }
    let steps = index.min(max_stagger) as u64;
    let delay_ms = steps * STEP_MS;
    let total_ms = delay_ms + FADE_MS;
    let hold = delay_ms as f32 / total_ms as f32;
    el.with_animation(
        key,
        Animation::new(Duration::from_millis(total_ms)).with_easing(move |delta| {
            if delta <= hold {
                0.0
            } else {
                ease_out_quint()((delta - hold) / (1.0 - hold))
            }
        }),
        |el, delta| el.opacity(delta),
    )
    .into_any_element()
}
