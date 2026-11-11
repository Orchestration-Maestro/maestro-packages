//! Refuses files the game would stop on, before the game reads them.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use std::collections::BTreeSet;

/// One directory entry is a 4-byte offset, a 4-byte size and an 8-byte name.
const ENTRY_BYTES: usize = 16;
/// The lumps every game start reads.
const REQUIRED: [&str; 9] = [
    "PLAYPAL", "COLORMAP", "TEXTURE1", "PNAMES", "TITLEPIC", "STBAR", "STFB1", "WIPCNT", "FLOOR4_8",
];

/// Reads a little-endian number from four bytes at `at`.
fn number(bytes: &[u8], at: usize) -> Result<usize, String> {
    let field = bytes
        .get(at..at + 4)
        .and_then(|field| <[u8; 4]>::try_from(field).ok())
        .ok_or("The WAD directory is cut short")?;
    usize::try_from(u32::from_le_bytes(field)).map_err(|error| error.to_string())
}

/// Collects the lump names of a directory whose entries lie inside the file.
fn lump_names(bytes: &[u8]) -> Result<BTreeSet<String>, String> {
    let count = number(bytes, 4)?;
    let table = number(bytes, 8)?;
    let end = count
        .checked_mul(ENTRY_BYTES)
        .and_then(|length| length.checked_add(table))
        .filter(|end| *end <= bytes.len())
        .ok_or("The WAD directory lies outside the file")?;
    let mut names = BTreeSet::new();
    for entry in bytes[table..end].as_chunks::<ENTRY_BYTES>().0 {
        let (offset, size) = (number(entry, 0)?, number(entry, 4)?);
        if offset
            .checked_add(size)
            .is_none_or(|lump_end| lump_end > bytes.len())
        {
            return Err("A WAD lump lies outside the file".into());
        }
        let name = entry[8..]
            .split(|byte| *byte == 0)
            .next()
            .unwrap_or_default();
        names.insert(String::from_utf8_lossy(name).to_ascii_uppercase());
    }
    Ok(names)
}

/// Accepts an IWAD with a sound directory and the lumps a game start reads.
/// # Errors
/// Names the first problem: a wrong type, a damaged directory or a missing lump.
pub fn check_iwad(bytes: &[u8]) -> Result<(), String> {
    if bytes.get(..4) != Some(b"IWAD") {
        return Err("The file is not an IWAD".into());
    }
    let names = lump_names(bytes)?;
    if let Some(missing) = REQUIRED.iter().find(|name| !names.contains(**name)) {
        return Err(format!("The IWAD has no {missing} lump"));
    }
    if !(names.contains("E1M1") || names.contains("MAP01")) {
        return Err("The IWAD has no first map".into());
    }
    Ok(())
}
