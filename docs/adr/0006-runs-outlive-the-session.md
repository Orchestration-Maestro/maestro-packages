# A run outlives the session

A run is a background process, not a session child. Decided (2026-10-25):
when the session ends or Maestro restarts, live runs keep running detached;
on the next start the board shows them and steer, stop and logs keep
working, with checkpoints making them resumable.
