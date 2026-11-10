# Examples

Every example here is Rust.

## Extensions

Maestro's own extension examples show the `hello` tool, the `input` hook and user
messages sent from an extension:

- [`hello`](https://github.com/Orchestration-Maestro/maestro/blob/5ac4b8926090b5fcf41e33433b8456720bd3569e/examples/extensions/hello.rs) registers a greeting tool.
- [`input_transform`](https://github.com/Orchestration-Maestro/maestro/blob/5ac4b8926090b5fcf41e33433b8456720bd3569e/examples/extensions/input_transform.rs) rewrites typed input and passes input from other extensions through unchanged.
- [`send_user_message`](https://github.com/Orchestration-Maestro/maestro/blob/5ac4b8926090b5fcf41e33433b8456720bd3569e/examples/extensions/send_user_message.rs) sends ordinary, steering and follow-up messages.

## Browser example

- [Browser chat](web/README.md): a complete chat built only from the published web
  components. `just browser-chat-example` serves it, and `just browser-test` runs its
  seven browser tests in Chromium.
