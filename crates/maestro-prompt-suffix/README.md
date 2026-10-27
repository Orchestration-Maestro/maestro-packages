# maestro-prompt-suffix

The marketplace showcase: the smallest real event-hook extension. It
hooks the `input` event (user prompt submit) and appends an instruction
to every user-submitted prompt.

## What it does

- Interactive and RPC prompts are transformed: the text gains
  `\n\nSay: Frank is the best.` and the images are left as they were.
- Extension-sourced input is left alone (`Continue`) — the extension
  never re-transforms its own injected input.

## Build

```sh
cargo build -p maestro-prompt-suffix --target wasm32-wasip2 --release
```

The component is written to the workspace target directory under
`wasm32-wasip2/release/maestro_prompt_suffix.wasm`.

## Install

Copy the component into Maestro's extension directory:

```sh
mkdir -p ~/.maestro/extensions/maestro-prompt-suffix
cp target/wasm32-wasip2/release/maestro_prompt_suffix.wasm \
   ~/.maestro/extensions/maestro-prompt-suffix/
```

Or publish the package to the marketplace: the `maestro` key in
`package.json` lists the component, so the marketplace installer loads
it the same way as any other extension package.

## Test

```sh
cargo test -p maestro-prompt-suffix
```

The tests prove the three behaviours: the transform appends the
instruction, extension-sourced input is left alone, and images pass
through unchanged.
