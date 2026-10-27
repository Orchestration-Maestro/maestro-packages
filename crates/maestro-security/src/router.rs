//! Task routing: map a task hint to the domain that owns it.
//!
//! The domain list is data, not code: a `domains.json` document loaded at
//! run time, with an embedded default so the router works with no file.
//! Adding a domain is a data change, never a recompile.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Schema tag for the domains document.
pub const SCHEMA: &str = "maestro-security/domains/1";

/// One domain the router can dispatch to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    /// Stable id, e.g. `web`.
    pub id: String,
    /// Human name, e.g. `Web applications`.
    pub name: String,
    /// Task hints that route here: words and substrings.
    pub hints: Vec<String>,
    /// The primary capability for the domain.
    pub primary: String,
    /// The attack classes or playbook topics the domain covers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub topics: Vec<String>,
}

/// The domains document: the single source of truth for routing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainsDoc {
    /// Schema tag.
    pub schema: String,
    /// The known domains.
    pub domains: Vec<Domain>,
}

/// The router: a loaded domains document plus its matching rules.
#[derive(Debug, Clone)]
pub struct Router {
    /// The loaded domains document.
    doc: DomainsDoc,
}

impl Router {
    /// The embedded default domains: the core set the pipeline needs with
    /// no configuration file.
    #[must_use]
    pub fn default_() -> Self {
        let mut domains = Self::core_domains();
        domains.extend(Self::extended_domains());
        Self {
            doc: DomainsDoc {
                schema: SCHEMA.into(),
                domains,
            },
        }
    }

    /// The core domains: web, network, and binary.
    fn core_domains() -> Vec<Domain> {
        vec![
            Domain {
                id: "web".into(),
                name: "Web applications".into(),
                hints: vec![
                    "http".into(),
                    "web".into(),
                    "api".into(),
                    "rest".into(),
                    "graphql".into(),
                    "sqli".into(),
                    "xss".into(),
                    "csrf".into(),
                    "endpoint".into(),
                ],
                primary: "surface".into(),
                topics: vec![
                    "owasp-top-10".into(),
                    "injection".into(),
                    "authentication".into(),
                    "authorization".into(),
                ],
            },
            Domain {
                id: "network".into(),
                name: "Network services".into(),
                hints: vec![
                    "port".into(),
                    "tcp".into(),
                    "udp".into(),
                    "scan".into(),
                    "service".into(),
                    "banner".into(),
                ],
                primary: "surface".into(),
                topics: vec!["service-enumeration".into(), "protocol-abuse".into()],
            },
            Domain {
                id: "binary".into(),
                name: "Binary and reverse engineering".into(),
                hints: vec![
                    "binary".into(),
                    "elf".into(),
                    "pe".into(),
                    "disassemble".into(),
                    "decompile".into(),
                    "crackme".into(),
                ],
                primary: "reverse".into(),
                topics: vec!["static-analysis".into(), "dynamic-analysis".into()],
            },
        ]
    }

    /// The extended domains: source, cloud, identity, and ctf.
    fn extended_domains() -> Vec<Domain> {
        vec![
            Domain {
                id: "source".into(),
                name: "Source code hunting".into(),
                hints: vec![
                    "source".into(),
                    "repository".into(),
                    "repo".into(),
                    "code".into(),
                    "sink".into(),
                    "taint".into(),
                ],
                primary: "sourcehunt".into(),
                topics: vec!["sink-tracing".into(), "config-audit".into()],
            },
            Domain {
                id: "cloud".into(),
                name: "Cloud and infrastructure".into(),
                hints: vec![
                    "aws".into(),
                    "azure".into(),
                    "gcp".into(),
                    "kubernetes".into(),
                    "k8s".into(),
                    "terraform".into(),
                    "iac".into(),
                ],
                primary: "audit".into(),
                topics: vec![
                    "iam".into(),
                    "misconfiguration".into(),
                    "supply-chain".into(),
                ],
            },
            Domain {
                id: "identity".into(),
                name: "Identity and access".into(),
                hints: vec![
                    "ldap".into(),
                    "ad".into(),
                    "kerberos".into(),
                    "oauth".into(),
                    "saml".into(),
                    "jwt".into(),
                    "privilege".into(),
                ],
                primary: "exploit".into(),
                topics: vec!["privilege-escalation".into(), "token-abuse".into()],
            },
            Domain {
                id: "ctf".into(),
                name: "Capture the flag".into(),
                hints: vec!["ctf".into(), "flag".into(), "pwn".into(), "crypto".into()],
                primary: "exploit".into(),
                topics: vec!["pwn".into(), "crypto".into(), "forensics".into()],
            },
        ]
    }

    /// Load the domains document from a file; falls back to the embedded
    /// default when the file is missing or unreadable.
    ///
    /// # Errors
    /// Does not fail: a bad file degrades to the embedded default.
    #[must_use]
    pub fn load(path: &Path) -> Self {
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default_();
        };
        match serde_json::from_str(&raw) {
            Ok(doc) => Self { doc },
            Err(_) => Self::default_(),
        }
    }

    /// Route a task hint to the best-matching domain. The match is the
    /// domain with the most hint hits; ties go to the earlier domain.
    #[must_use]
    pub fn route(&self, task: &str) -> Option<&Domain> {
        let task = task.to_lowercase();
        let mut best: Option<(&Domain, usize)> = None;
        for domain in &self.doc.domains {
            let hits = domain
                .hints
                .iter()
                .filter(|h| task.contains(&h.to_lowercase()))
                .count();
            if hits > 0 && best.is_none_or(|(_, score)| hits > score) {
                best = Some((domain, hits));
            }
        }
        best.map(|(d, _)| d)
    }

    /// The loaded domains, in document order.
    #[must_use]
    pub fn domains(&self) -> &[Domain] {
        &self.doc.domains
    }
}

impl From<DomainsDoc> for Router {
    fn from(doc: DomainsDoc) -> Self {
        Self { doc }
    }
}
