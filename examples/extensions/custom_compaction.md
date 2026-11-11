# Custom compaction

Build with `--example custom_compaction` using the [build command](README.md#building-an-example). Load its component only
when you want custom summaries: it selects `google/gemini-2.5-flash` independently
of the conversation model and needs credentials for that provider. Real model
calls can incur charges and send conversation contents to the provider; the
controlled run below uses no network or live credentials.

The hook combines the prepared summary messages and split-turn prefix, includes
any previous summary, and requests at most 8,192 output tokens. It preserves the
host's supplied `firstKeptEntryId` and token count; recent entries are not deleted
by the extension. Missing models or authentication, empty output, cancellation
and provider failures leave the host's default compaction path available. The
host remains responsible for cancellation and accepting or storing the result.
Notifications use the current UI host and are harmless in headless mode.

## Controlled runs

```sh
just prepare-tests
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked \
  -p maestro-extension-examples -E 'test(custom_compaction)'
```
