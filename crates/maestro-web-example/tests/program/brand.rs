//! Embedded brand input for the example's browser exports.
use maestro_theme::{BrandPack, ThemeError};

/// Decode the example's own copy of the shared brand pack; `path` places the marks and labels
/// errors.
///
/// # Errors
/// Returns the pack's decode failure.
pub fn pack(path: &str) -> Result<BrandPack, ThemeError> {
    BrandPack::parse(
        path,
        include_str!("../../../../examples/web/assets/brand/brand.json"),
    )
}
