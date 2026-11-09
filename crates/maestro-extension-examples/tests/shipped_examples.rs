//! The examples folder and the index page of each of its folders.

use maestro_markdown::{Cell, Kind, LinkType, Node, Table};
use std::fs;
use std::path::{Path, PathBuf};

use maestro_markdown::test_support::nodes;

/// The workspace's examples folder, two levels above this crate.
fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Where a listed path sits. A link that leaves the examples folder names a page of the
/// Maestro package the published examples sit in, so it resolves in the locked Maestro source.
fn listed_path(folder: &Path, target: &str) -> PathBuf {
    let Ok(inside) = folder.strip_prefix(examples()) else {
        return folder.join(target);
    };
    let inside = relative_name(Path::new(""), inside);
    let joined = maestro_path::posix::join(&["examples", &inside, target]);
    if joined.starts_with("examples/") {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(joined)
    } else {
        crate::source::checkout().join(joined)
    }
}

/// Every folder index lists each of its entries and names only paths that exist.
#[test]
fn examples_folder_matches_the_shipped_files() {
    let root = examples();
    assert!(root.is_dir(), "{}", root.display());
    assert_index_lists(&root, &directory_names(&root));
    let path = root.join("extensions");
    assert_index_lists(&path, &example_names(&path, &path));
}

/// Assert an index page lists every entry of its folder and names only paths that exist.
fn assert_index_lists(folder: &Path, entries: &[String]) {
    let index = fs::read_to_string(folder.join("README.md")).unwrap();
    let listed = index_entries(&index);
    for name in entries.iter().filter(|name| *name != "README.md") {
        let target = if folder.join(name).is_dir() {
            format!("{name}/")
        } else {
            name.clone()
        };
        assert!(
            listed.contains(&target),
            "{} index omits {target}",
            folder.display()
        );
    }
    for target in listed {
        assert!(
            listed_path(folder, &target).exists(),
            "{} index names {target}",
            folder.display()
        );
    }
}

/// List the files an index page points at, through a relative link or a table row's
/// first code span.
fn index_entries(index: &str) -> Vec<String> {
    let document = maestro_markdown::parse(index.to_owned());
    let mut entries = Vec::new();
    for node in descendants(document.nodes.iter(), nodes::children) {
        match &node.kind {
            Kind::Link(_, _, destination, link)
                if link.link_type == LinkType::Inline && !destination.contains("://") =>
            {
                entries.push(destination.clone());
            }
            Kind::Table(table) => entries.extend(table_entries(table)),
            _ => {}
        }
    }
    entries
}

/// The file names one table lists, in row order.
fn table_entries(table: &Table) -> impl Iterator<Item = String> + '_ {
    table.rows.iter().filter_map(|row| first_code(row))
}

/// The examples index keeps its table rows and its controlled-run commands in file-name
/// order, so a new example inserts at its own place instead of appending at the end.
#[test]
fn examples_index_stays_in_file_name_order() {
    let index = fs::read_to_string(examples().join("extensions/README.md")).unwrap();
    let rows = table_rows(&index);
    assert!(
        rows.is_sorted(),
        "the examples table is not in file-name order: {rows:?}"
    );
    let filters = run_filters(&index);
    assert!(
        filters.is_sorted(),
        "the controlled-run commands are not in file-name order: {filters:?}"
    );
}

/// The file names every table of one index page lists, in authored order.
fn table_rows(index: &str) -> Vec<String> {
    let document = maestro_markdown::parse(index.to_owned());
    descendants(document.nodes.iter(), nodes::children)
        .filter_map(|node| match &node.kind {
            Kind::Table(table) => Some(table),
            _ => None,
        })
        .flat_map(table_entries)
        .collect()
}

/// The test filters one index page's controlled-run commands name, in authored order.
fn run_filters(index: &str) -> Vec<String> {
    let document = maestro_markdown::parse(index.to_owned());
    descendants(document.nodes.iter(), nodes::children)
        .filter_map(|node| match &node.kind {
            Kind::CodeBlock(code, _) => Some(code.as_str()),
            _ => None,
        })
        .flat_map(str::lines)
        .filter_map(|line| line.split_once("--lib "))
        .map(|(_, filter)| filter.trim().trim_start_matches("-- ").to_owned())
        .collect()
}

/// A flag mentioned in prose is not a controlled run, so only code blocks supply commands.
#[test]
fn run_filters_reads_only_commands_the_page_blocks_hold() {
    let index = "Pass `--lib -- filter` to narrow a run.\n\n```sh\ncargo nextest run --lib -- bookmark_\n```\n";
    assert_eq!(run_filters(index), ["bookmark_"]);
}

/// The first code span among one table row's cells.
fn first_code(row: &[Cell]) -> Option<String> {
    let cells = row.iter().flat_map(|cell| &cell.children);
    descendants(cells, nodes::children).find_map(|node| match &node.kind {
        Kind::Code(code) => Some(code.clone()),
        _ => None,
    })
}

