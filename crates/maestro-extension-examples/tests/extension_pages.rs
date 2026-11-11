//! The extension pages' command blocks name only commands this workspace can run.
use regex::Regex;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Words a command block must not contain: packages and wrappers a reader of this workspace
/// does not have.
const FORBIDDEN: [&str; 4] = [
    "maestro-extensions-wasm",
    "maestro-extensions-wasmtime",
    "capped",
    "mise exec",
];

/// The package whose tests the pages' test filters select from.
const EXAMPLES_PACKAGE: &str = "maestro-extension-examples";

/// What the pages' commands are checked against.
struct Workspace {
    /// Every test of the examples package, as nextest names it: module path, then function.
    tests: Vec<String>,
    /// The example targets each package declares.
    examples: BTreeMap<String, Vec<String>>,
}

/// The repository root.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The module path a test source file adds to its tests' names.
fn module_path(tests: &Path, file: &Path) -> String {
    let relative = file.strip_prefix(tests).unwrap().with_extension("");
    let mut parts: Vec<String> = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.last().is_some_and(|last| last == "mod") {
        parts.pop();
    }
    parts.join("::")
}

/// Every `.rs` file below `directory`.
fn sources(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found
}

/// The names of the functions a source marks as tests.
fn test_functions(source: &str) -> Vec<String> {
    let function = Regex::new(r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)").unwrap();
    let mut marked = false;
    let mut names = Vec::new();
    for line in source.lines() {
        let line_text = line.trim();
        if line_text.starts_with("#[test]") || line_text.starts_with("#[tokio::test") {
            marked = true;
        } else if let Some(captures) = function.captures(line) {
            if marked {
                names.push(captures[1].to_owned());
            }
            marked = false;
        }
    }
    names
}

/// Every test of the examples package.
fn example_tests(root: &Path) -> Vec<String> {
    let tests = root.join("crates").join(EXAMPLES_PACKAGE).join("tests");
    let mut names = Vec::new();
    for file in sources(&tests) {
        let prefix = module_path(&tests, &file);
        for name in test_functions(&fs::read_to_string(&file).unwrap()) {
            names.push(format!("{prefix}::{name}"));
        }
    }
    names
}

