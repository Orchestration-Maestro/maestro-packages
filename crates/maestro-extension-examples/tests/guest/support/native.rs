//! Loading a component with the application's native effects (file watches, child processes,
//! HTTP), which the harness's loader does not supply.
use maestro_extensions_wasmtime::test_support::{
    Loaded,
    maestro_extensions::{
        ExtensionCommandContextActions, ExtensionExecution, ExtensionRunner, ExtensionWidgets,
        LiveTheme, Local, ModuleLoader,
        execution::ExtensionRunnerSession,
        load_extensions_with_runtime,
        test_support::{ToolkitWidgets, diagnostics},
        types::ProviderActions,
    },
    unconfigured_command_operations, unconfigured_context_actions,
};
use std::rc::Rc;

/// Runs a test body on a local task set it can hand to the application's native effects.
pub fn local_native<F: Future>(test: impl FnOnce(Rc<tokio::task::LocalSet>) -> F) -> F::Output {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, test(local.clone()))
}

/// The loader that builds each component over the application's native effects.
fn loader(local: &Rc<tokio::task::LocalSet>) -> ModuleLoader<Local> {
    let io = maestro_app::extension_native_io::extension_native_io(local);
    let widgets: Rc<dyn ExtensionWidgets> =
        Rc::new(ToolkitWidgets::new(LiveTheme::default()).unwrap());
    let agent_dir = maestro_test_process::run_agent_directory().unwrap_or_default();
    Rc::new(move |path: &str| {
        let factory = maestro_extensions_wasmtime::artifact_factory_with_io(
            path.to_owned(),
            widgets.clone(),
            maestro_extensions_wasmtime::Installation {
                version: env!("CARGO_PKG_VERSION").into(),
                agent_dir: agent_dir.clone(),
            },
            io.clone(),
            None,
        );
        Box::pin(std::future::ready(Ok(Some(factory))))
    })
}

/// Loads the component `variable` names into `harness` with the application's native effects
/// (file watches, child processes, HTTP), which the harness's own loader does not supply.
pub async fn load_native(
    harness: &maestro_extensions_wasmtime::test_support::Harness,
    variable: &str,
    local: &Rc<tokio::task::LocalSet>,
) -> Loaded {
    let extensions = load_extensions_with_runtime(
        vec![super::component(variable)],
        &harness.cwd,
        &harness.bus,
        &loader(local),
        harness.runtime.clone(),
    )
    .await
    .unwrap();
    let errors: Vec<_> = extensions.errors.iter().map(|error| &error.error).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let runner = Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(std::sync::RwLock::new(extensions.extensions)),
        extensions.runtime,
        ExtensionRunnerSession {
            cwd: harness.cwd.clone(),
            session_manager: harness.session_manager.clone(),
            model_registry: harness.model_registry.clone(),
        },
        Rc::new(diagnostics::write_stderr),
        LiveTheme::default(),
    ));
    runner
        .bind_core(
            Local::unbound_actions(),
            unconfigured_context_actions(),
            ProviderActions::default(),
        )
        .unwrap();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        unconfigured_command_operations(),
    )));
    Loaded {
        runner,
        errors: extensions.errors,
    }
}
