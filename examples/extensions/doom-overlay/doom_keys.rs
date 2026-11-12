//! Terminal input translated through the host's key decoder.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use gamestate_traits::KeyCode;
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult, matches_key, parse_key};

/// Keys that are named, not typed: decoder name, then the game key.
const NAMED: [(&str, KeyCode); 9] = [
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("right", KeyCode::Right),
    ("left", KeyCode::Left),
    ("space", KeyCode::Space),
    ("enter", KeyCode::Return),
    ("escape", KeyCode::Escape),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
];

/// Raw bytes some terminals send for the keys the decoder also names.
const RAW: [(&str, KeyCode); 8] = [
    ("\x1b[A", KeyCode::Up),
    ("\x1b[B", KeyCode::Down),
    ("\x1b[C", KeyCode::Right),
    ("\x1b[D", KeyCode::Left),
    (" ", KeyCode::Space),
    ("\r", KeyCode::Return),
    ("\x1b", KeyCode::Escape),
    ("\t", KeyCode::Tab),
];

/// Game key code of `A`; `B`..`Z` follow in order.
const FIRST_LETTER_CODE: i32 = 4;
/// Game key code of `Num1`; `Num2`..`Num9` follow in order.
const FIRST_DIGIT_CODE: i32 = 30;
/// Game key code of `Num0`, which follows `Num9`.
const ZERO_CODE: i32 = 39;

/// Typed characters the game reads: letters, digits and the two screen-size keys.
///
/// The game key codes run `A`..`Z`, then `Num1`..`Num9` and `Num0`.
fn character_key(character: char) -> Option<KeyCode> {
    let byte = u8::try_from(character.to_ascii_lowercase()).ok()?;
    let code = match byte {
        b'a'..=b'z' => FIRST_LETTER_CODE + i32::from(byte - b'a'),
        b'1'..=b'9' => FIRST_DIGIT_CODE + i32::from(byte - b'1'),
        b'0' => ZERO_CODE,
        b'-' => return Some(KeyCode::Minus),
        b'+' | b'=' => return Some(KeyCode::Equals),
        _ => return None,
    };
    KeyCode::from_i32(code)
}

/// Maps plain and enhanced terminal keys to game keys; the first is the one that stays held.
/// # Errors
/// Returns a host key-decoder error.
pub fn map_key_to_doom(api: &ExtensionAPI, data: &str) -> ExtensionResult<Vec<KeyCode>> {
    for (raw, key) in RAW {
        if data == raw {
            return Ok(vec![key]);
        }
    }
    for (name, key) in NAMED {
        if matches_key(api, data, name)? {
            return Ok(vec![key]);
        }
    }
    for (letter, key) in [
        ("w", KeyCode::W),
        ("s", KeyCode::S),
        ("a", KeyCode::A),
        ("d", KeyCode::D),
    ] {
        if data == letter || matches_key(api, data, letter)? {
            return Ok(vec![key]);
        }
        if data == letter.to_uppercase() || matches_key(api, data, &format!("shift+{letter}"))? {
            return Ok(vec![key, KeyCode::LShift]);
        }
    }
    if matches!(data, "f" | "F")
        || matches_key(api, data, "f")?
        || matches_key(api, data, "shift+f")?
    {
        // The letter follows the fire key so that cheat codes see it.
        return Ok(vec![KeyCode::RCtrl, KeyCode::F]);
    }
    let named = parse_key(api, data)?;
    if named
        .as_deref()
        .is_some_and(|key| key.starts_with("ctrl+") && key != "ctrl+c")
        || (data.len() == 1 && data.as_bytes()[0] < 32 && data != "\x03")
    {
        return Ok(vec![KeyCode::RCtrl]);
    }
    // An enhanced release arrives as an escape sequence, so the decoder's name stands in for
    // the character.
    let typed = single_character(data)
        .or_else(|| named.as_deref().and_then(single_character))
        .and_then(character_key);
    Ok(typed.into_iter().collect())
}

/// The one character of `text`, if it has exactly one.
fn single_character(text: &str) -> Option<char> {
    let mut characters = text.chars();
    characters.next().filter(|_| characters.next().is_none())
}
