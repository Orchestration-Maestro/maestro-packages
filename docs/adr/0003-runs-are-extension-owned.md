# Runs are extension-owned; the native-hosting gap is deferred

A run would ideally be a first-class subagent in Maestro's runtime, but the
extension contract exposes no subagent API. Decided (2026-10-25): build the
run as an extension-owned async task whose UX mirrors a subagent (live
entry, steering, kill, events), and file the maestro core gap ticket for
native run hosting later, once maestro itself is fully set up.
