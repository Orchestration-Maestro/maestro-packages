//! WAD discovery and the controlled download through the shipped command.
#![cfg(test)]
use maestro_extensions::{
    EventBusController, ExtensionRunner, ExtensionUIContext, Local, create_event_bus_for,
    create_extension_runtime, load_extension_from_factory,
};
use maestro_test_directory::Scratch;
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::RwLock,
};
mod io;
mod ui;

pub(super) use ui::Scenario;

/// The pinned download address the example reads when no local WAD exists.
const WAD_URL: &str = "https://raw.githubusercontent.com/Daivuk/PureDOOM/355cfbd16fac119718879239336ee2ea408886bd/doom1.wad";

/// One loaded instance and its caller-owned effects.
pub(super) struct Demo {
    /// Command and lifecycle dispatcher.
    pub runner: ExtensionRunner<Local>,
    /// Keeps the background channel alive.
    _bus: EventBusController<Local>,
    /// Actual announced background jobs.
    tails: Rc<RefCell<Vec<tokio::task::JoinHandle<()>>>>,
    /// Notices and the views the overlay opens.
    pub ui: Rc<ui::Ui>,
    /// Controlled HTTP request owner.
    io: Rc<io::Io>,
}

/// Loads from a controlled module directory with real filesystem access; a headless run gets no UI.
pub(super) async fn loaded(root: &Path, io: Rc<io::Io>, headless: bool) -> Demo {
    let (session, _history) =
        super::session::runner_session(root.join("project").to_str().unwrap());
    let tails = Rc::new(RefCell::new(Vec::new()));
    let started = tails.clone();
    let bus = create_event_bus_for::<Local>(
        Rc::new(move |tail| {
            started.borrow_mut().push(tokio::task::spawn_local(tail));
        }),
        Rc::new(|error| panic!("unexpected background error: {error:?}")),
    );
    let runner = runner(root, &bus, io.clone(), &session).await;
    let (tui, _terminal) = super::canonical_ui::screen();
    let ui = Rc::new(ui::Ui {
        tui,
        notices: RefCell::default(),
        scenarios: RefCell::default(),
        stopped: Rc::default(),
        until_stopped: std::cell::Cell::default(),
    });
    if !headless {
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    }
    Demo {
        runner,
        _bus: bus,
        tails,
        ui,
        io,
    }
}

impl Demo {
    /// Runs `/doom-overlay` with the arguments, to the end of the command.
    pub async fn command(&self, args: &str) {
        (self
            .runner
            .get_command("doom-overlay")
            .unwrap()
            .command
            .handler)(args.into(), self.runner.create_command_context())
        .await
        .unwrap();
    }

    /// Queues what the next opened view does before it closes.
    pub fn view(&self, scenario: Scenario) {
        self.ui.scenarios.borrow_mut().push_back(scenario);
    }

    /// Every notice so far.
    pub fn notices(&self) -> Vec<String> {
        self.ui.notices.borrow().clone()
    }

    /// Ends the session and waits for the overlay's background work to finish.
    pub async fn shutdown(&self) {
        let mut shutdown: maestro_extensions::types::SessionShutdownEvent =
            serde_json::from_value(serde_json::json!({"reason":"quit"})).unwrap();
        self.runner.emit(&mut shutdown).await.unwrap();
        for tail in self.tails.take() {
            tail.await.unwrap();
        }
    }
}

/// Native candidate locations in their documented order.
fn candidates(root: &Path) -> [PathBuf; 5] {
    [
        root.join("module/doom1.wad"),
        root.join("project/doom1.wad"),
        root.join("project/DOOM1.WAD"),
        root.join("home/doom1.wad"),
        root.join("home/.doom/doom1.wad"),
    ]
}

/// Seeds lower-priority choices too, so the announced path proves ordering.
fn prepare(root: &Path, case: &str) -> (String, Option<PathBuf>) {
    if let Ok(first) = case.parse::<usize>() {
        let all = candidates(root);
        for (index, path) in all.iter().enumerate().skip(first) {
            if !path.exists() {
                std::fs::write(path, format!("candidate {index}")).unwrap();
            }
        }
        return (String::new(), Some(all[first].clone()));
    }
    match case {
        "explicit" => {
            let path = root.join("custom.wad");
            std::fs::write(&path, b"custom WAD").unwrap();
            (path.to_str().unwrap().into(), Some(path))
        }
        "missing" => ("missing.wad".into(), None),
        _ => (String::new(), None),
    }
}

