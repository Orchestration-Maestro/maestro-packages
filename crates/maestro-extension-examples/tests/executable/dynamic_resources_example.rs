//! The dynamic resources example, discovered and reloaded by a real SDK session.
use crate::executable::support::{Root, session, widgets};
use maestro_app::{AgentSession, ResourceLoader};
use std::{fs, path::Path, rc::Rc};

/// Copies the shipped assets beside a copy of the prepared component and returns the
/// component's path.
fn install(directory: &Path) -> String {
    fs::create_dir_all(directory).unwrap();
    let shipped =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/extensions/dynamic-resources");
    for leaf in ["SKILL.md", "dynamic.md", "dynamic.json"] {
        fs::copy(shipped.join(leaf), directory.join(leaf)).unwrap();
    }
    let artifact = directory.join("resources.wasm");
    fs::copy(
        std::env::var_os("MAESTRO_DYNAMIC_RESOURCES_COMPONENT").unwrap(),
        &artifact,
    )
    .unwrap();
    artifact.to_str().unwrap().into()
}

/// Checks that a resource keeps the installed path and the extension that contributed it.
fn assert_installed(info: &maestro_app::SourceInfo, directory: &Path, leaf: &str) {
    assert_eq!(Path::new(&info.path), directory.join(leaf));
    assert_eq!(info.source, "extension:resources.wasm");
    assert_eq!(Path::new(info.base_dir.as_ref().unwrap()), directory);
}

/// Checks the skill the session discovered.
fn assert_skill(loader: &dyn ResourceLoader<maestro_app::Local>, directory: &Path, reloaded: bool) {
    let skills = loader.get_skills();
    assert!(skills.diagnostics.is_empty(), "{:?}", skills.diagnostics);
    let [skill] = skills.skills.as_slice() else {
        panic!("one skill expected: {:?}", skills.skills);
    };
    assert_eq!(skill.name, "dynamic-resources");
    assert_eq!(
        skill.description,
        if reloaded {
            "Reloaded skill"
        } else {
            "Example skill loaded from resources_discover"
        }
    );
    assert!(
        fs::read_to_string(&skill.file_path)
            .unwrap()
            .contains("# Dynamic Resources Skill")
    );
    assert_installed(&skill.source_info, directory, "SKILL.md");
}

/// Checks the prompt template the session discovered.
fn assert_prompt(
    loader: &dyn ResourceLoader<maestro_app::Local>,
    directory: &Path,
    reloaded: bool,
) {
    let prompts = loader.get_prompts();
    assert!(prompts.diagnostics.is_empty(), "{:?}", prompts.diagnostics);
    let [prompt] = prompts.prompts.as_slice() else {
        panic!("one prompt expected: {:?}", prompts.prompts);
    };
    assert_eq!(prompt.name, "dynamic");
    assert_eq!(
        prompt.description,
        "Example prompt template loaded from resources_discover"
    );
    assert_eq!(
        prompt.content,
        if reloaded {
            "Reloaded prompt."
        } else {
            "Summarize the current repository structure and mention any build or test commands."
        }
    );
    assert_installed(&prompt.source_info, directory, "dynamic.md");
}

/// Checks the theme the session discovered.
fn assert_theme(loader: &dyn ResourceLoader<maestro_app::Local>, directory: &Path, reloaded: bool) {
    let themes = loader.get_themes();
    assert!(themes.diagnostics.is_empty(), "{:?}", themes.diagnostics);
    let [theme] = themes.themes.as_slice() else {
        panic!("one theme expected: {:?}", themes.themes.len());
    };
    assert_eq!(
        theme.name(),
        Some(if reloaded {
            "dynamic-reloaded"
        } else {
            "dynamic-resources"
        })
    );
    assert_eq!(
        Path::new(theme.source_path().unwrap()),
        directory.join("dynamic.json")
    );
    assert_installed(
        &theme.source_info().unwrap().borrow(),
        directory,
        "dynamic.json",
    );
}

/// Checks the skill, the prompt template and the theme the session discovered.
fn assert_resources(
    loader: &dyn ResourceLoader<maestro_app::Local>,
    directory: &Path,
    reloaded: bool,
) {
    assert_skill(loader, directory, reloaded);
    assert_prompt(loader, directory, reloaded);
    assert_theme(loader, directory, reloaded);
}

/// Rewrites the installed assets, the way an author edits them between reloads.
fn edit_assets(directory: &Path) {
    for (leaf, from, to) in [
        (
            "SKILL.md",
            "Example skill loaded from resources_discover",
            "Reloaded skill",
        ),
        (
            "dynamic.md",
            "Summarize the current repository structure and mention any build or test commands.",
            "Reloaded prompt.",
        ),
        (
            "dynamic.json",
            "\"name\": \"dynamic-resources\"",
            "\"name\": \"dynamic-reloaded\"",
        ),
    ] {
        let path = directory.join(leaf);
        fs::write(&path, fs::read_to_string(&path).unwrap().replace(from, to)).unwrap();
    }
}

/// Discovers the installed resources, then reloads them after the files change.
async fn discover_and_reload(root: &Root, local: Rc<tokio::task::LocalSet>, name: &str) {
    let directory = root.path(name).join("dynamic-resources");
    let component = install(&directory);
    let session: Rc<AgentSession> =
        session::component(root, local, component, None, widgets()).await;
    // A reload rediscovers resources only for a session that has a binding.
    session
        .bind_extensions(maestro_app::ExtensionBindings {
            on_error: Some(Rc::new(|error| panic!("{error:?}"))),
            ..maestro_app::ExtensionBindings::default()
        })
        .await
        .unwrap();
    assert_resources(&**session.resource_loader(), &directory, false);
    edit_assets(&directory);
    session.reload().await.unwrap();
    assert_resources(&**session.resource_loader(), &directory, true);
    session.dispose();
}

/// Discovery and reload load the shipped skill, prompt template and theme from beside each
/// installed component, never from the session directory.
#[test]
fn dynamic_resources_discovery_and_reload_load_shipped_files() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = Rc::new(tokio::task::LocalSet::new());
    local.block_on(&runtime, async {
        let root = Root::new();
        for name in ["original", "relocated # Ω"] {
            discover_and_reload(&root, local.clone(), name).await;
        }
    });
}
