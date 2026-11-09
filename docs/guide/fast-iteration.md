# Fast iteration

## Look files reload without restarting

Enable `stillkit/inspect`. Set `STILLKIT_LOOK` to an existing `.toml` or `.json`
look file, then start any `el::run` app. The initial look is parsed before showing
the window. Native Windows directory notifications handle saves and atomic file
replacement; there is no polling timer. A valid save restyles the retained tree
without replacing application state. Invalid or temporarily missing documents
keep the last valid look and emit a `log` warning. Show warnings with your normal
logger. The next valid save recovers automatically. Dropping the runtime cancels
the watcher. There is no watcher unless the environment variable or
`Runtime::watch_theme` explicitly requests one.

```powershell
$env:STILLKIT_LOOK = (Resolve-Path crates/stillkit/looks/sunset.toml)
cargo run -p stillkit-playground --example stories
```

Use the existing [look document format](themes-and-looks.md); no second theme
schema is introduced. Custom hosts can construct `el::inspect::ThemeWatch` with
the UI thread's `stillkit::waker()` and drain `take()` after waking. Errors are
returned rather than applied. Native file watching currently exists on Windows;
other platforms return an explicit unsupported error when watching is requested.
The rest of the playground and inspector remains portable.

## Rust changes use a restart

Install cargo-watch or bacon separately, then run one of these from the workspace:

```sh
cargo watch -w crates/stillkit-playground -w crates/stillkit-kit -x 'run -p stillkit-playground --example ten_minutes'
bacon check
```

cargo-watch restarts the process when Rust files change; bacon keeps diagnostics
visible while you relaunch the example yourself. Neither preserves in-memory Rust
state across a process restart. For apps with a persistent model, save that model
on each accepted edit and load it at startup. Keep stable IDs, version the saved
format, and persist domain values rather than Runtime, element handlers or native
resources. Keep transient focus and animation state in the runtime. Theme-only
edits need no persistence because the process stays alive. This deliberately avoids
a dylib patcher and its ABI and ownership constraints.

The [restart-state example](../../crates/stillkit-playground/examples/restart_state.rs)
implements that loop for a small domain model. It loads an accepted count at
startup and persists each click before accepting it. IO errors are shown in the
view. Use one process for this simple file; production applications should use
their normal durable storage and migration policy.

```powershell
$env:STILLKIT_STATE = 'D:\stillkit-targets\devex\iteration-state.txt'
cargo watch -w crates/stillkit-playground -x 'run -p stillkit-playground --example restart_state'
```
