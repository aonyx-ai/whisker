//! The TypeScript provider against real projects on disk
//!
//! Each test writes a small project with a `tsconfig.json`, loads it
//! through tsrs, and asks the provider about one file. The projects live
//! in temporary directories so each test controls exactly what the
//! program holds.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use whisker_types::{
    Coverage, CoverageGap, DecoratedNode, DecoratedTree, DecorationProvider, Language,
};
use whisker_typescript::TypeScriptDecorationProvider;
use whisker_typescript::decorations::TypeText;

/// A temporary project: its directory and the files written into it
struct Project {
    directory: TempDir,
}

impl Project {
    /// Writes `tsconfig` and every `(path, text)` file into a new project
    fn new(tsconfig: &str, files: &[(&str, &str)]) -> Self {
        let directory = tempfile::tempdir().expect("temporary directory should be created");
        std::fs::write(directory.path().join("tsconfig.json"), tsconfig)
            .expect("tsconfig should be written");
        for (path, text) in files {
            let path = directory.path().join(path);
            std::fs::create_dir_all(path.parent().expect("a file has a parent"))
                .expect("directories should be created");
            std::fs::write(path, text).expect("file should be written");
        }
        Self { directory }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.directory.path().join(relative)
    }

    fn provider(&self) -> TypeScriptDecorationProvider {
        TypeScriptDecorationProvider::load(self.directory.path()).expect("the project should load")
    }
}

const STRICT: &str = r#"{ "compilerOptions": { "strict": true, "target": "es2022" }, "include": ["src"], "exclude": ["src/generated"] }"#;

/// Parses `source` as the file at `path`, picking the grammar by extension
fn parse(path: &Path, source: &str) -> DecoratedTree {
    let grammar = match path.extension().and_then(|extension| extension.to_str()) {
        Some("tsx") => whisker_typescript::tsx_language(),
        Some(_) | None => whisker_typescript::language(),
    };
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&grammar)
        .expect("the grammar should load");
    let tree = parser.parse(source, None).expect("the source should parse");
    DecoratedTree::new(tree, Language::TypeScript, source.to_owned(), path)
}

/// Decorates the file at `relative` with its text on disk
///
/// Returns the tree with the provider's decorations merged in, or the gap
/// the provider declined the file with.
fn decorate(project: &Project, relative: &str) -> Result<DecoratedTree, CoverageGap> {
    let path = project.path(relative);
    let source = std::fs::read_to_string(&path).expect("the file should exist");
    let mut tree = parse(&path, &source);
    let coverage = project
        .provider()
        .decorate(&tree)
        .expect("decoration should not fail");
    match coverage {
        Coverage::Covered(decorations) => {
            tree.merge_decorations(decorations);
            Ok(tree)
        }
        Coverage::NotCovered(gap) => Err(gap),
    }
}

/// Returns the type text of every node whose text is `name`, in order
fn types_of(tree: &DecoratedTree, name: &str) -> Vec<Option<String>> {
    fn walk(node: &DecoratedNode<'_>, name: &str, found: &mut Vec<Option<String>>) {
        if node.text() == name && node.named_child_count() == 0 {
            found.push(
                node.decoration::<TypeText>()
                    .map(|ty| ty.as_str().to_owned()),
            );
        }
        for child in node.named_children() {
            walk(&child, name, found);
        }
    }

    let mut found = Vec::new();
    walk(&tree.root_node(), name, &mut found);
    found
}

#[test]
fn decorate_a_changed_file_reports_stale_source() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);
    let path = project.path("src/main.ts");
    let tree = parse(&path, "export const a = 2;\n");

    let coverage = project.provider().decorate(&tree).expect("should decorate");

    let Coverage::NotCovered(CoverageGap::StaleSource) = &coverage else {
        panic!("expected a stale-source gap, got {coverage:?}");
    };
}

