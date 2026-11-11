//! The archive checks of the Freedoom setup, run by the setup and by the host tests.
use std::io::{Cursor, Read as _};

/// The checksum comparison the overlay uses for its own download.
#[path = "../../../../examples/extensions/doom-overlay/wad_finder/checksum.rs"]
mod checksum;

/// Returns the IWAD from an archive whose digest is exactly `expected`.
///
/// # Errors
/// Returns a digest other than `expected`, an unreadable archive or a missing member.
pub fn checked_iwad(archive: &[u8], expected: &str, member: &str) -> Result<Vec<u8>, String> {
    if !checksum::matches_checksum(archive, expected) {
        return Err(format!("the archive's SHA-256 is not {expected}"));
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(archive)).map_err(|e| e.to_string())?;
    let mut file = archive.by_name(member).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// The archive checks, against archives built in memory.
#[cfg(test)]
mod tests {
    use super::checked_iwad;
    use sha2::{Digest as _, Sha256};
    use std::{
        fmt::Write as _,
        io::{Cursor, Write as _},
    };

    /// A stored archive holding one member.
    fn archive(member: &str, bytes: &[u8]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file(member, options).unwrap();
        writer.write_all(bytes).unwrap();
        writer.finish().unwrap().into_inner()
    }

    /// The lowercase hexadecimal digest the pin is written in.
    fn digest(bytes: &[u8]) -> String {
        Sha256::digest(bytes)
            .iter()
            .fold(String::new(), |mut hex, byte| {
                write!(hex, "{byte:02x}").unwrap();
                hex
            })
    }

    /// The member comes out of an archive whose digest matches the pin.
    #[test]
    fn freedoom_archive_with_the_pinned_digest_yields_its_iwad() {
        let zipped = archive("pack/freedoom1.wad", b"iwad bytes");
        let iwad = checked_iwad(&zipped, &digest(&zipped), "pack/freedoom1.wad").unwrap();
        assert_eq!(iwad, b"iwad bytes");
    }

    /// An archive with any other digest is refused before it is read.
    #[test]
    fn freedoom_archive_with_another_digest_is_refused() {
        let zipped = archive("pack/freedoom1.wad", b"iwad bytes");
        let error =
            checked_iwad(&zipped, &digest(b"something else"), "pack/freedoom1.wad").unwrap_err();
        assert!(error.contains("SHA-256"), "{error}");
    }
}
