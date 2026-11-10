//! The example reads Maestro's brand pack through the public accessor and carries no copy.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Collect every file named `name` under `directory`.
fn find(directory: &Path, name: &str, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            find(&path, name, found)?;
        } else if path.file_name().is_some_and(|file| file == name) {
            found.push(path);
        }
    }
    Ok(())
}

/// Neither the example nor its browser tests hold a `brand.json` of their own.
#[test]
fn example_and_browser_tests_carry_no_brand_pack_copy() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for root in [crate_root.join("../../examples"), crate_root.join("tests")] {
        find(&root, "brand.json", &mut found).unwrap();
    }
    assert_eq!(found, Vec::<PathBuf>::new());
}
