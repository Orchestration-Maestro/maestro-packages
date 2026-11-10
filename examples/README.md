# Examples

Every example here is Rust. Each folder below has its own index page.

| Path | Purpose |
| --- | --- |
| [extensions/](extensions/) | Extensions that register tools, commands, event handlers and interface components, built as WebAssembly components. |
| [sdk/](sdk/) | In-process SDK samples and the interactive RPC client demo. |
| [rpc_extension_ui.rs](rpc_extension_ui.rs) | A terminal RPC client with streaming output and extension dialogs. |
| [web/](web/) | A chat built only from the published web components, for the browser. |

The `maestro-sdk-examples` crate builds the SDK samples and the terminal client and runs their checks.

## Maestro's own extension examples

Maestro's repository carries three more extension examples, each shown here by its tool, hook or message:

- [`hello`](https://github.com/Orchestration-Maestro/maestro/blob/5ac4b8926090b5fcf41e33433b8456720bd3569e/examples/extensions/hello.rs) registers a greeting tool.
- [`input_transform`](https://github.com/Orchestration-Maestro/maestro/blob/5ac4b8926090b5fcf41e33433b8456720bd3569e/examples/extensions/input_transform.rs) rewrites typed input and passes input from other extensions through unchanged.
- [`send_user_message`](https://github.com/Orchestration-Maestro/maestro/blob/5ac4b8926090b5fcf41e33433b8456720bd3569e/examples/extensions/send_user_message.rs) sends ordinary, steering and follow-up messages.

## Browser example

- [Browser chat](web/README.md): a complete chat built only from the published web
  components. `just browser-chat-example` serves it, and `just browser-test` runs its
  seven browser tests in Chromium.
