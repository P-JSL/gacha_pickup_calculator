//! Explicit, source-linked banner profiles. A named game alone is not a rule:
//! special/collaboration/anniversary banners may use different guarantees.
//! These defaults describe ordinary featured banners as documented in Nov 2026.

use crate::{Banner, BannerRule, RateSchedule, SoftPity, Spark};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Game {
    StarRail,
    WutheringWaves,
    Neverness,
    StellaSora,
}

impl Game {
    pub const ALL: [Self; 4] = [Self::StarRail, Self::WutheringWaves, Self::Neverness, Self::StellaSora];
    pub fn label(self) -> &'static str {
        match self {
            Self::StarRail => "붕괴: 스타레일",
            Self::WutheringWaves => "명조: 워더링 웨이브",
            Self::Neverness => "이환 (Neverness to Everness)",
            Self::StellaSora => "스텔라 소라",
        }
    }
    pub fn currency_name(self) -> &'static str {
        match self {
            Self::StarRail => "성옥",
            Self::WutheringWaves => "별의 소리",
            Self::Neverness => "Annulith",
            Self::StellaSora => "Stellanite Dust",
        }
    }
    pub fn secondary_banner_name(self) -> &'static str {
        match self {
            Self::StarRail => "광추",
            Self::WutheringWaves => "무기",
            Self::Neverness => "Arc",
            Self::StellaSora => "레코드",
        }
    }
    pub fn exchange_points_name(self) -> &'static str {
        match self {
            Self::StarRail => "꺼지지 않는 스타라이트",
            Self::WutheringWaves => "여파의 산호",
            Self::Neverness => "Warp Pieces",
            Self::StellaSora => "탐험 전문가 증명서",
        }
    }
    pub fn cost_per_pull(self) -> usize {
        if self == Self::StellaSora { 300 } else { 160 }
    }
    pub fn special_notes(self) -> &'static str {
        match self {
            Self::StarRail => "일반 한정 캐릭터 90회/50:50, 광추 80회/75:25; 픽업 실패 후 다음 5성 확정. 소프트 천장 증가는 비공식 모델.",
            Self::WutheringWaves => "한정 공명자 80회/50:50, 픽업 무기 80회/100%. 명조의 소프트 천장은 공개 실측 추정만 사용.",
            Self::Neverness => "한정 캐릭터는 70회 이후 보드 변화(19.59%), 90회 확정/픽업 100%. Arc는 10개씩 뽑는 배너이며, 6번째 묶음 S급·8번째 묶음 픽업 확정. 자연 발생 복수 S급은 집계하며, 10연 내부 보장 개입 시점은 근사 처리.",
            Self::StellaSora => "일반 픽업 캐릭터 160회 확정, 픽업 약 1%/회. 동일 배너 누적 120회에 픽업 1장 별도 수령 가능(일회·배너 종료 시 만료·천장 미초기화). 일반 디스크 픽업 1.5%/회, 120회 픽업 확정. 일부 특별/기념 배너는 규칙이 다름.",
        }
    }
    pub fn source_links(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::StarRail => &[
                ("HoYoverse 공식 광추 천장", "https://support.hoyoverse.com/hc/en-us/articles/50913708153625-How-does-the-guarantee-system-work-for-the-Light-Cone-Event-Warp-and-does-my-pity-carry-over"),
                ("Prydwen 캐릭터/광추 규칙", "https://www.prydwen.gg/star-rail/guides/gacha-system/"),
            ],
            Self::WutheringWaves => &[
                ("Convene 공지·확률", "https://wutheringwaves.gg/convene-details/"),
                ("실측 소프트 천장 모델", "https://www.wutheringtools.com/convene"),
            ],
            Self::Neverness => &[
                ("Prydwen NTE 가챠", "https://www.prydwen.gg/neverness-to-everness/guides/gacha-system"),
                ("NTE 무기 Arc 확률", "https://gamewith.net/nte/74412"),
                ("이환 기본/종합 확률 구분", "https://alkapa.gg/neverness-to-everness/guide/reroll"),
            ],
            Self::StellaSora => &[
                ("공식 2.0 스페셜 모집", "https://stellasora.global/news/4926"),
                ("일반 천장/특수 스파크 설명", "https://stellasora.pro/pity/"),
            ],
        }
    }
}

