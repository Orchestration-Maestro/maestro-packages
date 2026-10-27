//! Baseline semantic memory behaviours through the public index.
use super::{Memory, embed};

/// Baseline: indexed findings survive reload, rank by relevance and respect limits.
#[test]
fn baseline_persistent_search() {
    crate::test_support::isolated(|_| {
        let mut memory = Memory::load().expect("missing index");
        let unrelated = memory
            .index("cache.test", "http cache stale response")
            .unwrap();
        let related = memory.index("ssh.test", "ssh root login exposed").unwrap();
        assert_ne!(unrelated, related);
        let loaded = Memory::load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert!(!loaded.is_empty());
        let hits = loaded.search("SSH root login exposed", 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, related);
        assert_eq!(hits[0].target, "ssh.test");
        assert_eq!(hits[0].text, "ssh root login exposed");
        assert!((hits[0].score - 1.0).abs() < 0.00001);
        assert!(hits[0].score > hits[1].score);
        let mut loaded = loaded;
        let third = loaded.index("third.test", "new finding").unwrap();
        assert_ne!(third, related);
        assert_ne!(third, unrelated);
        assert_eq!(Memory::load().unwrap().len(), 3);
    });
}

/// Baseline: casing and punctuation do not change matching; empty text stays finite.
#[test]
fn baseline_embedding_normalization() {
    let normalized = embed("SSH root-login 22");
    assert_eq!(normalized, embed("ssh ROOT login 22"));
    let norm: f32 = normalized.iter().map(|value| value * value).sum();
    assert!((norm - 1.0).abs() < 0.00001);
    assert!(embed(" \n!!!").iter().all(|value| *value == 0.0));
}

/// Baseline: a malformed index is rejected and failed persistence is surfaced.
#[test]
fn baseline_memory_storage_errors() {
    crate::test_support::isolated(|home| {
        std::fs::write(home.join("memory.json"), "{bad").unwrap();
        assert!(Memory::load().is_err());
        std::fs::remove_file(home.join("memory.json")).unwrap();
        std::fs::create_dir(home.join("memory.json")).unwrap();
        assert!(
            Memory::new()
                .index("example.test", "observed finding")
                .is_err()
        );
    });
}

/// Baseline: empty indexes and blank queries produce no hits; limits are honored.
#[test]
fn baseline_empty_search_and_limits() {
    crate::test_support::isolated(|_| {
        let mut memory = Memory::new();
        assert!(memory.is_empty());
        assert_eq!(memory.len(), 0);
        assert!(memory.search("ssh", 2).is_empty());
        memory.index("ssh.test", "ssh root login").unwrap();
        memory.index("cache.test", "http cache").unwrap();
        assert!(!memory.is_empty());
        assert_eq!(memory.search("ssh", 1).len(), 1);
        assert!(memory.search(" \t\n", 2).is_empty());
        assert!(memory.search("ssh", 0).is_empty());
    });
}

/// An existing but unreadable index must not be replaced by empty memory.
#[test]
fn unreadable_memory_index_is_an_error() {
    crate::test_support::isolated(|home| {
        std::fs::create_dir(home.join("memory.json")).unwrap();
        assert!(Memory::load().is_err());
    });
}
