// SPDX-License-Identifier: GPL-3.0-or-later
//! Where text may be drawn, given a screen profile with exclusion zones (ADR 0008).
//!
//! The algorithm is deliberately simple: with only a few zones, every rectangle spanned by zone
//! and safe-area edges is a candidate, and the largest one that avoids all zones wins.

use sursum_protocol::{MaskMode, Rect, SCENE_HEIGHT, SCENE_WIDTH, ScreenLayout, ScreenProfile};

/// Why a profile cannot be laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LayoutError {
    #[error("the safe-area margins leave no space on the stage")]
    SafeAreaEmpty,
    #[error("the exclusion zones cover the whole safe area")]
    NoFreeSpace,
}

/// The safe area: the stage minus the overscan margins.
pub fn safe_area(profile: &ScreenProfile) -> Result<Rect, LayoutError> {
    let insets = profile.safe_area;
    let width = SCENE_WIDTH.checked_sub(insets.left + insets.right).filter(|w| *w > 0);
    let height = SCENE_HEIGHT.checked_sub(insets.top + insets.bottom).filter(|h| *h > 0);
    match (width, height) {
        (Some(width), Some(height)) => Ok(Rect::new(insets.left, insets.top, width, height)),
        _ => Err(LayoutError::SafeAreaEmpty),
    }
}

/// The largest rectangle inside the safe area that touches no exclusion zone.
/// Ties are broken by closeness to the centre of the stage.
pub fn content_region(profile: &ScreenProfile) -> Result<Rect, LayoutError> {
    let safe = safe_area(profile)?;
    let zones: Vec<Rect> =
        profile.exclusions.iter().map(|zone| zone.rect).filter(|rect| rect.intersects(&safe)).collect();
    let xs = edges(safe.x, safe.right(), zones.iter().flat_map(|zone| [zone.x, zone.right()]));
    let ys = edges(safe.y, safe.bottom(), zones.iter().flat_map(|zone| [zone.y, zone.bottom()]));

    let mut best: Option<Rect> = None;
    for (i, &left) in xs.iter().enumerate() {
        for &right in &xs[i + 1..] {
            for (j, &top) in ys.iter().enumerate() {
                for &bottom in &ys[j + 1..] {
                    let candidate = Rect::new(left, top, right - left, bottom - top);
                    if zones.iter().any(|zone| zone.intersects(&candidate)) {
                        continue;
                    }
                    if best.is_none_or(|current| is_better(&candidate, &current)) {
                        best = Some(candidate);
                    }
                }
            }
        }
    }
    best.ok_or(LayoutError::NoFreeSpace)
}

/// The full layout sent to the screen: content rectangle plus black masks.
pub fn screen_layout(profile: &ScreenProfile) -> Result<ScreenLayout, LayoutError> {
    let content = content_region(profile)?;
    let masks = profile
        .exclusions
        .iter()
        .filter(|zone| zone.mask == MaskMode::Black)
        .filter_map(|zone| clip(&zone.rect, &Rect::SCENE))
        .collect();
    Ok(ScreenLayout { content, masks })
}

/// Sorted, unique edge coordinates clamped to `[low, high]`.
fn edges(low: u32, high: u32, inner: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut values: Vec<u32> =
        std::iter::once(low).chain(std::iter::once(high)).chain(inner.map(|v| v.clamp(low, high))).collect();
    values.sort_unstable();
    values.dedup();
    values
}

/// Larger area wins; on a tie, the rectangle whose centre is closer to the stage centre wins.
fn is_better(candidate: &Rect, current: &Rect) -> bool {
    match candidate.area().cmp(&current.area()) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => distance_to_centre(candidate) < distance_to_centre(current),
    }
}

/// Squared distance (in doubled coordinates, to stay in integers) from the rectangle's centre to
/// the stage centre.
fn distance_to_centre(rect: &Rect) -> u64 {
    let dx = i64::from(rect.x * 2 + rect.width) - i64::from(SCENE_WIDTH);
    let dy = i64::from(rect.y * 2 + rect.height) - i64::from(SCENE_HEIGHT);
    (dx * dx + dy * dy) as u64
}

