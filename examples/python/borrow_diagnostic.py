"""Diagnostic for componentize-py async borrow cleanup, not an extension example."""

import inspect
from abc import update_abstractmethods

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


@reject_unregistered
class Guest(exports.Guest):
    """Register one observer to isolate async borrowed-resource cleanup."""

    def __init__(self):
        """Retain the registered callback owner independently of invocation borrows."""
        self.callback: host.Callback | None = None

    async def start(self):
        """Register an input observer; no tool or event edits are involved."""
        self.callback = host.Callback()
        host.on("input", self.callback)

    def release(self, handler):
        """Drop the sole callback owner when the host releases it."""
        self.callback = None

    async def invoke_event(self, handler, event, resources):
        """Return no edits or decision, retaining no invocation operands."""
        return host.EventOutcome(Ok(None), host.Decision_Returned(Ok(None)), None)