#[test]
fn decorate_a_file_no_include_names_reports_unreachable() {
    let project = Project::new(
        STRICT,
        &[
            ("src/main.ts", "export const a = 1;\n"),
            ("scripts/build.ts", "export const b = 2;\n"),
        ],
    );

    let gap = decorate(&project, "scripts/build.ts").expect_err("should decline");

    let CoverageGap::Unreachable { .. } = gap else {
        panic!("expected an unreachable gap, got {gap:?}");
    };
}

#[test]
fn decorate_a_file_outside_the_project_reports_outside_root() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);
    let elsewhere = tempfile::tempdir().expect("temporary directory should be created");
    let path = elsewhere.path().join("other.ts");
    let tree = parse(&path, "export const a = 1;\n");

    let coverage = project.provider().decorate(&tree).expect("should decorate");

    let Coverage::NotCovered(CoverageGap::OutsideRoot { root }) = &coverage else {
        panic!("expected an outside-root gap, got {coverage:?}");
    };
    assert_eq!(
        std::fs::canonicalize(&**root).unwrap(),
        std::fs::canonicalize(project.directory.path()).unwrap()
    );
}

#[test]
fn decorate_a_file_the_exclude_list_names_reports_excluded() {
    let project = Project::new(
        STRICT,
        &[
            ("src/main.ts", "export const a = 1;\n"),
            ("src/generated/types.ts", "export const b = 2;\n"),
        ],
    );

    let gap = decorate(&project, "src/generated/types.ts").expect_err("should decline");

    let CoverageGap::ExcludedByToolchain { .. } = gap else {
        panic!("expected an excluded gap, got {gap:?}");
    };
}

#[test]
fn decorate_types_identifiers_after_multibyte_text() {
    let project = Project::new(
        STRICT,
        &[(
            "src/main.ts",
            "// é and 日本 come first\nconst café = \"x\";\nexport const tasse = café;\n",
        )],
    );

    let tree = decorate(&project, "src/main.ts").expect("should cover the file");

    assert_eq!(
        types_of(&tree, "café"),
        vec![Some("\"x\"".to_owned()), Some("\"x\"".to_owned())]
    );
}

#[test]
fn decorate_types_the_identifiers_of_a_file_in_the_program() {
    let project = Project::new(
        STRICT,
        &[(
            "src/main.ts",
            "function add(a: number, b: number) { return a + b; }\n\
             export const total = add(1, 2);\n",
        )],
    );

    let tree = decorate(&project, "src/main.ts").expect("should cover the file");

    assert_eq!(types_of(&tree, "total"), vec![Some("number".to_owned())]);
    assert_eq!(
        types_of(&tree, "add"),
        vec![
            Some("(a: number, b: number) => number".to_owned()),
            Some("(a: number, b: number) => number".to_owned())
        ]
    );
}

#[test]
fn decorate_types_the_identifiers_of_a_tsx_file() {
    let project = Project::new(
        r#"{ "compilerOptions": { "strict": true, "jsx": "preserve" }, "include": ["src"] }"#,
        &[(
            "src/view.tsx",
            "declare namespace JSX { interface IntrinsicElements { div: {} } }\n\
             const label = \"hi\";\n\
             export const view = <div>{label}</div>;\n",
        )],
    );

    let tree = decorate(&project, "src/view.tsx").expect("should cover the file");

    assert_eq!(
        types_of(&tree, "label"),
        vec![Some("\"hi\"".to_owned()), Some("\"hi\"".to_owned())]
    );
}

#[test]
fn load_finds_the_configuration_above_the_path() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);

    let provider = TypeScriptDecorationProvider::load(&project.path("src"));

    assert!(provider.is_ok());
}

#[test]
fn load_without_a_configuration_returns_error() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");

    let error = TypeScriptDecorationProvider::load(directory.path())
        .err()
        .expect("loading should fail");

    assert!(
        error.to_string().contains("discover a TypeScript project"),
        "{error:#}"
    );
}

#[test]
fn name_returns_typescript() {
    let project = Project::new(STRICT, &[("src/main.ts", "export const a = 1;\n")]);

    let name = project.provider().name();

    assert_eq!(name.as_str(), "typescript");
}