/// The part of `rect` inside `bounds`, if any.
fn clip(rect: &Rect, bounds: &Rect) -> Option<Rect> {
    let left = rect.x.max(bounds.x);
    let top = rect.y.max(bounds.y);
    let right = rect.right().min(bounds.right());
    let bottom = rect.bottom().min(bounds.bottom());
    (right > left && bottom > top).then(|| Rect::new(left, top, right - left, bottom - top))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_protocol::{ExclusionZone, Insets};

    fn profile(margin: u32, zones: &[Rect]) -> ScreenProfile {
        ScreenProfile {
            safe_area: Insets::uniform(margin),
            exclusions: zones
                .iter()
                .enumerate()
                .map(|(i, rect)| ExclusionZone {
                    name: format!("strefa {i}"),
                    rect: *rect,
                    mask: MaskMode::Black,
                })
                .collect(),
        }
    }

    #[test]
    fn without_zones_the_content_is_the_safe_area() {
        assert_eq!(content_region(&profile(50, &[])), Ok(Rect::new(50, 50, 1820, 980)));
    }

    #[test]
    fn pillar_on_the_left_shifts_content_right() {
        let pillar = Rect::new(0, 0, 400, 1080);
        assert_eq!(content_region(&profile(0, &[pillar])), Ok(Rect::new(400, 0, 1520, 1080)));
    }

    #[test]
    fn pillar_on_the_right_shifts_content_left() {
        let pillar = Rect::new(1500, 0, 420, 1080);
        assert_eq!(content_region(&profile(0, &[pillar])), Ok(Rect::new(0, 0, 1500, 1080)));
    }

    #[test]
    fn pillar_in_the_middle_picks_the_larger_side() {
        let pillar = Rect::new(800, 0, 200, 1080);
        assert_eq!(content_region(&profile(0, &[pillar])), Ok(Rect::new(1000, 0, 920, 1080)));
    }

    #[test]
    fn short_zone_may_leave_a_wider_band_above_or_below() {
        let lamp = Rect::new(900, 900, 120, 180);
        assert_eq!(content_region(&profile(0, &[lamp])), Ok(Rect::new(0, 0, 1920, 900)));
    }

    #[test]
    fn zones_outside_the_safe_area_are_ignored_for_content_but_still_masked() {
        let outside = Rect::new(0, 0, 1920, 20);
        let layout = screen_layout(&profile(40, &[outside])).unwrap();
        assert_eq!(layout.content, Rect::new(40, 40, 1840, 1000));
        assert_eq!(layout.masks, vec![outside]);
    }

    #[test]
    fn zone_covering_everything_is_an_error() {
        assert_eq!(content_region(&profile(0, &[Rect::SCENE])), Err(LayoutError::NoFreeSpace));
    }

    #[test]
    fn absurd_margins_are_an_error() {
        assert_eq!(content_region(&profile(960, &[])), Err(LayoutError::SafeAreaEmpty));
    }

    #[test]
    fn masks_respect_mask_mode_and_are_clipped_to_the_scene() {
        let mut p = profile(0, &[Rect::new(1800, 0, 400, 1080)]);
        p.exclusions.push(ExclusionZone {
            name: "bez maski".into(),
            rect: Rect::new(0, 0, 100, 100),
            mask: MaskMode::None,
        });
        let layout = screen_layout(&p).unwrap();
        assert_eq!(layout.masks, vec![Rect::new(1800, 0, 120, 1080)]);
    }

    proptest::proptest! {
        #[test]
        fn content_never_touches_a_zone_and_stays_in_the_safe_area(
            margin in 0u32..200,
            zones in proptest::collection::vec((0u32..1900, 0u32..1060, 1u32..900, 1u32..700), 0..4),
        ) {
            let rects: Vec<Rect> = zones.iter().map(|&(x, y, w, h)| Rect::new(x, y, w, h)).collect();
            let p = profile(margin, &rects);
            if let Ok(content) = content_region(&p) {
                let safe = safe_area(&p).unwrap();
                proptest::prop_assert!(safe.contains(&content));
                for zone in &rects {
                    proptest::prop_assert!(!zone.intersects(&content));
                }
            }
        }
    }
}
