# DOOM overlay

`/doom-overlay` plays DOOM in a terminal overlay. The engine is room4doom, a
Rust engine compiled into the extension, so the example is one component with
nothing to build beside it.

## Quick start

Build the component and load it through the normal
[extension loading interface](../../docs/extensions/loading.md):

```sh
cargo build --locked --release -p maestro-extension-examples \
  --target wasm32-wasip2 --example doom_overlay
```

The component is `target/wasm32-wasip2/release/examples/doom_overlay.wasm`. In
an interactive terminal with true colour, run `/doom-overlay`. The overlay
opens on the title screen; Escape opens the menu and the menu starts a game. Q
pauses the game and closes the overlay, and `/doom-overlay` resumes it.

The overlay needs a component-enabled host with true-colour half-block output,
enhanced keyboard releases and native background waits. A headless run reports
that interactive mode is required.

## WAD files

DOOM needs an IWAD, the game's data file. An explicit path wins:
`/doom-overlay ~/games/doom1.wad`. Without one, the overlay resumes the game
already open in this session. With none open, it checks the component's
directory, `./doom1.wad`, `./DOOM1.WAD`, `~/doom1.wad` and
`~/.doom/doom1.wad`, in that order. If no file exists, it downloads the
shareware WAD beside the component.

The download uses the [PureDOOM mirror pinned at commit
355cfbd16fac119718879239336ee2ea408886bd](https://raw.githubusercontent.com/Daivuk/PureDOOM/355cfbd16fac119718879239336ee2ea408886bd/doom1.wad).
It must match the official shareware v1.9 file:

- Size: 4,196,020 bytes.
- SHA-256: `1d7d43be501e67d927e415e0b8f3e29c3bf33075e859721816f652a526cac771`.
- MD5 identification: `f0cefca49926d00903cf57551d901abe`.

The overlay rejects a checksum mismatch before it creates the cache file. A
network, body or filesystem failure produces a notification and opens no game.
No game data ships with Maestro.

Other IWADs are not held to the shareware checksum. Freedoom Phase 1, a free
IWAD, plays too. The overlay checks that the file is an IWAD whose directory
lies inside the file and that it holds the lumps a game start reads. A file that
fails the check produces `Failed to load DOOM: <reason>` and starts nothing.

## Controls and lifetime

Arrow keys turn and move. WASD moves and strafes, Shift+WASD runs, Space uses,
F fires and 1 to 7 select weapons. Enter, Escape, the arrow keys and the F keys
operate the menus. Q pauses the game and closes the overlay.

The overlay keeps one game per session. `/doom-overlay` resumes it. A different
WAD path replaces it, and choosing Quit Game in the menu ends it and closes the
view; the next `/doom-overlay` starts a new game. Closing the view cancels its
wait and releases held keys.

The game draws a 640×400 frame at 35 Hz, sampled as two pixels per terminal
cell. It has no audio. The automap (Tab) is not available yet.

The IWAD copy, settings and saves live in one private temporary root. Saves are
not kept after the session: shutdown or a replaced game removes the root. A WAD
the engine cannot play is refused before it starts, because an engine panic would
stop the extension rather than show a notice.

## Licence

room4doom is MIT-licensed and carries DOOM-derived code; its notice and its
stated sources are in [THIRD-PARTY-NOTICES.md](../../THIRD-PARTY-NOTICES.md).

## Controlled runs

```sh
just prepare-tests
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked \
  -p maestro-extension-examples -E 'test(/^host::doom/)'
```

The `host::doom_wad::` tests use synthetic bytes: they prove the search order,
the download checks and the refusal of a file that is not an IWAD, and they need
no game data. The `host::doom_game::` tests play Freedoom Phase 1 0.13.0. Their
`freedoom` setup downloads the release archive, checks its pinned SHA-256,
extracts `freedoom1.wad` under `target/freedoom` and exports
`MAESTRO_FREEDOOM_WAD`. A missing variable fails those tests. Nothing from the
archive is committed.
