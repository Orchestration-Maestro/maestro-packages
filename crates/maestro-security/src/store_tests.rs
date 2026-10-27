//! Public storage regressions and baseline behaviours.
use super::{RunRecord, Store};

/// Missing workspace documents are empty collections, ready for first use.
#[test]
fn missing_decks_and_credentials_are_empty() {
    crate::test_support::isolated(|_| {
        assert!(Store.load_cards("new").expect("missing deck").is_empty());
        assert!(
            Store
                .load_creds("new")
                .expect("missing credentials")
                .is_empty()
        );
    });
}

/// Listing follows start time, including backfilled and replaced records.
#[test]
fn runs_are_listed_by_start_time_after_upserts() {
    crate::test_support::isolated(|_| {
        for (id, started_at) in [("later", 30), ("early", 10), ("middle", 20)] {
            let mut record = RunRecord::starting(id.into(), "target", "agent", "model", None);
            record.started_at = started_at;
            Store.upsert_run(&record).expect("insert");
        }
        let ids = |limit| {
            Store
                .list_runs(limit)
                .expect("list")
                .into_iter()
                .map(|run| run.run_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(0), ["later", "middle", "early"]);
        assert_eq!(ids(2), ["later", "middle"]);
        let mut early = Store.get_run("early").unwrap().unwrap();
        early.started_at = 40;
        Store.upsert_run(&early).expect("replace");
        assert_eq!(ids(0), ["early", "later", "middle"]);
    });
}
