
    use super::*;

    #[test]
    fn poisoned_catalog_lock_refuses_to_read_unvalidated_state() {
        let isolated = Arc::new(RwLock::new(None));
        let writer = Arc::clone(&isolated);
        assert!(std::thread::spawn(move || {
            let _guard = writer.write().expect("fresh lock");
            panic!("intentional write-lock poisoning");
        }).join().is_err());
        assert!(snapshot_from_lock(&isolated).is_err());
    }
    #[test]
    fn upcoming_and_released_characters_are_prioritized() {
        assert_eq!(first(Game::StarRail, Kind::Character), "1511");
        assert_eq!(first(Game::Neverness, Kind::Character), "upcoming_exe");
        assert_eq!(first(Game::StellaSora, Kind::Character), "162");
        assert_eq!(first(Game::WutheringWaves, Kind::Character), "1312");
    }
    #[test]
    fn image_download_policy_rejects_untrusted_servers_and_payloads() {
        assert!(approved_icon_url("https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.png"));
        for url in [
            "http://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.png",
            "https://raw.githubusercontent.com.evil.invalid/a.png",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/../private.png",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.png?token=1",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/icon.svg",
            "https://raw.githubusercontent.com/Mar-7th/StarRailRes/main/%2e%2e/icon.png",
            "https://127.0.0.1/image.png",
        ] { assert!(!approved_icon_url(url), "{url}"); }
        assert!(!approved_icon_bytes(b"<svg xmlns='http://www.w3.org/2000/svg'/>"));
        assert!(!approved_icon_bytes(b"MZ executable"));
        assert!(approved_icon_bytes(b"\x89PNG\r\n\x1a\nnot-a-valid-image"));
        assert!(CatalogUi::fetch_icon("HSR/../../evil.png").is_none());
        assert!(CatalogUi::fetch_icon("HSR/asset.svg").is_none());
    }
    #[test]
    fn signature_pairs_resolve_only_to_real_five_star_catalog_entries() {
        let mut pairs = HashSet::new();
        let mut count = 0;
        for line in include_str!("../../signature_pairs.tsv").lines() {
            if line.starts_with('#') || line.trim().is_empty() { continue; }
            let parts: Vec<_> = line.split('\t').collect();
            assert_eq!(parts.len(), 4, "invalid signature mapping: {line}");
            assert!(parts[3].starts_with("https://"), "unverified source: {line}");
            let game = match parts[0] {
                "hsr" => Game::StarRail, "wuwa" => Game::WutheringWaves,
                "nte" => Game::Neverness, "sora" => Game::StellaSora,
                _ => panic!("unsupported game: {line}"),
            };
            let character = by_id(game, Kind::Character, parts[1])
                .unwrap_or_else(|| panic!("unknown character: {line}"));
            assert!(character.is_featured_target(), "non-banner character: {line}");
            let weapon = by_id(game, Kind::Weapon, parts[2])
                .unwrap_or_else(|| panic!("unknown weapon: {line}"));
            assert_eq!(weapon.rarity, 5, "non-5-star weapon: {line}");
            assert!(pairs.insert((parts[0], parts[1])), "duplicated mapping: {line}");
            assert_eq!(signature_weapon(game, parts[1]).unwrap().id, parts[2]);
            count += 1;
        }
        assert_eq!(count, 141, "signature mapping coverage changed unexpectedly");
        assert_eq!(signature_weapon(Game::StarRail, "1511").unwrap().id, "23065");
        assert!(signature_weapon(Game::StarRail, "upcoming_ellen_joe").is_none());
        assert!(signature_weapon(Game::StarRail, "upcoming_astra_yao").is_none());
        assert_eq!(by_id(Game::StarRail, Kind::Weapon, "22009").unwrap().rarity, 4);
        assert_eq!(signature_weapon(Game::Neverness, "1056").map(|e| &e.id), None);
        assert!(signature_weapon(Game::StarRail, "8001").is_none());
        // IDs come from the official 2.0 data snapshot, not earlier aliases.
        assert_eq!(signature_weapon(Game::StellaSora, "162").unwrap().id, "214066");
        assert!(by_id(Game::StellaSora, Kind::Character, "firstlight_noya").is_none());
        assert!(by_id(Game::StellaSora, Kind::Weapon, "guiding_star_disc").is_none());
        assert_eq!(signature_weapon(Game::StellaSora, "139").unwrap().id, "214061");
    }

    #[test]
    fn all_unpaired_five_star_targets_are_explicitly_classified() {
        let mut exceptions = HashSet::new();
        for line in include_str!("../../unpaired_five_star_targets.tsv").lines() {
            if line.starts_with('#') || line.is_empty() { continue; }
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 4, "bad unpaired target: {line}");
            let game = match fields[0] {
                "hsr" => Game::StarRail, "wuwa" => Game::WutheringWaves,
                "nte" => Game::Neverness, "sora" => Game::StellaSora,
                _ => panic!("unknown game in exception: {line}"),
            };
            let ch = by_id(game, Kind::Character, fields[1]).expect("unknown exception identity");
            assert_eq!(ch.name, fields[3]);
            assert_eq!(ch.rarity, 5);
            assert!(signature_weapon(game, fields[1]).is_none(),
                "exception has a mapped signature: {line}");
            assert!(exceptions.insert((game, fields[1])), "duplicate exception: {line}");
        }
        assert_eq!(exceptions.len(), 30, "unmapped target coverage changed");
        for entry in entries().iter().filter(|entry|
            entry.kind == Kind::Character && entry.rarity == 5
        ) {
            if signature_weapon(entry.game, &entry.id).is_none() {
                assert!(exceptions.contains(&(entry.game, entry.id.as_str())),
                    "five-star character missing a signature and an explicit exception: {}",
                    entry.id);
            }
        }
    }

    #[test]
    fn portraits_and_names_do_not_cross_link_between_character_identities() {
        let mut names = HashSet::new();
        let mut source_urls = HashSet::new();
        for e in entries() {
            assert!(names.insert((game_code(e.game), e.kind as u8, e.name.as_str())),
                "duplicate visible name for different IDs: {} {}", game_code(e.game), e.name);
            if !e.icon_url.is_empty() {
                assert!(source_urls.insert((game_code(e.game), e.kind as u8, e.icon_url.as_str())),
                    "same upstream portrait assigned to multiple identities: {}", e.name);
            }
            assert!(!e.name.trim().is_empty());
            assert!(!e.name.starts_with('{') && !e.name.chars().all(|ch| ch.is_ascii_digit()),
                "placeholder name leaked into catalog: {}", e.name);
        }
        assert_eq!(by_id(Game::Neverness, Kind::Character, "1091").map(|e| e.name.as_str()), None);
        assert_eq!(by_id(Game::Neverness, Kind::Character, "1051").map(|e| e.name.as_str()), None);
        let latest_female = by_id(Game::StarRail, Kind::Character, "8010").unwrap();
        assert_eq!(latest_female.name, "개척자 (여성 · 환락 · 번개)");
        assert!(!latest_female.is_featured_target());
    }

    #[test]
    fn all_games_have_both_named_catalogs() {
        let mut unique = HashSet::new();
        for game in Game::ALL {
            assert!(count(game, Kind::Character) > 0);
            assert!(count(game, Kind::Weapon) > 0);
            let first = first(game, Kind::Character);
            assert!(by_id(game, Kind::Character, &first).is_some());
        }
        for item in entries() {
            assert!(!item.name.trim().is_empty());
            if !item.icon_url.is_empty() {
                assert!(item.has_verified_image(),
                    "{} {} must use an approved image source", item.id, item.name);
            }
            assert!(unique.insert((game_code(item.game), item.kind as u8, item.id.clone())),
                "duplicate catalog id: {}", item.id);
        }
        assert_eq!(count(Game::Neverness, Kind::Weapon), 51);
        assert_eq!(count(Game::Neverness, Kind::Character), 26);
        assert!(count(Game::WutheringWaves, Kind::Character) >= 60);
    }
