/** A `probe` tool that queues a user message as another extension would, for a host test. */
import { Callback, registerTool, sendUserMessage } from 'maestro:extension/host@0.1.0';
import type { ToolCapabilities, ToolInvocation } from 'maestro:extension/host@0.1.0';

/** The message the tool queues; the host test expects the model to receive exactly this. */
const SENT = 'Say: the probe sent this.';

/** Registration metadata; the tool takes no arguments. */
const metadata = {
  name: 'probe',
  label: 'Probe',
  description: 'Queues a user message',
  parameters: { type: 'object', properties: {} },
  schemaKinds: {},
};

/** The tool's host-owned identity, kept so the host's handle stays alive. */
let execute: Callback | undefined;

export const guest = {
  /** Runs the factory: registers the tool. */
  async start(): Promise<void> {
    execute = new Callback();
    const route = await registerTool(JSON.stringify(metadata), undefined, execute, [], undefined, undefined);
    const step = route.next();
    if (step.tag !== 'done') throw new Error('tool registration needs a nested call');
    if (step.val.tag === 'err') throw new Error(step.val.val);
  },

  /** Nothing to drop: the only handle lives as long as the extension. */
  release(_handler: Callback): void {},

  /** Queues the message for after the current turn; the host takes the content as JSON text. */
  async invokeTool(_handler: Callback, _invocation: ToolInvocation, _resources: ToolCapabilities): Promise<string> {
    const step = sendUserMessage(JSON.stringify(SENT), JSON.stringify({ deliverAs: 'followUp' })).next();
    if (step.tag !== 'done') throw new Error('send needs a nested call');
    if (step.val.tag === 'err') throw new Error(step.val.val);
    return JSON.stringify({ content: [{ type: 'text', text: 'queued' }] });
  },
};
