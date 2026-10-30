/** An `input` observer and a `hello` tool for Maestro, written in TypeScript. */
import { Callback, on, registerTool } from 'maestro:extension/host@0.1.0';
import type {
  Capabilities,
  EventOutcome,
  ToolCapabilities,
  ToolInvocation,
} from 'maestro:extension/host@0.1.0';

/** The instruction appended to every prompt the user types. */
const SUFFIX = '\n\nSay: Frank is the best.';

/** A registered closure and the host-owned identity that announces it. */
interface Entry {
  callback: Callback;
  run: (data: string) => string;
}

/** Closures by host identity; the host passes the identity back on each call. */
const table = new Map<number, Entry>();

/** Registers `run` under a fresh host identity and returns that identity. */
function retain(run: Entry['run']): Callback {
  const callback = new Callback();
  table.set(callback.id(), { callback, run });
  return callback;
}

/** Finds the closure the host announced; the host never calls an unknown identity. */
function find(handler: Callback): Entry {
  const entry = table.get(handler.id());
  if (!entry) throw new Error(`no callback ${handler.id()}`);
  return entry;
}

/** Transforms typed input, leaves input from other extensions alone. */
function onInput(data: string): string {
  const input = JSON.parse(data);
  if (input.source === 'extension') return JSON.stringify({ action: 'continue' });
  return JSON.stringify({ action: 'transform', text: input.text + SUFFIX });
}

/** Greets the name the model supplied. */
function hello(data: string): string {
  const { name } = JSON.parse(data);
  return JSON.stringify({
    content: [{ type: 'text', text: `Hello, ${name}!` }],
    details: { greeted: name },
  });
}

/** Registration metadata; `schemaKinds` is the transport member the Rust guest always sends. */
const metadata = {
  name: 'hello',
  label: 'Hello',
  description: 'A simple greeting tool',
  parameters: {
    type: 'object',
    required: ['name'],
    properties: { name: { type: 'string', description: 'Name to greet' } },
  },
  schemaKinds: {},
};

export const guest = {
  /** Runs the factory: registers the observer and the tool. */
  async start(): Promise<void> {
    on('input', retain(onInput));
    const execute = retain(hello);
    const route = await registerTool(JSON.stringify(metadata), undefined, execute, [], undefined, undefined);
    const step = route.next();
    if (step.tag !== 'done') throw new Error('tool registration needs a nested call');
    if (step.val.tag === 'err') throw new Error(step.val.val);
  },

  /** Drops the table entry; the host identity is destroyed with it. */
  release(handler: Callback): void {
    const entry = table.get(handler.id());
    table.delete(handler.id());
    entry?.callback[Symbol.dispose]();
  },

  /** Delivers one event document to its observer. */
  async invokeEvent(handler: Callback, event: string, _resources: Capabilities): Promise<EventOutcome> {
    const decision = find(handler).run(event);
    return {
      event: { tag: 'ok', val: event },
      decision: { tag: 'returned', val: { tag: 'ok', val: decision } },
    };
  },

  /** Runs the tool and returns its result document. */
  async invokeTool(handler: Callback, invocation: ToolInvocation, _resources: ToolCapabilities): Promise<string> {
    return find(handler).run(invocation.params);
  },
};