/// Checks the announced WAD or a visible failure, and that no game opened from these bytes.
fn observed(demo: &Demo, root: &Path, case: &str, selected: Option<&Path>) {
    let notices = demo.notices();
    if let Some(selected) = selected {
        let announced = format!("Loading DOOM from {}...", selected.to_str().unwrap());
        assert!(notices.contains(&announced), "{notices:?}");
        assert!(
            notices
                .iter()
                .any(|message| message.starts_with("Failed to load DOOM:")),
            "synthetic bytes are not an IWAD: {notices:?}"
        );
    }
    if ["mismatch", "http-error", "stream-error"].contains(&case) {
        assert_eq!(*demo.io.requests.borrow(), [WAD_URL]);
        assert!(!root.join("module/doom1.wad").exists());
    } else {
        assert!(demo.io.requests.borrow().is_empty());
    }
    if ["missing", "mismatch", "http-error", "stream-error"].contains(&case) {
        assert!(
            notices
                .iter()
                .any(|message| message.starts_with("Failed to")),
            "{notices:?}"
        );
    }
}

/// An IWAD the overlay must refuse and the reason it gives.
fn refused(case: &str) -> Option<(Vec<u8>, &'static str)> {
    let header = |id: &[u8], lumps: u32, table: u32| {
        let mut bytes = id.to_vec();
        bytes.extend(lumps.to_le_bytes());
        bytes.extend(table.to_le_bytes());
        bytes
    };
    match case {
        "not-iwad" => Some((header(b"PWAD", 0, 12), "The file is not an IWAD")),
        "no-lumps" => Some((header(b"IWAD", 0, 12), "The IWAD has no PLAYPAL lump")),
        "bad-directory" => Some((
            header(b"IWAD", 1, 4000),
            "The WAD directory lies outside the file",
        )),
        _ => None,
    }
}

