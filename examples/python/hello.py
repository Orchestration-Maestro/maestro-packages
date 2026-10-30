"""Python extension: an `input` hook that upper-cases text and the `hello` greeting tool."""

import inspect
import json
from abc import update_abstractmethods
from collections.abc import Callable

from componentize_py_types import Err, Ok
from wit_world import exports
from wit_world.imports import host

# These WIT exports carry diagnostic-error-info instead of string errors.
DIAGNOSTIC_EXPORTS = {
    "invoke_schema",
    "describe_schema",
    "retain_schema",
    "invoke_read",
    "invoke_write",
    "invoke_grep",
    "invoke_ls",
    "invoke_find",
    "invoke_edit",
}

HELLO_TOOL = {
    "name": "hello",
    "label": "Hello",
    "description": "A simple greeting tool",
    "parameters": {
        "type": "object",
        "required": ["name"],
        "properties": {"name": {"type": "string", "description": "Name to greet"}},
    },
}


def rejecting_export(name, asynchronous):
    """Reject an unregistered surface using its declared WIT error payload."""

    def reject(*args, **kwargs):
        """Fail without consuming any callback operands."""
        message = "Unregistered callback surface: " + name
        if name in DIAGNOSTIC_EXPORTS:
            raise Err(host.DiagnosticErrorInfo(None, message, None, None))
        raise Err(message)

    async def reject_async(*args, **kwargs):
        """Return the same failure through an async export."""
        return reject(*args, **kwargs)

    return reject_async if asynchronous else reject


def reject_unregistered(cls):
    """Complete the world without replacing generated async dispatch wrappers."""
    for name in cls.__abstractmethods__:
        setattr(
            cls,
            name,
            rejecting_export(name, inspect.iscoroutinefunction(getattr(cls, name))),
        )
    return update_abstractmethods(cls)


def upper_case_input(event):
    """Answer an `input` event with its text upper-cased, leaving other events alone."""
    if event["type"] == "input":
        return {"action": "transform", "text": event["text"].upper()}
    return None


def greet(params):
    """Return the tool result greeting the requested name, byte for byte."""
    name = params["name"]
    return {
        "content": [{"type": "text", "text": f"Hello, {name}!"}],
        "details": {"greeted": name},
    }


# componentize-py 0.25.1 builds one `Guest` object per exported function, so the table
# of callbacks lives at module level where every export sees it.
CALLBACKS: dict[int, tuple[host.Callback, Callable]] = {}


def keep(function):
    """Create a host identity for `function`, keep it until released, and return it."""
    callback = host.Callback()
    CALLBACKS[callback.id()] = (callback, function)
    return callback


def function_for(handler):
    """Find the function a borrowed callback identity stands for."""
    key = handler.id()
    if key not in CALLBACKS:
        raise Err(f"Unknown callback {key}")
    return CALLBACKS[key][1]


@reject_unregistered
class Guest(exports.Guest):
    """Register the hook and the tool, then serve the callbacks the host invokes."""

    async def start(self):
        """Register the `input` hook and the `hello` tool."""
        host.on("input", keep(upper_case_input))
        route = await host.register_tool(
            json.dumps(HELLO_TOOL), None, keep(greet), [], None, None
        )
        step = route.next()
        if not isinstance(step, host.RouteStep_Done):
            raise Err("Tool registration made a nested call this example does not serve")
        if not isinstance(step.value, Ok):
            raise step.value

    def release(self, handler):
        """Drop the sole callback owner when the host releases it."""
        del CALLBACKS[handler.id()]

    async def invoke_event(self, handler, event, resources):
        """Run the hook on the event document and return its result document."""
        result = function_for(handler)(json.loads(event))
        encoded = None if result is None else json.dumps(result)
        return host.EventOutcome(Ok(None), host.Decision_Returned(Ok(encoded)), None)

    async def invoke_tool(self, handler, invocation, resources):
        """Run the tool on its parameter document and return its result document."""
        return json.dumps(function_for(handler)(json.loads(invocation.params)))
