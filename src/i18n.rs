//! Offline desktop localization. No IP geolocation, web requests or external dependencies.
use gacha_pickup_calculator::games::Game;
use std::{env, fs, path::PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language { Korean, English, Japanese, ChineseSimplified, ChineseTraditional }
impl Language {
    pub fn code(self) -> &'static str {
        match self {
            Self::Korean => "KO", Self::English => "EN", Self::Japanese => "JA",
            Self::ChineseSimplified => "ZH-CN", Self::ChineseTraditional => "ZH-TW",
        }
    }
    const fn index(self) -> usize {
        match self { Self::Korean => 0, Self::English => 1, Self::Japanese => 2,
            Self::ChineseSimplified => 3, Self::ChineseTraditional => 4 }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LanguagePreference { Automatic, Korean, English, Japanese, ChineseSimplified, ChineseTraditional }
impl LanguagePreference {
    pub const ALL: [Self; 6] = [Self::Automatic, Self::Korean, Self::English,
        Self::Japanese, Self::ChineseSimplified, Self::ChineseTraditional];
    fn key(self) -> &'static str {
        match self { Self::Automatic => "auto", Self::Korean => "ko", Self::English => "en",
            Self::Japanese => "ja", Self::ChineseSimplified => "zh-cn",
            Self::ChineseTraditional => "zh-tw" }
    }
    fn from_key(s: &str) -> Option<Self> {
        match s.trim() { "auto" => Some(Self::Automatic), "ko" => Some(Self::Korean),
            "en" => Some(Self::English), "ja" => Some(Self::Japanese),
            "zh-cn" => Some(Self::ChineseSimplified), "zh-tw" => Some(Self::ChineseTraditional),
            _ => None }
    }
    pub fn effective(self, system: Language) -> Language {
        match self { Self::Automatic => system, Self::Korean => Language::Korean,
            Self::English => Language::English, Self::Japanese => Language::Japanese,
            Self::ChineseSimplified => Language::ChineseSimplified,
            Self::ChineseTraditional => Language::ChineseTraditional }
    }
    pub fn display(self, system: Language, ui: Language) -> String {
        match self {
            Self::Automatic => format!("{} ({})", t(ui, "Automatic"), system.code()),
            Self::Korean => "한국어".into(), Self::English => "English".into(),
            Self::Japanese => "日本語".into(), Self::ChineseSimplified => "简体中文".into(),
            Self::ChineseTraditional => "繁體中文".into(),
        }
    }
}

/// Locale is taken from the OS user locale/region, never inferred from IP addresses.
pub fn parse_system_locale(raw: &str) -> Language {
    let locale = raw.trim().split([':', '.', '@']).next().unwrap_or("")
        .replace('_', "-").to_ascii_lowercase();
    if locale == "ko" || locale.starts_with("ko-") { Language::Korean }
    else if locale == "ja" || locale.starts_with("ja-") { Language::Japanese }
    else if locale == "zh-tw" || locale.starts_with("zh-hk") || locale.starts_with("zh-mo")
        || locale.starts_with("zh-hant") { Language::ChineseTraditional }
    else if locale == "zh" || locale.starts_with("zh-") { Language::ChineseSimplified }
    else { Language::English }
}
#[cfg(windows)]
fn windows_user_locale() -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultLocaleName(locale_name: *mut u16, length: i32) -> i32;
    }
    let mut buffer = [0u16; 85]; // Windows LOCALE_NAME_MAX_LENGTH
    let length = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 1 || length as usize > buffer.len() { return None; }
    String::from_utf16(&buffer[..length as usize - 1]).ok()
}
pub fn system_language() -> Language {
    #[cfg(windows)]
    if let Some(locale) = windows_user_locale() { return parse_system_locale(&locale); }
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(v) = env::var(key) {
            if !v.is_empty() && v != "C" && v != "POSIX" { return parse_system_locale(&v); }
        }
    }
    Language::English
}
fn settings_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("APPDATA").map(|p| PathBuf::from(p)
            .join("GachaPickupCalculator").join("language.txt"))
    }
    #[cfg(not(windows))]
    {
        env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
            .map(|p| p.join("gacha-pickup-calculator").join("language.txt"))
    }
}
pub fn load_preference() -> LanguagePreference {
    settings_path().and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| LanguagePreference::from_key(&s))
        .unwrap_or(LanguagePreference::Automatic)
}
pub fn save_preference(pref: LanguagePreference) -> std::io::Result<()> {
    let path = settings_path().ok_or_else(|| std::io::Error::new(
        std::io::ErrorKind::NotFound, "No system settings directory"))?;
    if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
    fs::write(path, pref.key())
}