/// Every node at or below the given ones, in authored order, read lazily through a
/// stack of child iterators.
fn descendants<'a>(
    roots: impl Iterator<Item = &'a Node> + 'a,
    children: impl Fn(&'a Kind) -> Box<dyn Iterator<Item = &'a Node> + 'a> + 'a,
) -> impl Iterator<Item = &'a Node> {
    let mut levels: Vec<Box<dyn Iterator<Item = &'a Node> + 'a>> = vec![Box::new(roots)];
    std::iter::from_fn(move || {
        while let Some(level) = levels.last_mut() {
            if let Some(node) = level.next() {
                levels.push(children(&node.kind));
                return Some(node);
            }
            levels.pop();
        }
        None
    })
}

/// Link labels and escaped backticks are Markdown syntax, not entry delimiters.
#[test]
fn index_entries_follow_the_markdown_structure() {
    let index = r"[`a](b)`](hello.rs)

| File | Notes |
| --- | --- |
| \`hello\` greets | `shutdown.rs` |
";
    assert_eq!(index_entries(index), ["hello.rs", "shutdown.rs"]);
}

/// The walk reads each node only when the consumer asks for it, depth first in authored order.
#[test]
fn descendants_read_each_node_on_demand_in_authored_order() {
    let document = maestro_markdown::parse("`a` *`b` **`c`** `d`* `e`\n\n> `f`\n".to_owned());
    let reads = std::cell::Cell::new(0);
    let walk = descendants(counted(document.nodes.iter(), &reads), |kind| {
        counted(nodes::children(kind), &reads)
    });
    let mut labels = Vec::new();
    for (taken, node) in walk.enumerate() {
        assert_eq!(
            reads.get(),
            taken + 1,
            "the walk read ahead of the consumer"
        );
        if let Kind::Code(code) = &node.kind {
            labels.push(code.clone());
        }
    }
    assert_eq!(labels, ["a", "b", "c", "d", "e", "f"]);
}

/// Count every node the walk reads from one level.
fn counted<'a>(
    level: impl Iterator<Item = &'a Node> + 'a,
    reads: &'a std::cell::Cell<usize>,
) -> Box<dyn Iterator<Item = &'a Node> + 'a> {
    Box::new(level.inspect(move |_| reads.set(reads.get() + 1)))
}

/// Both rejection branches run against disposable files, without changing shipped inputs.
#[test]
fn shipped_examples_index_rejects_unlisted_and_missing_files() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    fs::write(root.join("hello.rs"), "").unwrap();
    fs::write(root.join("README.md"), "# Examples\n").unwrap();
    let unlisted = std::panic::catch_unwind(|| assert_index_lists(root, &["hello.rs".to_owned()]));
    assert!(unlisted.is_err(), "an unlisted shipped file must fail");
    fs::write(
        root.join("README.md"),
        "| File |\n| --- |\n| `missing.rs` |\n",
    )
    .unwrap();
    let missing = std::panic::catch_unwind(|| assert_index_lists(root, &[]));
    assert!(missing.is_err(), "a listed missing file must fail");
    fs::write(
        root.join("README.md"),
        "| File |\n| --- |\n| `hello.rs` |\n",
    )
    .unwrap();
    assert_index_lists(root, &["hello.rs".to_owned()]);
}

/// List one directory's entry names in name order.
fn directory_names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// A directory that carries an entry point is one example; its support files need no
/// row.
#[test]
fn directory_example_needs_only_its_entry_point_row() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path();
    fs::create_dir(root.join("provider")).unwrap();
    fs::write(root.join("provider/main.rs"), "").unwrap();
    fs::write(root.join("provider/oauth.rs"), "").unwrap();
    fs::write(
        root.join("README.md"),
        "| File |\n| --- |\n| `provider/main.rs` |\n",
    )
    .unwrap();
    assert_eq!(example_names(root, root), ["README.md", "provider/main.rs"]);
    assert_index_lists(root, &example_names(root, root));
}

/// List the example names an index page below a directory must carry, relative to its
/// root, in name order.
///
/// Mirrors the rule `maestro_tooling::example_files::collect` owns: a directory that
/// holds `main.rs` is one example named by that entry point, and its support files need
/// no row.
fn example_names(root: &Path, directory: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let main = path.join("main.rs");
        if main.is_file() {
            names.push(relative_name(root, &main));
        } else if path.is_dir() {
            names.extend(example_names(root, &path));
        } else {
            names.push(relative_name(root, &path));
        }
    }
    names.sort();
    names
}

/// Name one file relative to the index's folder, joining with `/` whatever the host
/// separator.
fn relative_name(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap();
    let parts: Vec<_> = relative.iter().map(|part| part.to_string_lossy()).collect();
    parts.join("/")
}
