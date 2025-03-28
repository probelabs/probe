use probe::path_resolver::{
    resolve_path, GoPathResolver, JavaScriptPathResolver, PathResolver, RustPathResolver,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn test_regular_path_resolution() {
    let path = "/some/regular/path";
    let result = resolve_path(path);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), PathBuf::from(path));
}

#[test]
fn test_go_path_resolution() {
    // Skip this test if go is not installed
    if Command::new("go").arg("version").output().is_err() {
        println!("Skipping test_go_path_resolution: Go is not installed");
        return;
    }

    // Test with a standard library package
    let result = resolve_path("go:fmt");
    assert!(
        result.is_ok(),
        "Failed to resolve 'go:fmt' package: {:?}",
        result
    );

    // The path should exist and contain the package
    let path = result.unwrap();
    assert!(path.exists(), "Path does not exist: {:?}", path);

    // The path should be a directory
    assert!(path.is_dir(), "Path is not a directory: {:?}", path);

    // The path should contain Go source files
    let has_go_files = std::fs::read_dir(&path)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "go"));

    assert!(has_go_files, "No Go source files found in: {:?}", path);
}

#[test]
fn test_js_path_resolution() {
    // Create a temporary directory with a package.json file
    let temp_dir = tempfile::tempdir().unwrap();
    let package_json_path = temp_dir.path().join("package.json");

    // Write a minimal package.json
    fs::write(
        &package_json_path,
        r#"{"name": "test-package", "version": "1.0.0"}"#,
    )
    .expect("Failed to write package.json");

    // Test with the path to the package.json file
    let path_str = format!("js:{}", package_json_path.to_str().unwrap());
    let result = resolve_path(&path_str);

    assert!(result.is_ok(), "Failed to resolve js: path: {:?}", result);
    assert_eq!(result.unwrap(), temp_dir.path());
}

#[test]
fn test_rust_path_resolution() {
    // Create a temporary directory with a Cargo.toml file
    let temp_dir = tempfile::tempdir().unwrap();
    let cargo_toml_path = temp_dir.path().join("Cargo.toml");

    // Write a minimal Cargo.toml
    fs::write(
        &cargo_toml_path,
        r#"[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("Failed to write Cargo.toml");

    // Test with the path to the Cargo.toml file
    let path_str = format!("rust:{}", cargo_toml_path.to_str().unwrap());
    let result = resolve_path(&path_str);

    assert!(result.is_ok(), "Failed to resolve rust: path: {:?}", result);
    assert_eq!(result.unwrap(), temp_dir.path());
}

#[test]
fn test_resolver_trait_implementations() {
    let go_resolver = GoPathResolver::new();
    let js_resolver = JavaScriptPathResolver::new();
    let rust_resolver = RustPathResolver::new();

    assert_eq!(go_resolver.prefix(), "go:");
    assert_eq!(js_resolver.prefix(), "js:");
    assert_eq!(rust_resolver.prefix(), "rust:");

    // Create a vector of trait objects
    let resolvers: Vec<Box<dyn PathResolver>> = vec![
        Box::new(go_resolver),
        Box::new(js_resolver),
        Box::new(rust_resolver),
    ];

    // Verify we can call methods on the trait objects
    for resolver in resolvers {
        let prefix = resolver.prefix();
        assert!(!prefix.is_empty(), "Prefix should not be empty");
        assert!(prefix.ends_with(':'), "Prefix should end with a colon");
    }
}