/// User-visible strings: Korean, English, Japanese, Simplified Chinese, Traditional Chinese.
/// Unknown locale defaults to English; game names and item identities are not modified.
pub fn t(language: Language, text: &'static str) -> &'static str {
    let values = match text {
        "Automatic" => ["자동 (시스템 언어)", "Automatic", "自動（システム言語）", "自动（系统语言）", "自動（系統語言）"],
        "Language" => ["언어", "Language", "言語", "语言", "語言"],
        "System language" => ["시스템 언어", "System language", "システム言語", "系统语言", "系統語言"],
        "Saved language preference" => ["선택한 언어는 재실행 후에도 유지됩니다.", "Your language choice is saved for the next launch.", "選択した言語は次回起動時も維持されます。", "语言选择将在下次启动时保留。", "語言選擇將在下次啟動時保留。"],
        "Dashboard" => ["대시보드", "Dashboard", "ダッシュボード", "仪表盘", "儀表板"],
        "Simulator" => ["시뮬레이터", "Simulator", "シミュレーター", "模拟器", "模擬器"],
        "Currency" => ["재화", "Currency", "通貨", "货币", "貨幣"],
        "Characters" => ["캐릭터", "Characters", "キャラクター", "角色", "角色"],
        "Weapons" => ["무기", "Weapons", "武器", "武器", "武器"],
        "Analytics" => ["분석", "Analytics", "分析", "分析", "分析"],
        "Settings" => ["설정", "Settings", "設定", "设置", "設定"],
        "Probability & Refund Simulator" => ["확률 및 환급 시뮬레이터", "Probability & Refund Simulator", "確率・還元シミュレーター", "概率与返还模拟器", "機率與回饋模擬器"],
        "Search games or sections..." => ["게임 또는 메뉴 검색...", "Search games or sections...", "ゲーム・メニューを検索...", "搜索游戏或页面...", "搜尋遊戲或頁面..."],
        "Display & behavior settings" => ["화면 및 동작 설정", "Display & behavior settings", "表示・動作設定", "显示与行为设置", "顯示與行為設定"],
        "Recent simulations" => ["최근 시뮬레이션", "Recent simulations", "最近のシミュレーション", "最近的模拟", "最近的模擬"],
        "View all history" => ["전체 기록 보기", "View all history", "履歴をすべて表示", "查看所有记录", "查看所有紀錄"],
        "Select game" => ["게임 바로 선택", "Select game", "ゲームを選択", "选择游戏", "選擇遊戲"],
        "Simulation Settings" => ["시뮬레이션 설정", "Simulation Settings", "シミュレーション設定", "模拟设置", "模擬設定"],
        "Game" => ["게임", "Game", "ゲーム", "游戏", "遊戲"],
        "Target Character" => ["목표 캐릭터", "Target Character", "目標キャラクター", "目标角色", "目標角色"],
        "Target Weapon" => ["목표 무기", "Target Weapon", "目標武器", "目标武器", "目標武器"],
        "Pity (Current)" => ["현재 천장 스택", "Pity (Current)", "現在の天井カウント", "当前保底次数", "目前保底次數"],
        "Rate-up Guarantee" => ["픽업 확정", "Rate-up Guarantee", "ピックアップ確定", "UP 保底", "UP 保底"],
        "Premium Currency" => ["프리미엄 재화", "Premium Currency", "プレミアム通貨", "高级货币", "高級貨幣"],
        "Refund Currency" => ["환급 재화", "Refund Currency", "還元通貨", "返还货币", "回饋貨幣"],
        "Shared Currency" => ["공용 재화", "Shared Currency", "共通通貨", "通用货币", "通用貨幣"],
        "Character Tickets" => ["캐릭터 티켓", "Character Tickets", "キャラチケット", "角色票", "角色票"],
        "Weapon Tickets" => ["무기 티켓", "Weapon Tickets", "武器チケット", "武器票", "武器票"],
        "No target" => ["선택 안 함", "No target", "選択なし", "未选择", "未選擇"],
        "Not needed" => ["필요 없음", "Not needed", "不要", "不需要", "不需要"],
        "Always On" => ["항상 적용", "Always On", "常に適用", "始终启用", "始終啟用"],
        "Run Simulation" => ["시뮬레이션 실행", "Run Simulation", "シミュレーション実行", "运行模拟", "執行模擬"],
        "Reset" => ["초기화", "Reset", "リセット", "重置", "重設"],
        "Advanced settings" => ["상세 설정", "Advanced settings", "詳細設定", "详细设置", "詳細設定"],
        "Target Success Rate" => ["목표 달성 확률", "Target Success Rate", "目標達成確率", "目标达成概率", "目標達成機率"],
        "Expected Pulls" => ["평균 뽑기 횟수", "Expected Pulls", "平均ガチャ回数", "平均抽数", "平均抽數"],
        "Remaining Currency" => ["잔여 재화", "Remaining Currency", "残り通貨", "剩余货币", "剩餘貨幣"],
        "Character / Weapon Tickets" => ["캐릭터 / 무기 티켓", "Character / Weapon Tickets", "キャラ／武器チケット", "角色／武器票", "角色／武器票"],
        "Character Probability" => ["캐릭터 달성 확률", "Character Probability", "キャラ達成確率", "角色达成概率", "角色達成機率"],
        "Probability" => ["확률", "Probability", "確率", "概率", "機率"],
        "Overall Result" => ["전체 결과", "Overall Result", "総合結果", "综合结果", "綜合結果"],
        "Success" => ["성공", "Success", "成功", "成功", "成功"],
        "Total Success" => ["전체 성공", "Total Success", "総合成功", "总体成功", "整體成功"],
        "Accurate probability DP" => ["정확 확률 DP", "Exact probability DP", "正確な確率DP", "精确概率 DP", "精確機率 DP"],
        "Including recycled refunds" => ["환급 재투입 포함", "Including recycled refunds", "還元再投入を含む", "包含返还再投入", "包含回饋再投入"],
        "Average pulls to all targets" => ["목표까지의 평균 총 뽑기", "Average pulls to all targets", "目標達成までの平均回数", "达成目标的平均抽数", "達成目標的平均抽數"],
        "Shared character/weapon ticket" => ["캐릭터·광추 공용 티켓", "Shared character/weapon ticket", "キャラ・光円錐共通チケット", "角色／光锥共用票", "角色／光錐共用票"],
        "Changes pending. Select Run Simulation to update." => ["입력이 변경되었습니다. 시뮬레이션 실행으로 갱신하세요.", "Inputs changed. Select Run Simulation to update.", "入力が変更されました。実行して更新してください。", "输入已变更。运行模拟以更新。", "輸入已變更。執行模擬以更新。"],
        "Calculating in background. Previous results remain visible." => ["백그라운드 계산 중 · 이전 결과를 유지합니다.", "Calculating in background. Previous results remain visible.", "バックグラウンドで計算中。前の結果を表示します。", "后台计算中，保留之前的结果。", "背景運算中，保留之前的結果。"],
        "Probability Overview" => ["확률 개요", "Probability Overview", "確率概要", "概率概览", "機率概覽"],
        "Calculating probability and refunds..." => ["확률과 환급을 계산하고 있습니다.", "Calculating probability and refunds...", "確率と還元を計算しています...", "正在计算概率与返还...", "正在計算機率與回饋..."],
        "No calculation available" => ["계산 결과가 없습니다.", "No calculation available", "計算結果はありません", "尚无计算结果", "尚無計算結果"],
        "Result Summary" => ["결과 요약", "Result Summary", "結果の概要", "结果摘要", "結果摘要"],
        "Refund Loop" => ["환급 재투입", "Refund Loop", "還元再投入", "返还循环", "回饋循環"],
        "Refunds recycled" => ["반복 재투입 적용", "Refunds recycled", "還元を再投入", "返还已重新投入", "回饋已重新投入"],
        "Extra Pulls from Refund" => ["환급으로 추가한 뽑기", "Extra Pulls from Refund", "還元による追加回数", "返还额外抽数", "回饋額外抽數"],
        "Estimated Outcome" => ["예상 결과", "Estimated Outcome", "推定結果", "预估结果", "預估結果"],
        "Overall" => ["전체", "Overall", "総合", "总体", "整體"],
        "Simulation History" => ["시뮬레이션 기록", "Simulation History", "シミュレーション履歴", "模拟历史", "模擬歷史"],
        "No simulation yet" => ["시뮬레이션 기록 없음", "No simulation yet", "シミュレーション履歴なし", "暂无模拟", "暫無模擬"],
        "View All History" => ["전체 기록 보기", "View All History", "履歴をすべて表示", "查看全部记录", "查看全部紀錄"],
        "Calculating" => ["계산 중", "Calculating", "計算中", "计算中", "計算中"],
        "Use game-specific guarantee" => ["게임별 별도 보장", "Use game-specific guarantee", "ゲーム別の保証方式", "使用游戏特定保底", "使用遊戲特定保底"],
        "Target settings and refund recycling are shared across tabs." => ["캐릭터·무기 목표와 환급 재투입을 계산합니다.", "Target settings and refund recycling are shared across tabs.", "キャラ・武器目標と還元再投入を計算します。", "角色、武器目标与返还循环在各页面同步。", "角色、武器目標與回饋循環在各頁同步。"],
        "Individual curves assume full allocation of flexible tickets." => ["단독 그래프는 공용 티켓 전량 배정을 가정합니다.", "Individual curves assume full allocation of flexible tickets.", "個別グラフは共通チケットの全量配分を仮定します。", "单独曲线假设全部通用票投入对应卡池。", "單獨曲線假設全部通用票投入對應卡池。"],
        "Application settings" => ["화면 및 동작 설정", "Application settings", "アプリ設定", "应用设置", "應用設定"],
        "Change the appearance and calculation behavior, not banner inputs." => ["뽑기 설정이 아닌 프로그램의 표시와 계산 동작을 변경합니다.", "Change the appearance and calculation behavior, not banner inputs.", "ガチャ設定ではなく、表示と計算動作を変更します。", "调整应用外观和计算行为，而非抽卡参数。", "調整應用外觀和計算行為，而非抽卡參數。"],
        "Auto calculation" => ["실시간 계산", "Auto calculation", "自動計算", "自动计算", "自動計算"],
        "Update results automatically when input changes" => ["입력 변경 시 결과 자동 갱신", "Update results automatically when input changes", "入力変更時に結果を自動更新", "输入改变时自动更新结果", "輸入改變時自動更新結果"],
        "Input debounce" => ["연속 입력 대기시간", "Input debounce", "入力待機時間", "输入防抖延迟", "輸入防抖延遲"],
        "Run calculation now" => ["현재 입력으로 계산 실행", "Run calculation now", "現在の入力で計算", "立即计算", "立即計算"],
        "Display options" => ["대시보드 표시", "Display options", "表示オプション", "显示选项", "顯示選項"],
        "Show right result/history panel" => ["오른쪽 결과 및 이력 패널 표시", "Show right result/history panel", "右側の結果・履歴パネルを表示", "显示右侧结果与历史面板", "顯示右側結果與歷史面板"],
        "Fill area under probability curves" => ["확률 그래프 아래 반투명 영역 표시", "Fill area under probability curves", "確率曲線の下を塗りつぶす", "填充概率曲线下的区域", "填充機率曲線下的區域"],
        "Show in-chart hover probability" => ["마우스 위치에서 차트 내부 확률 표시", "Show in-chart hover probability", "グラフ内にホバー確率を表示", "仅在图表内显示悬停概率", "僅在圖表內顯示懸停機率"],
        "Compact spacing" => ["화면 내부 간격을 컴팩트하게 표시", "Compact spacing", "コンパクトな余白", "紧凑间距", "緊湊間距"],
        "Restore defaults" => ["화면 설정 기본값 복원", "Restore defaults", "設定を初期化", "恢复默认设置", "恢復預設設定"],
        "Other display preferences reset at exit." => ["언어 외 화면 설정은 프로그램 종료 시 초기화됩니다.", "Other display preferences reset at exit.", "言語以外の表示設定は終了時に初期化されます。", "语言以外的显示设置会在退出时重置。", "語言以外的顯示設定會在退出時重設。"],
        "Edit only the selected banner; values are shared across tabs." => ["선택한 배너만 수정합니다. 설정은 모든 탭에 연동됩니다.", "Edit only the selected banner; values are shared across tabs.", "選択したガチャのみ編集します。設定は各タブで共有されます。", "仅编辑当前卡池；数据在各页面同步。", "僅編輯目前卡池；資料在各頁同步。"],
        "Character targets" => ["캐릭터 목표 관리", "Character targets", "キャラ目標の管理", "角色目标", "角色目標"],
        "Weapon targets" => ["무기·광추·레코드·Arc 목표 관리", "Weapon / Light Cone / Record / Arc targets", "武器・光円錐・レコード・Arcの目標", "武器／光锥／唱片／Arc 目标", "武器／光錐／唱片／Arc 目標"],
        "Currency and refunds" => ["재화 및 환급 설정", "Currency and refunds", "通貨・還元設定", "货币与返还设置", "貨幣與回饋設定"],
        "View Dashboard" => ["대시보드 보기", "View Dashboard", "ダッシュボードを表示", "查看仪表盘", "查看儀表板"],
        "Probability model" => ["확률 모델", "Probability model", "確率モデル", "概率模型", "機率模型"],
        "Published base rates and hard pity" => ["공개 기본 확률 + 확정 천장", "Published base rates and hard pity", "公開基本確率＋確定天井", "公布基础概率与硬保底", "公布基礎機率與硬保底"],
        "Estimated soft pity model" => ["소프트 천장 추정 모델", "Estimated soft pity model", "推定ソフト天井モデル", "软保底估计模型", "軟保底估計模型"],
        "Rules, confidence and sources" => ["게임별 규칙 · 신뢰도 · 출처", "Rules, confidence and sources", "ゲーム別ルール・信頼度・出典", "游戏规则、可信度与来源", "遊戲規則、可信度與來源"],
        "Current-session results, cleared on exit." => ["현재 실행에서 계산한 최근 결과입니다. 종료 시 초기화됩니다.", "Current-session results, cleared on exit.", "現在のセッションの結果です。終了時に消去されます。", "仅显示本次运行的结果，退出后清除。", "僅顯示本次執行的結果，離開後清除。"],
        _ => [text, text, text, text, text],
    };
    values[language.index()]
}
pub fn game_name(language: Language, game: Game) -> &'static str {
    match (language,game) {
        (Language::Korean,_) => game.label(),
        (_, Game::StarRail) => "Honkai: Star Rail",
        (_, Game::WutheringWaves) => "Wuthering Waves",
        (_, Game::StellaSora) => "Stella Sora",
        (_, Game::Neverness) => "Neverness to Everness",
    }
}
pub fn banner_name(language: Language, game: Game) -> &'static str {
    match (language,game) {
        (Language::Korean,_) => game.secondary_banner_name(),
        (Language::Japanese, Game::StarRail) => "光円錐",
        (Language::ChineseSimplified, Game::StarRail) => "光锥",
        (Language::ChineseTraditional, Game::StarRail) => "光錐",
        (_, Game::StarRail) => "Light Cone",
        (_, Game::WutheringWaves) => "Weapon",
        (_, Game::StellaSora) => "Record",
        (_, Game::Neverness) => "Arc",
    }
}
pub fn currency_name(language: Language, game: Game) -> &'static str {
    match (language,game) {
        (Language::Korean,_) => game.currency_name(),
        (Language::Japanese, Game::StarRail) => "星玉",
        (Language::ChineseSimplified, Game::StarRail) => "星琼",
        (Language::ChineseTraditional, Game::StarRail) => "星瓊",
        (_,Game::StarRail) => "Stellar Jade",
        (Language::Japanese, Game::WutheringWaves) => "星声",
        (Language::ChineseSimplified, Game::WutheringWaves) => "星声",
        (Language::ChineseTraditional, Game::WutheringWaves) => "星聲",
        (_,Game::WutheringWaves) => "Astrite",
        (Language::Japanese, Game::StellaSora) => "星の欠片",
        (Language::ChineseSimplified, Game::StellaSora) => "星之碎片",
        (Language::ChineseTraditional, Game::StellaSora) => "星之碎片",
        (_,Game::StellaSora) => "Stellar Fragments",
        (_,Game::Neverness) => "Annulith",
    }
}
pub fn ticket_name(language: Language, game: Game, weapon: bool) -> &'static str {
    if language == Language::Korean {
        return if weapon { game.weapon_ticket_name() } else { game.character_ticket_name() };
    }
    match (game,weapon) {
        (Game::StarRail,_) => "Star Rail Special Pass",
        (Game::WutheringWaves,false) => "Radiant Tide",
        (Game::WutheringWaves,true) => "Forging Tide",
        (Game::StellaSora,false) => "Cerulean Ticket",
        (Game::StellaSora,true) => "Gold Disc",
        (Game::Neverness,false) => "Solid Dice",
        (Game::Neverness,true) => "Tri-Key",
    }
}
pub fn refund_name(language: Language, game: Game) -> &'static str {
    if language == Language::Korean { return game.exchange_points_name(); }
    match game { Game::StarRail => "Undying Starlight",
        Game::WutheringWaves => "Afterglow Coral",
        Game::StellaSora => "Explorer Expert Certificate",
        Game::Neverness => "Warp Pieces" }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_region_and_variants() {
        assert_eq!(parse_system_locale("ko_KR.UTF-8"), Language::Korean);
        assert_eq!(parse_system_locale("ja-JP"), Language::Japanese);
        assert_eq!(parse_system_locale("zh_CN.UTF-8"), Language::ChineseSimplified);
        assert_eq!(parse_system_locale("zh_Hant_TW"), Language::ChineseTraditional);
        assert_eq!(parse_system_locale("zh_TW"), Language::ChineseTraditional);
        assert_eq!(parse_system_locale("en-US"), Language::English);
        assert_eq!(parse_system_locale("fr-FR"), Language::English);
    }
    #[test]
    fn preference_selection_overrides_system_language() {
        assert_eq!(LanguagePreference::Automatic.effective(Language::Japanese), Language::Japanese);
        assert_eq!(LanguagePreference::Korean.effective(Language::Japanese), Language::Korean);
        assert_eq!(LanguagePreference::from_key("zh-tw"), Some(LanguagePreference::ChineseTraditional));
        assert_eq!(LanguagePreference::from_key("invalid"), None);
    }
}