//! External observation references for **validation**, not hidden rate overrides.
//!
//! Aggregate means cannot identify a per-pity hazard function without the full,
//! censoring-corrected pull distribution. Never silently calibrate gacha odds
//! from an observed mean, banner-mixed count, or win-rate sample.
use crate::games::Game;
use crate::{Banner, BannerRule, RateMode, RateSchedule};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackedBanner {
    Character,
    Weapon,
    General,
}

#[derive(Debug, Clone, Copy)]
pub struct TrackerReference {
    pub name: &'static str,
    pub url: &'static str,
    pub observed_on: &'static str,
    pub banner: TrackedBanner,
    pub mean_rare_pulls: Option<f64>,
    pub total_pulls: Option<u64>,
    pub notes: &'static str,
}

const STAR_RAIL: [TrackerReference; 3] = [
    TrackerReference {
        name: "Gacha Tracker — 캐릭터 이벤트",
        url: "https://gacha-tracker.com/hsr/stats",
        observed_on: "2026-11-05",
        banner: TrackedBanner::Character,
        mean_rare_pulls: Some(58.9),
        total_pulls: Some(353_307),
        notes: "5성 평균 58.9뽑, 관측 50:50 승률 58%. 추적자 표본이며 공식 확률이 아닙니다.",
    },
    TrackerReference {
        name: "Gacha Tracker — 광추 이벤트",
        url: "https://gacha-tracker.com/hsr/stats",
        observed_on: "2026-11-05",
        banner: TrackedBanner::Weapon,
        mean_rare_pulls: Some(48.6),
        total_pulls: Some(87_513),
        notes: "5성 평균 48.6뽑, 관측 픽업 승률 78%. 확정 획득과 비확정을 구분해야 합니다.",
    },
    TrackerReference {
        name: "StarDB — 개인별 실제 워프 이력",
        url: "https://stardb.gg/en/warp-tracker/",
        observed_on: "2026-11-05",
        banner: TrackedBanner::General,
        mean_rare_pulls: None,
        total_pulls: None,
        notes: "개인별 4성·5성 천장과 픽업 당첨 이력 검증용. 전역 평균으로 사용하지 않습니다.",
    },
];

const WUTHERING: [TrackerReference; 2] = [
    TrackerReference {
        name: "WutheringTools — 소환 실측 곡선",
        url: "https://www.wutheringtools.com/convene",
        observed_on: "2026-11-05",
        banner: TrackedBanner::Character,
        mean_rare_pulls: Some(54.1),
        total_pulls: None,
        notes: "실측 기반 5성 평균 약 54.1뽑. 0.8% 기본, 66회부터 관측 소프트 천장 추정.",
    },
    TrackerReference {
        name: "WuWa Tracker — 글로벌 배너별 통계",
        url: "https://wuwatracker.com/ko/tracker/stats/100024",
        observed_on: "2026-11-05",
        banner: TrackedBanner::General,
        mean_rare_pulls: None,
        total_pulls: Some(18_472_535),
        notes: "표시된 뽑기 수는 특정 배너 페이지의 공개 집계이며 전체 게임 공통 평균으로 사용하지 않습니다.",
    },
];

const NEVERNESS: [TrackerReference; 1] = [TrackerReference {
    name: "nteTracker — S급/Arc 개인 이력",
    url: "https://ntetracker.com/",
    observed_on: "2026-11-05",
    banner: TrackedBanner::General,
    mean_rare_pulls: None,
    total_pulls: None,
    notes: "전역 표본 평균 미확인. 0.99% 기본과 천장 규칙을 우선하며 Arc 내부 보장 순서는 근사.",
}];

const STELLA: [TrackerReference; 2] = [
    TrackerReference {
        name: "StellaDB — 게임 데이터·소환 기록",
        url: "https://stelladb.pages.dev/",
        observed_on: "2026-11-05",
        banner: TrackedBanner::General,
        mean_rare_pulls: None,
        total_pulls: None,
        notes: "전역 평균 스택 미검증. 개인 추적 데이터와 확률표를 혼동하지 않습니다.",
    },
    TrackerReference {
        name: "StellaSora.pro — 천장·보너스 설명",
        url: "https://stellasora.pro/tools/pity-calculator/",
        observed_on: "2026-11-05",
        banner: TrackedBanner::General,
        mean_rare_pulls: None,
        total_pulls: None,
        notes: "픽업 160회 확정 및 배너별 120회 별도 교환은 다른 카운터입니다.",
    },
];

pub fn references(game: Game) -> &'static [TrackerReference] {
    match game {
        Game::StarRail => &STAR_RAIL,
        Game::WutheringWaves => &WUTHERING,
        Game::Neverness => &NEVERNESS,
        Game::StellaSora => &STELLA,
    }
}

/// E[T] = sum_{n>=0} P(T>n), conditional on the specified pity.
/// This is a 5★ rarity wait, not an E0 pickup wait.
pub fn expected_rare_wait(schedule: &RateSchedule, mode: RateMode, current_pity: usize) -> Option<f64> {
    if schedule.hard == 0 || current_pity >= schedule.hard {
        return None;
    }
    let mut survival = 1.0;
    let mut expected = 0.0;
    for next in (current_pity + 1)..=schedule.hard {
        expected += survival;
        survival *= 1.0 - schedule.chance(next, mode);
    }
    Some(expected)
}

/// Only mathematically comparable if the tracker reports the same 5★ rarity,
/// same banner, start pity=0, and observation has enough complete waiting times.
pub fn expected_for_banner(banner: &Banner, mode: RateMode) -> Option<f64> {
    match &banner.rule {
        BannerRule::Standard { rarity, .. } => expected_rare_wait(rarity, mode, 0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SoftPity;

    #[test]
    fn guaranteed_one_pull_has_expected_wait_one() {
        let r = RateSchedule { base: 0.0, hard: 1, soft: SoftPity::None };
        assert_eq!(expected_rare_wait(&r, RateMode::PublishedOnly, 0), Some(1.0));
    }

    #[test]
    fn hard_pity_reference_is_exact() {
        let r = RateSchedule { base: 0.0, hard: 10, soft: SoftPity::None };
        assert_eq!(expected_rare_wait(&r, RateMode::PublishedOnly, 0), Some(10.0));
        assert_eq!(expected_rare_wait(&r, RateMode::PublishedOnly, 8), Some(2.0));
        assert_eq!(expected_rare_wait(&r, RateMode::PublishedOnly, 10), None);
    }

    #[test]
    fn tracker_mean_is_not_substituted_into_base_probabilities() {
        let setup = crate::games::GameSetup::new(Game::StarRail);
        let before = expected_for_banner(&setup.character, RateMode::CommunityEstimate).unwrap();
        assert!((before - 62.297_332_039_630_95).abs() < 1e-8);
        assert!(references(Game::StarRail)[0].mean_rare_pulls.is_some());
        let after = expected_for_banner(&setup.character, RateMode::CommunityEstimate).unwrap();
        assert_eq!(before, after);
    }
}
