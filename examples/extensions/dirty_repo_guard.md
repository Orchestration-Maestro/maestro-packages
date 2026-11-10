# Dirty repo guard

Build with `--example dirty_repo_guard` using the [build command](README.md#building-an-example).
Loading is opt-in and needs a component-enabled native host with Git on `PATH`.
The guard only reads repository state: it runs `git status --porcelain` in the
host's working directory and never commits, stashes or resets anything.

Before a new session, a session switch or a fork, the guard counts the nonempty
lines of that status output. When the status run fails, for example outside a
repository or without Git, or the tree is clean, the change goes ahead without a
question. With uncommitted files and no UI, the change is cancelled. With UI, a
selector asks **Start a new session with N uncommitted file(s)?**, or names
**Switch sessions** or **Fork** instead, and offers **Continue** and **Cancel
and commit first**. Only **Continue** lets the change go ahead. Any other
answer, or dismissing the selector, shows the warning `Cancelled. Commit the
changes, then try again.` and cancels the change.

The controlled `dirty_repo_guard_` runs deliver every case to the in-process
adapter and to the built component with scripted status results and selector
answers. They run no Git command and need no provider, credentials or
installation.

## Controlled runs

The controlled runs for this example are not hosted here yet.
[#75](https://github.com/Orchestration-Maestro/maestro-packages/issues/75) adds
them.