/// The example targets each workspace package declares.
fn example_targets(root: &Path) -> BTreeMap<String, Vec<String>> {
    let target = Regex::new(r#"\[\[example\]\]\s*name\s*=\s*"([^"]+)""#).unwrap();
    let mut examples = BTreeMap::new();
    let package = Regex::new(r#"(?m)^name\s*=\s*"([^"]+)""#).unwrap();
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let manifest = entry.unwrap().path().join("Cargo.toml");
        let Ok(text) = fs::read_to_string(&manifest) else {
            continue;
        };
        let name = package.captures(&text).unwrap()[1].to_owned();
        let declared = target.captures_iter(&text).map(|c| c[1].to_owned());
        examples.insert(name, declared.collect());
    }
    examples
}

/// The workspace the real pages are checked against.
fn workspace(root: &Path) -> Workspace {
    Workspace {
        tests: example_tests(root),
        examples: example_targets(root),
    }
}

/// The text of each fenced code block of a page, in order.
fn blocks(text: &str) -> Vec<String> {
    let fence = Regex::new(r"(?s)```[a-z]*\n(.*?)```").unwrap();
    fence.captures_iter(text).map(|c| c[1].to_owned()).collect()
}

/// The commands of a block, with continued lines joined.
fn commands(block: &str) -> Vec<String> {
    block
        .replace("\\\n", " ")
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect()
}

/// Whether the nextest filter `test(<argument>)` selects `name`.
fn selects(argument: &str, name: &str) -> bool {
    if let Some(exact) = argument.strip_prefix('=') {
        return name == exact;
    }
    if let Some(expression) = argument.strip_prefix('/').and_then(|a| a.strip_suffix('/')) {
        return Regex::new(expression).unwrap().is_match(name);
    }
    name.contains(argument)
}

/// The `-p` package a command selects, if any.
fn package(command: &str) -> Option<&str> {
    let words: Vec<&str> = command.split_whitespace().collect();
    words
        .windows(2)
        .find(|pair| pair[0] == "-p")
        .map(|pair| pair[1])
}

/// What is wrong with one command of a page.
fn command_problems(command: &str, workspace: &Workspace) -> Vec<String> {
    let mut problems = Vec::new();
    for word in FORBIDDEN {
        if command.contains(word) {
            problems.push(format!("`{command}` names `{word}`"));
        }
    }
    let filter = Regex::new(r"test\(([^)]*)\)").unwrap();
    if package(command) == Some(EXAMPLES_PACKAGE) {
        for captures in filter.captures_iter(command) {
            if !workspace
                .tests
                .iter()
                .any(|name| selects(&captures[1], name))
            {
                problems.push(format!("`test({})` selects no test", &captures[1]));
            }
        }
    }
    let example = Regex::new(r"--example (\S+)").unwrap();
    if let Some(captures) = example.captures(command).filter(|c| !c[1].starts_with('<')) {
        let known = match package(command) {
            Some(package) => workspace.examples.get(package).cloned().unwrap_or_default(),
            None => workspace.examples.values().flatten().cloned().collect(),
        };
        if !known.contains(&captures[1].to_owned()) {
            problems.push(format!("`--example {}` names no target here", &captures[1]));
        }
    }
    problems
}

/// What is wrong with the order of the commands of a page.
fn order_problems(commands: &[String]) -> Vec<String> {
    let prepare = commands
        .iter()
        .position(|c| c.starts_with("just prepare-tests"));
    let nextest = commands
        .iter()
        .position(|c| c.contains("cargo nextest run"));
    match (prepare, nextest) {
        (None, Some(_)) => vec!["runs nextest without `just prepare-tests`".to_owned()],
        (Some(prepare), Some(nextest)) if prepare > nextest => {
            vec!["runs nextest before `just prepare-tests`".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// Whether the page gives a test command or names the issue that brings its runs.
fn names_its_runs(text: &str) -> bool {
    let issue = Regex::new(r"#\d+|/issues/\d+").unwrap();
    text.contains("cargo nextest run") || issue.is_match(text)
}

/// What is wrong with a page. `example_page` pages must also give or defer their test runs.
fn page_problems(text: &str, example_page: bool, workspace: &Workspace) -> Vec<String> {
    let all: Vec<String> = blocks(text)
        .iter()
        .flat_map(|block| commands(block))
        .collect();
    let mut problems: Vec<String> = all
        .iter()
        .flat_map(|command| command_problems(command, workspace))
        .collect();
    problems.extend(order_problems(&all));
    if example_page && !names_its_runs(text) {
        problems.push("gives no test command and names no issue".to_owned());
    }
    problems
}

/// The pages the check reads: each extension page and the SDK page.
fn pages(root: &Path) -> Vec<PathBuf> {
    let mut pages: Vec<PathBuf> = fs::read_dir(root.join("examples/extensions"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .collect();
    pages.sort();
    pages.push(root.join("examples/sdk/README.md"));
    pages
}

/// A workspace with two tests and one example target, for the rule witnesses.
fn small_workspace() -> Workspace {
    Workspace {
        tests: vec!["guest::hello_example::greets".into()],
        examples: BTreeMap::from([(EXAMPLES_PACKAGE.to_owned(), vec!["hello".to_owned()])]),
    }
}

/// A page with a test command that runs from a clean clone.
const GOOD_PAGE: &str = "# Hello\n\n```sh\njust prepare-tests\ncargo nextest run --locked \\\n  -p maestro-extension-examples -E 'test(hello_example)'\ncargo build --locked -p maestro-extension-examples --example hello\n```\n";

#[test]
fn extension_pages_name_only_commands_this_workspace_runs() {
    let root = root();
    let workspace = workspace(&root);
    let mut problems = Vec::new();
    for page in pages(&root) {
        let name = page.strip_prefix(&root).unwrap().display().to_string();
        let example_page = name.starts_with("examples/extensions/") && !name.ends_with("README.md");
        let text = fs::read_to_string(&page).unwrap();
        problems.extend(
            page_problems(&text, example_page, &workspace)
                .into_iter()
                .map(|problem| format!("{name}: {problem}")),
        );
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn page_check_refuses_each_command_this_workspace_cannot_run() {
    let workspace = small_workspace();
    assert!(page_problems(GOOD_PAGE, true, &workspace).is_empty());
    for (page, expected) in [
        (
            GOOD_PAGE.replace(
                "-p maestro-extension-examples -E",
                "-p maestro-extensions-wasmtime -E",
            ),
            "maestro-extensions-wasm",
        ),
        (
            GOOD_PAGE.replace("cargo build", "mise exec -- cargo build"),
            "mise exec",
        ),
        (
            GOOD_PAGE.replace("cargo nextest", "~/.local/bin/capped cargo nextest"),
            "capped",
        ),
        (
            GOOD_PAGE.replace("test(hello_example)", "test(absent_example)"),
            "selects no test",
        ),
        (
            GOOD_PAGE.replace("--example hello", "--example absent"),
            "names no target here",
        ),
        (
            GOOD_PAGE.replace("just prepare-tests\n", ""),
            "without `just prepare-tests`",
        ),
        (
            GOOD_PAGE.replace("just prepare-tests\n", "") + "```sh\njust prepare-tests\n```\n",
            "before `just prepare-tests`",
        ),
        ("# Hello\n\nNothing runs.\n".to_owned(), "names no issue"),
    ] {
        let problems = page_problems(&page, true, &workspace);
        assert!(
            problems.iter().any(|problem| problem.contains(expected)),
            "{expected}: {problems:?}"
        );
    }
    assert!(page_problems("# Hello\n\nDeferred to #75.\n", true, &workspace).is_empty());
}

#[test]
fn test_filters_select_by_substring_exact_name_and_expression() {
    let name = "guest::hello_example::greets";
    assert!(selects("hello_example", name));
    assert!(!selects("=greets", name));
    assert!(selects("=guest::hello_example::greets", name));
    assert!(!selects("=hello", name));
    assert!(selects("/^guest::.*greets$/", name));
    assert!(!selects("::greets_", name));
}

#[test]
fn gitlab_duo_page_loads_the_gateway_component_file() {
    let page = root().join("examples/extensions/custom-provider-gitlab-duo.md");
    let text = fs::read_to_string(page).unwrap();
    let loads = blocks(&text)
        .iter()
        .flat_map(|block| commands(block))
        .any(|command| {
            command.starts_with("maestro -e ./gateway/custom_provider_gitlab_duo.wasm ")
        });
    assert!(
        loads,
        "no command loads ./gateway/custom_provider_gitlab_duo.wasm"
    );
}
