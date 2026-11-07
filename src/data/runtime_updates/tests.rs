
    use super::*;

    #[test]
    fn url_allowlist_rejects_unpinned_and_escaped_update_paths() {
        let rev = "a".repeat(40);
        assert!(approved_update_url(&format!("{COMMIT_API}{rev}")));
        for resource in [
            "src/catalog_data.tsv", "src/signature_pairs.tsv",
            "Image/SHA256SUMS.txt", "Image/HSR/c-1511.png",
            "Image/StellaSora/w-214066.png",
        ] {
            assert!(approved_update_url(&source_url(&rev, resource)), "{resource}");
        }
        for url in [
            format!("{REPO_RAW}main/src/catalog_data.tsv"),
            format!("{REPO_RAW}{rev}/Image/HSR/../c-1511.png"),
            format!("{REPO_RAW}{rev}/Image/HSR/%2e%2e.png"),
            format!("{REPO_RAW}{rev}/Image/HSR/c-1511.png?x=1"),
            format!("{REPO_RAW}{rev}/Image/Other/c-1511.png"),
            format!("{REPO_RAW}{rev}/src/lib.rs"),
            format!("{REPO_RAW}{rev}/Image/HSR/c-1511.svg"),
            format!("{COMMIT_API}main"),
            format!("{COMMIT_API}{rev}?page=1"),
            "https://evil.invalid/".to_owned(),
        ] {
            assert!(!approved_update_url(&url), "{url}");
        }
    }

    #[test]
    fn commit_attestation_check_rejects_unsigned_untrusted_and_wrong_revisions() {
        let sha = "a".repeat(40);
        let good = format!(
            "{{\"sha\":\"{sha}\",\"commit\":{{\"committer\":{{\"name\":\"GitHub\",\"email\":\"noreply@github.com\"}},\"verification\":{{\"verified\":true,\"reason\":\"valid\"}}}},\"author\":{{\"login\":\"P-JSL\"}}}}"
        );
        assert!(verified_github_commit(&good, &sha));
        assert!(!verified_github_commit(&good.replace("\"verified\":true", "\"verified\":false"), &sha));
        assert!(!verified_github_commit(&good.replace("\"login\":\"P-JSL\"", "\"login\":\"other\""), &sha));
        assert!(!verified_github_commit(&good, &"b".repeat(40)));
        assert!(!verified_github_commit("malformed", &sha));
    }

    #[test]
    fn rejects_poisoned_or_partial_candidate_data() {
        let catalog = include_str!("../catalog_data.tsv");
        let pairs = include_str!("../signature_pairs.tsv");
        let manifest = include_str!("../../Image/SHA256SUMS.txt");
        let rev = "a".repeat(40);
        assert!(parse_bundle(catalog, pairs, manifest, rev.clone(), PathBuf::new()).is_ok());
        assert!(parse_bundle(catalog, pairs, "", rev.clone(), PathBuf::new()).is_err());
        assert!(parse_bundle(catalog, pairs, manifest, "invalid".into(), PathBuf::new()).is_err());
        let broken = pairs.replace("hsr\t1511\t23065\t", "hsr\t1511\t999999\t");
        assert!(parse_bundle(catalog, &broken, manifest, rev, PathBuf::new()).is_err());
    }
    #[test]
    fn bounded_cache_rejects_oversize_non_utf8_and_symlink_files() {
        let root = std::env::temp_dir().join(format!(
            "gacha-cache-test-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("catalog.tsv");
        fs::write(&path, b"0123456789").unwrap();
        assert!(read_bounded_cache(&path, 9).is_err());
        assert_eq!(read_bounded_cache(&path, 10).unwrap(), "0123456789");
        fs::write(&path, [0xff]).unwrap();
        assert!(read_bounded_cache(&path, 10).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let link = root.join("link");
            symlink(&path, &link).unwrap();
            assert!(read_bounded_cache(&link, 10).is_err());
            assert!(write_atomic(&link, b"safe").is_err());
        }
        write_atomic(&path, b"replacement").unwrap();
        assert_eq!(read_bounded_cache(&path, 11).unwrap(), "replacement");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn rejects_unexpected_release_versions_and_unsafe_urls() {
        assert!(newer_version("v0.2.3", "0.2.2"));
        assert!(!newer_version("v0.2.2", "0.2.2"));
        assert!(!newer_version("v0.2.4-evil", "0.2.2"));
        assert!(!is_commit_sha("../main"));
        assert!(valid_id("upcoming_astra_yao"));
        assert!(!valid_id("../bad"));
        assert_eq!(json_field(r#"{"tag_name":"v0.2.3"}"#, "tag_name").as_deref(), Some("v0.2.3"));
    }
