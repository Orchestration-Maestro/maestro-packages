# Browser chat example

A chat that runs in one browser tab. It uses only the published `maestro-web`
components.

## Quick start

Build the page and serve it on loopback:

```sh
mise exec -- just browser-chat-example --port 9000
```

Open `http://127.0.0.1:9000`. The first message asks for a provider key and keeps
your draft until you supply one.

The recipe runs `just browser-build`, which builds `chat` for
`wasm32-unknown-unknown` and binds it with `wasm-bindgen` into
`maestro-browser/chat` beside `index.html`, under Cargo's target directory
(`CARGO_TARGET_DIR`, or `target`). `just browser-chat-serve` then serves that
directory with `miniserve`. Each extra argument goes to `miniserve` as given. Mise pins
`wasm-bindgen` 0.2.129, the version the lock holds for the library, and
`miniserve` 0.35.0.

## Files

| File | Role |
|---|---|
| `chat.rs` | Starts the example in the page's `app` element and builds the page operands, reading Maestro's brand pack with `BrandPack::shipped()` |
| `chat/app.rs` | Wires the stores, the chat screen, the header and session restore |
| `chat/title.rs` | Derives, shows and edits the session title |
| `chat/custom_messages.rs` | Adds the example's own notification message type |
| `chat/index.html` | The page that loads the module |

`crates/maestro-web-example` builds `chat.rs` as a `cdylib` example and runs the
browser tests. The example reaches Maestro only through the public API of
`maestro-web`, `maestro-app` and `maestro-theme`, at the pinned revision. It
carries no copy of Maestro's brand pack: the pack comes from
`maestro_theme::BrandPack::shipped()`.

## What it shows

- One IndexedDB database, `maestro-chat-example`, holds the settings, provider
  keys, custom providers, sessions and session summaries. It becomes the page's
  application storage.
- A `ChatPanel` shows the session. A provider without a stored key opens the key
  prompt, and a refusal keeps the draft. The session's tools are the artifacts
  tool and a JavaScript tool whose runtime providers come from the chat screen.
- Once a session holds a user message and a response, it gets a title, an
  identifier in the address (`?session=`) and a stored copy. The copy is refreshed
  after every completed message and keeps its first creation time.
- The title is the first sentence when its punctuation follows 1 to 50
  characters. Otherwise, text up to 50 characters stays whole, and longer text
  uses its first 47 characters and an ellipsis.
- Sessions opens the stored list. Choosing one loads it, and deleting the shown
  one starts a new session. New Session clears the address and starts afresh.
- The title is a button that turns into a field with its text selected. Enter or
  leaving the field keeps a new non-blank title, and Escape keeps the old one.
- The bell queues a demonstration notification for the next turn.
  `SystemNotificationMessage` is a message type of the example's own. Its
  renderer shows it as an alert with its local time, and its conversion hands it
  to the model as a user message wrapped in `<system>` tags.
- The header's appearance control stands in for the chat's, which the chat screen
  turns off. Settings opens the provider and proxy tabs.
- An address that names a stored session restores it. An unknown session starts a
  new one and clears the address.

## Behaviour notes

The page never reloads. New Session and a missing session replace the address and
start a new session in place. Replacing the shown session aborts its active
request. Message completion keeps an open title draft, and a pending title save
updates only the session that started it. A restored session keeps the example's
instructions, and a stored summary sums the assistant usage. The example does not
ask for persistent storage.

## Browser tests

Seven exports in `crates/maestro-web-example/tests/program` compile the example's
own modules into one WebAssembly program. They drive the example through its
controls, with a recorded fetch and real browser storage. The native test
`browser::every_program_export_has_a_browser_test` fails when an export has no
browser test, or a browser test names no export.

Run them in the supplied Chromium:

```sh
CHROMIUM_PATH=/path/to/chrome mise exec -- just browser-test
```

Nextest builds the program and binds it once, then runs each export in its own
browser and a fresh page. The exports run as `browser::chat_example_*`. A run that
selects no export fails.