/// A damaged or unsuitable WAD is refused with its reason, and leaves no files behind.
async fn run_refused(root: &Path, bytes: &[u8], reason: &str) {
    let path = root.join("refused.wad");
    std::fs::write(&path, bytes).unwrap();
    let demo = loaded(root, controlled_io("none"), false).await;
    demo.command(path.to_str().unwrap()).await;
    let notices = demo.notices();
    assert!(
        notices.contains(&format!("Failed to load DOOM: {reason}")),
        "{notices:?}"
    );
    demo.shutdown().await;
    assert!(
        std::fs::read_dir(root.join("temporary"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// Runs one environment-isolated scenario without changing the parent's HOME.
async fn run() {
    let root = PathBuf::from(std::env::var("MAESTRO_DOOM_WAD_ROOT").unwrap());
    let case = std::env::var("MAESTRO_DOOM_WAD_CASE").unwrap();
    if super::doom_game::run(&root, &case).await {
        return;
    }
    if let Some((bytes, reason)) = refused(&case) {
        run_refused(&root, &bytes, reason).await;
        return;
    }
    let (args, selected) = prepare(&root, &case);
    let demo = loaded(&root, controlled_io(&case), case == "headless").await;
    demo.command(&args).await;
    observed(&demo, &root, &case, selected.as_deref());
    demo.shutdown().await;
    assert!(
        std::fs::read_dir(root.join("temporary"))
            .unwrap()
            .next()
            .is_none()
    );
}

/// Builds once, then executes each scenario in an exclusive disposable root.
pub(super) fn checked(name: &str, cases: &[&str]) {
    if maestro_test_process::selection().is_child() {
        super::example_process::run(run());
        return;
    }
    let component = super::support::component("MAESTRO_DOOM_OVERLAY_COMPONENT").unwrap();
    for case in cases {
        let scratch = Scratch::new().unwrap();
        for name in ["module", "project", "home/.doom", "temporary"] {
            std::fs::create_dir_all(scratch.path().join(name)).unwrap();
        }
        std::fs::copy(&component, scratch.path().join("module/doom_overlay.wasm")).unwrap();
        let status = maestro_test_process::TestProcess::current(name)
            .unwrap()
            .case(case)
            .env("MAESTRO_DOOM_WAD_ROOT", scratch.path())
            .env("MAESTRO_DOOM_WAD_CASE", case)
            .env("HOME", scratch.path().join("home"))
            .env("USERPROFILE", scratch.path().join("home"))
            .env("TMPDIR", scratch.path().join("temporary"))
            .env("TMP", scratch.path().join("temporary"))
            .env("TEMP", scratch.path().join("temporary"))
            .current_dir(scratch.path().join("project"))
            .nocapture()
            .status()
            .unwrap();
        assert!(status.success(), "scenario {case}");
    }
}

/// Module candidates take precedence over every later location.
#[test]
fn doom_wad_search_module_precedes_later_locations() {
    checked("doom_wad_search_module_precedes_later_locations", &["0"]);
}
/// Lowercase working-directory candidates take precedence over every later location.
#[test]
fn doom_wad_search_cwd_lowercase_precedes_later_locations() {
    checked(
        "doom_wad_search_cwd_lowercase_precedes_later_locations",
        &["1"],
    );
}
/// Uppercase working-directory candidates take precedence over every later location.
#[test]
fn doom_wad_search_cwd_uppercase_precedes_later_locations() {
    checked(
        "doom_wad_search_cwd_uppercase_precedes_later_locations",
        &["2"],
    );
}
/// Home candidates take precedence over every later location.
#[test]
fn doom_wad_search_home_precedes_later_locations() {
    checked("doom_wad_search_home_precedes_later_locations", &["3"]);
}
/// Hidden home-directory candidates take precedence over every later location.
#[test]
fn doom_wad_search_home_dotdoom_precedes_later_locations() {
    checked(
        "doom_wad_search_home_dotdoom_precedes_later_locations",
        &["4"],
    );
}
/// An explicit path bypasses discovery and download.
#[test]
fn doom_wad_explicit_path_overrides_discovery() {
    checked("doom_wad_explicit_path_overrides_discovery", &["explicit"]);
}
/// A body with the wrong pinned digest is refused before caching.
#[test]
fn doom_wad_download_rejects_checksum_mismatch() {
    checked("doom_wad_download_rejects_checksum_mismatch", &["mismatch"]);
}
/// A missing explicit path leaves no cache or opened view.
#[test]
fn doom_wad_missing_path_notifies_without_partial_cache() {
    checked(
        "doom_wad_missing_path_notifies_without_partial_cache",
        &["missing"],
    );
}
/// An HTTP failure leaves no cache or opened view.
#[test]
fn doom_wad_http_error_notifies_without_partial_cache() {
    checked(
        "doom_wad_http_error_notifies_without_partial_cache",
        &["http-error"],
    );
}
/// A body failure leaves no cache or opened view.
#[test]
fn doom_wad_stream_error_notifies_without_partial_cache() {
    checked(
        "doom_wad_stream_error_notifies_without_partial_cache",
        &["stream-error"],
    );
}
/// A file that is not an IWAD is refused before the game reads it.
#[test]
fn doom_wad_file_that_is_not_an_iwad_is_refused() {
    checked(
        "doom_wad_file_that_is_not_an_iwad_is_refused",
        &["not-iwad"],
    );
}
/// An IWAD without the lumps a game start reads is refused before the game reads it.
#[test]
fn doom_wad_iwad_without_required_lumps_is_refused() {
    checked(
        "doom_wad_iwad_without_required_lumps_is_refused",
        &["no-lumps"],
    );
}
/// An IWAD whose directory lies outside the file is refused before the game reads it.
#[test]
fn doom_wad_iwad_with_directory_outside_the_file_is_refused() {
    checked(
        "doom_wad_iwad_with_directory_outside_the_file_is_refused",
        &["bad-directory"],
    );
}
/// Headless invocation performs no filesystem-dependent startup or request.
#[test]
fn doom_wad_headless_command_does_not_start_engine() {
    checked(
        "doom_wad_headless_command_does_not_start_engine",
        &["headless"],
    );
}

/// Loads one component with the native effect owner kept outside its store.
async fn runner(
    root: &Path,
    bus: &EventBusController<Local>,
    io: Rc<io::Io>,
    session: &maestro_extensions::runner::ExtensionRunnerSession,
) -> ExtensionRunner<Local> {
    let path = root
        .join("module/doom_overlay.wasm")
        .to_str()
        .unwrap()
        .to_owned();
    let runtime = create_extension_runtime::<Local>();
    let owner = load_extension_from_factory(
        maestro_extensions_wasmtime::artifact_factory_with_io(
            path.clone(),
            super::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "wad-host".into(),
                agent_dir: root.join("agent").to_str().unwrap().into(),
            },
            io,
            None,
        ),
        root.join("project").to_str().unwrap().into(),
        bus.for_extension(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        maestro_extensions::runner::ExtensionRunnerSession {
            cwd: session.cwd.clone(),
            session_manager: session.session_manager.clone(),
            model_registry: session.model_registry.clone(),
        },
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    super::support::bind_prompt(&runner, "controlled WAD");
    runner
}

/// Supplies deterministic chunks or a failure after their prefix.
pub(super) fn controlled_io(case: &str) -> Rc<io::Io> {
    let chunks = [
        Ok(b"con".to_vec()),
        Ok(b"trolled".to_vec()),
        if case == "stream-error" {
            Err("controlled body failure".into())
        } else {
            Ok(b" WAD".to_vec())
        },
    ];
    Rc::new(io::Io {
        requests: RefCell::default(),
        response: Rc::new(io::Reply {
            status: if case == "http-error" { 503 } else { 200 },
            reader: Rc::new(io::Reader {
                chunks: RefCell::new(chunks.into()),
            }),
        }),
    })
}
