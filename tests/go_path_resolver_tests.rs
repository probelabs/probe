use probe::path_resolver::resolve_path;
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
fn test_nonexistent_go_package() {
    // Skip this test if go is not installed
    if Command::new("go").arg("version").output().is_err() {
        println!("Skipping test_nonexistent_go_package: Go is not installed");
        return;
    }

    // Test with a non-existent package
    let result = resolve_path("go:this_package_should_not_exist_12345");
    assert!(
        result.is_err(),
        "Expected error for non-existent package, got: {:?}",
        result
    );
}

#[test]
fn test_external_go_package() {
    // Skip this test if go is not installed
    if Command::new("go").arg("version").output().is_err() {
        println!("Skipping test_external_go_package: Go is not installed");
        return;
    }

    // This test is more complex as it requires an external package to be installed
    // We'll make it conditional on whether the package is already available
    let check_pkg = Command::new("go")
        .args(["list", "-f", "{{.Dir}}", "github.com/stretchr/testify"])
        .output();

    if let Ok(output) = check_pkg {
        if output.status.success() {
            // Package is available, test it
            let result = resolve_path("go:github.com/stretchr/testify");
            assert!(
                result.is_ok(),
                "Failed to resolve external package: {:?}",
                result
            );

            let path = result.unwrap();
            assert!(path.exists(), "Path does not exist: {:?}", path);
            assert!(path.is_dir(), "Path is not a directory: {:?}", path);
        } else {
            println!(
                "Skipping test_external_go_package: github.com/stretchr/testify is not installed"
            );
        }
    } else {
        println!(
            "Skipping test_external_go_package: Could not check for github.com/stretchr/testify"
        );
    }
}
