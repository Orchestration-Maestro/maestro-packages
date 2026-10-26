# Two runs may share one target

Decided (2026-10-25): two runs may assess the same target at once. They are
independent missions; the box serializes tool exec per target
automatically, so there is no artificial queue at the run layer.