#[derive(Clone)]
pub struct GameSetup {
    pub game: Game,
    pub character: Banner,
    pub weapon: Banner,
    pub character_owned: usize,
    pub weapon_owned: usize,
    pub character_dedicated: usize,
    pub weapon_dedicated: usize,
    pub flexible: usize,
    /// Native premium-currency units (e.g. 성옥, 별의 소리, Annulith).
    /// The remainder below one full ticket remains unspent.
    pub premium_currency: usize,
}

impl GameSetup {
    pub fn new(game: Game) -> Self {
        let (character, weapon) = match game {
            Game::StarRail => (
                regular(0.006, 90, SoftPity::Linear { starts_at: 74, increment: 0.06 }, 0.5, 3, true, 7),
                regular(0.008, 80, SoftPity::Linear { starts_at: 66, increment: 0.07 }, 0.75, 0, false, 5),
            ),
            Game::WutheringWaves => (
                regular(0.008, 80, SoftPity::WutheringEstimated, 0.5, 0, false, 7),
                regular(0.008, 80, SoftPity::WutheringEstimated, 1.0, 0, false, 5),
            ),
            Game::Neverness => (
                regular(0.0099, 90, SoftPity::NteBoardChange, 1.0, 0, false, 7),
                Banner { rule: BannerRule::NteArcBatch, pity: 0, guaranteed: false, target_copies: 5, spark: None, any_rare_pity: 0 },
            ),
            Game::StellaSora => (
                Banner { rule: BannerRule::FeaturedCounter { featured_rate: 0.01, hard_featured: 160 },
                    pity: 0, guaranteed: false, target_copies: 7,
                    spark: Some(Spark { at: 120, progress: 0, used: false }), any_rare_pity: 0 },
                Banner { rule: BannerRule::FeaturedCounter { featured_rate: 0.015, hard_featured: 120 },
                    pity: 0, guaranteed: false, target_copies: 5, spark: None, any_rare_pity: 0 },
            ),
        };
        Self { game, character, weapon, character_owned: 0, weapon_owned: 0,
            character_dedicated: 0, weapon_dedicated: 0, flexible: 0, premium_currency: 0 }
    }

    /// Ticket-equivalent flexible budget. Leftover native currency is not a
    /// partial pull, and extra owned native units only finance whole pulls.
    pub fn available_flexible(&self) -> usize {
        self.flexible.saturating_add(self.premium_currency / self.game.cost_per_pull())
    }

    /// Exact native currency required to reach a given total flexible pull
    /// capacity, after pre-owned flexible tickets and native currency.
    pub fn additional_currency_for(&self, required_flexible: usize) -> usize {
        let missing_pulls = required_flexible.saturating_sub(self.flexible);
        missing_pulls.saturating_mul(self.game.cost_per_pull())
            .saturating_sub(self.premium_currency)
    }

    pub fn set_targets(&mut self, character_total: usize, weapon_total: usize) {
        self.character.target_copies = character_total.saturating_sub(self.character_owned);
        self.weapon.target_copies = weapon_total.saturating_sub(self.weapon_owned);
    }
}

fn regular(base: f64, hard: usize, soft: SoftPity, featured_share: f64, pity: usize, guaranteed: bool, target: usize) -> Banner {
    Banner { rule: BannerRule::Standard { rarity: RateSchedule { base, hard, soft }, featured_share },
        pity, guaranteed, target_copies: target, spark: None, any_rare_pity: 0 }
}
#[cfg(test)]
mod currency_tests {
    use super::*;
    #[test]
    fn native_currency_converts_only_in_complete_pulls_for_all_games() {
        for game in Game::ALL {
            let mut setup = GameSetup::new(game);
            let price = game.cost_per_pull();
            setup.flexible = 3;
            setup.premium_currency = 2 * price + price - 1;
            assert_eq!(setup.available_flexible(), 5);
            assert_eq!(setup.additional_currency_for(6), 1);
            assert_eq!(setup.additional_currency_for(5), 0);
        }
    }

    #[test]
    fn default_starting_goals_are_one_each() {
        for game in Game::ALL {
            let mut setup = GameSetup::new(game);
            setup.set_targets(1, 1);
            assert_eq!(setup.character.target_copies, 1);
            assert_eq!(setup.weapon.target_copies, 1);
        }
    }
}
