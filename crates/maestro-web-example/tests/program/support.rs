//! Controlled language effects and message records for the example's browser exports.

use maestro_web::{I18nDiagnostic, I18nError, LanguageHost};
use std::cell::RefCell;

/// Host state with an ordered effect record.
#[derive(Default)]
pub struct Host {
    /// Stored selection.
    pub stored: RefCell<Option<String>>,
    /// Browser locale.
    pub locale: RefCell<Option<String>>,
    /// Legacy locale.
    pub legacy: RefCell<Option<String>>,
    /// Effect that must fail.
    pub fail: RefCell<String>,
    /// Observed effects.
    pub effects: RefCell<Vec<String>>,
    /// Missing-entry notifications.
    pub diagnostics: RefCell<Vec<I18nDiagnostic>>,
}
impl Host {
    /// Observe an effect before deciding whether it fails.
    fn effect(&self, name: &str) -> Result<(), I18nError> {
        self.effects.borrow_mut().push(name.to_owned());
        if *self.fail.borrow() == name {
            Err(I18nError::Host(format!("{name} failed")))
        } else {
            Ok(())
        }
    }
}
impl LanguageHost for Host {
    fn stored_language(&self) -> Result<Option<String>, I18nError> {
        self.effect("get")?;
        Ok(self.stored.borrow().clone())
    }
    fn browser_language(&self) -> Result<Option<String>, I18nError> {
        self.effect("locale")?;
        Ok(self.locale.borrow().clone())
    }
    fn legacy_language(&self) -> Result<Option<String>, I18nError> {
        self.effect("legacy")?;
        Ok(self.legacy.borrow().clone())
    }
    fn store_language(&self, code: &str) -> Result<(), I18nError> {
        self.effect("set")?;
        *self.stored.borrow_mut() = Some(code.to_owned());
        Ok(())
    }
    fn reload(&self) -> Result<(), I18nError> {
        self.effect("reload")
    }
    fn unknown_translation(&self, diagnostic: &I18nDiagnostic) {
        self.diagnostics.borrow_mut().push(diagnostic.clone());
    }
}

/// Controlled message records decoded into the caller's browser record.
pub mod agent_records {
    /// A user entry saying `text`, decoded into the caller's record.
    ///
    /// # Panics
    /// Panics if the record is malformed.
    #[must_use]
    pub fn user_entry<T: serde::de::DeserializeOwned>(text: &str) -> T {
        serde_json::from_value(serde_json::json!({"role": "user", "content": text, "timestamp": 1}))
            .unwrap()
    }
}
