# Hello

The `hello` tool greets the supplied name. Its controlled runs load the built
component into the production engine through the
[test harness](../../docs/extensions/testing.md) and call the registered tool.

## Controlled runs

```sh
just prepare-tests
TEST_PREPARE_OUTPUT="$PWD/target/test-setup" cargo nextest run --locked \
  -p maestro-extension-examples -E 'test(hello_example)'
```
