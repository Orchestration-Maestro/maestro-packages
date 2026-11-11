# Repository check libraries

`maestro-packages-conventions` reads member manifests with `toml =1.1.6` (MIT OR
Apache-2.0). Maestro's tooling and conventions crates declare the same exact pin, and
the lock already held it through `maestro-tooling`, so the dependency adds no new
package version. The crate reads the repository's file list by running the `git`
command and takes no library for it.
