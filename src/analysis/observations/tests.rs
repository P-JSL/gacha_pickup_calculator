
    use super::*;

    #[test]
    fn reject_excessive_columns_and_duplicate_case_insensitive_headers() {
        let many = format!("pity,count,{}\n1,1\n",
            (0..63).map(|i| format!("extra_{i}")).collect::<Vec<_>>().join(","));
        assert!(parse_observation_csv(&many, 90).is_err());
        assert!(parse_observation_csv("pity,count,COUNT\n1,1,2\n", 90).is_err());
        assert!(parse_observation_csv("pity,count,extra\n1,2,3\n", 90).is_ok());
    }

    use crate::{SoftPity};

    fn constant_model(hard: usize, rate: f64) -> RateSchedule {
        RateSchedule { base: rate, hard, soft: SoftPity::None }
    }

    #[test]
    fn correctly_accounts_for_right_censoring() {
        // First completion at 2, second completion at 4, one stopped after 3.
        // At pity 2: 1/3 events. At pity 4: 1/1, not 1/2.
        let d = parse_observation_csv("duration,event\n2,1\n3,0\n4,1\n", 5).unwrap();
        assert_eq!(d.quality, ObservationQuality::IndividualRightCensored);
        assert_eq!(d.observations, 3);
        assert_eq!(d.events, 2);
        assert_eq!(d.censored, 1);
        assert!((d.rows[1].survival - 2.0 / 3.0).abs() < 1e-12);
        assert!((d.rows[3].survival - 0.0).abs() < 1e-12);
        assert_eq!(d.rows[3].at_risk, 1);
    }

    #[test]
    fn aggregated_risk_set_matches_individuals() {
        let individual = parse_observation_csv("duration,event\n2,1\n3,0\n4,1\n", 5).unwrap();
        let aggregated = parse_observation_csv(
            "pity,at_risk,events,censored\n1,3,0,0\n2,3,1,0\n3,2,0,1\n4,1,1,0\n", 5
        ).unwrap();
        for (a,b) in individual.rows.iter().zip(aggregated.rows.iter()) {
            assert_eq!(a.at_risk,b.at_risk);
            assert_eq!(a.events,b.events);
            assert!((a.survival-b.survival).abs() < 1e-12);
        }
    }

    #[test]
    fn samples_with_only_winning_pities_are_flagged_as_biased() {
        let d = parse_observation_csv("pity,count\n1,5\n2,5\n", 90).unwrap();
        assert_eq!(d.quality, ObservationQuality::CompletedOnly);
        assert!(!d.quality.is_censor_aware());
        assert_eq!(d.events, 10);
    }

    #[test]
    fn rejects_inconsistent_aggregated_risk_set() {
        let bad = "pity,at_risk,events,censored\n1,3,1,0\n2,3,0,0\n";
        assert!(parse_observation_csv(bad, 90).is_err());
    }

    #[test]
    fn rejects_out_of_order_and_excess_hard_pity() {
        let bad = "pity,count\n2,1\n1,2\n";
        assert!(parse_observation_csv(bad, 90).is_err());
        assert!(parse_observation_csv("duration,event\n91,1\n", 90).is_err());
    }

    #[test]
    fn detects_model_cdf_mismatch() {
        let d = parse_observation_csv("duration,event\n2,1\n3,0\n4,1\n", 5).unwrap();
        let report = d.compare(&constant_model(5, 0.5), RateMode::PublishedOnly).unwrap();
        assert!(report.restricted_observed_mean > 0.0);
        assert_eq!(report.reliable_risk_bins, 0);
    }


    #[test]
    fn complete_known_hazard_matches_kaplan_meier_with_no_censoring() {
        let csv = "pity,at_risk,events,censored\n1,100,50,0\n2,50,25,0\n3,25,25,0\n";
        let data = parse_observation_csv(csv, 3).unwrap();
        let c = data.compare(&constant_model(3, 0.5), RateMode::PublishedOnly).unwrap();
        assert_eq!(data.observations, 100);
        assert_eq!(data.events, 100);
        assert_eq!(data.censored, 0);
        assert_eq!(c.reliable_risk_bins, 3);
        assert!(c.max_cdf_delta < 1e-12);
        assert!(c.mean_abs_hazard_delta < 1e-12);
        let (lower, upper) = data.rows[1].cdf_ci_95.expect("interior KM confidence");
        assert!(lower <= 0.75);
        assert!(upper >= 0.75);
        assert!(data.rows[2].cdf_ci_95.is_none());
    }

    #[test]
    fn extreme_counts_are_rejected_without_integer_overflow() {
        let csv = "pity,at_risk,events,censored\n1,18446744073709551615,18446744073709551615,18446744073709551615\n";
        assert!(parse_observation_csv(csv, 90).is_err());
    }

    #[test]
    fn all_censored_cohort_is_valid_but_has_no_greenwood_band() {
        let data = parse_observation_csv("duration,event\n5,0\n5,0\n5,0\n", 90).unwrap();
        assert_eq!(data.observations, 3);
        assert_eq!(data.events, 0);
        assert_eq!(data.censored, 3);
        assert!(data.rows.iter().all(|r| r.survival == 1.0 && r.cdf_ci_95.is_none()));
    }

    #[test]
    fn partial_risk_set_header_cannot_be_interpreted_as_completed_only() {
        for csv in [
            "pity,at_risk,events\n1,100,5\n2,95,6\n",
            "pity,events,censored\n1,5,3\n",
            "pity,count,censored\n1,5,3\n",
            "pity,at_risk,count\n1,10,5\n",
            "pity,at_risk,events,censored,count\n1,10,5,5,5\n",
            "duration,pity,count\n1,1,1\n",
            "duration,event,pity,count\n1,1,1,1\n",
            "duration,duration,event\n1,1,1\n",
            "pity,pity,count\n1,1,2\n",
        ] {
            assert!(parse_observation_csv(csv, 90).is_err(), "{csv}");
        }
    }

    #[test]
    fn exact_synthetic_20_persons_match_constant_half_rate_until_pity_one() {
        let mut input = String::from("duration,event\n");
        for _ in 0..10 { input.push_str("1,1\n"); }
        for _ in 0..10 { input.push_str("2,1\n"); }
        let d = parse_observation_csv(&input, 3).unwrap();
        assert_eq!(d.rows[0].at_risk, 20);
        assert!((d.rows[0].hazard - 0.5).abs() < 1e-12);
        assert_eq!(d.compare(&constant_model(3, 0.5), RateMode::PublishedOnly)
            .unwrap().reliable_risk_bins, 1);
    }
