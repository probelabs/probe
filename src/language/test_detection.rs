use std::path::Path;

/// Function to determine if a file is a test file based on common naming conventions and directory patterns
pub fn is_test_file(path: &Path) -> bool {
    let _debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";

    // Check file name patterns
    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        // Rust: *_test.rs, *_tests.rs, test_*.rs, tests.rs
        if file_name.ends_with("_test.rs")
            || file_name.ends_with("_tests.rs")
            || (file_name.starts_with("test_") && file_name.ends_with(".rs"))
            || file_name == "tests.rs"
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (Rust pattern): {file_name}");
            }
            return true;
        }

        // JavaScript/TypeScript: *.test.js, *.spec.js, *.test.ts, *.spec.ts
        if file_name.ends_with(".test.js")
            || file_name.ends_with(".spec.js")
            || file_name.ends_with(".test.jsx")
            || file_name.ends_with(".spec.jsx")
            || file_name.ends_with(".test.ts")
            || file_name.ends_with(".spec.ts")
            || file_name.ends_with(".test.tsx")
            || file_name.ends_with(".spec.tsx")
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (JS/TS pattern): {file_name}");
            }
            return true;
        }

        // Python: test_*.py, *_test.py
        if file_name.starts_with("test_") && file_name.ends_with(".py")
            || file_name.ends_with("_test.py")
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (Python pattern): {file_name}");
            }
            return true;
        }

        // Go: *_test.go
        if file_name.ends_with("_test.go") {
            if _debug_mode {
                println!("DEBUG: Test file detected (Go pattern): {file_name}");
            }
            return true;
        }

        // C/C++: test_*.c, *_test.c, *_tests.c, test_*.cpp, *_test.cpp, *_tests.cpp
        if (file_name.starts_with("test_")
            || file_name.ends_with("_test.c")
            || file_name.ends_with("_tests.c"))
            && (file_name.ends_with(".c") || file_name.ends_with(".h"))
            || (file_name.starts_with("test_")
                || file_name.ends_with("_test.cpp")
                || file_name.ends_with("_tests.cpp"))
                && (file_name.ends_with(".cpp")
                    || file_name.ends_with(".hpp")
                    || file_name.ends_with(".cc")
                    || file_name.ends_with(".hxx")
                    || file_name.ends_with(".cxx"))
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (C/C++ pattern): {file_name}");
            }
            return true;
        }

        // Java: *Test.java, Test*.java
        if file_name.ends_with("Test.java")
            || file_name.starts_with("Test") && file_name.ends_with(".java")
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (Java pattern): {file_name}");
            }
            return true;
        }

        // Ruby: test_*.rb, *_test.rb, *_spec.rb
        if file_name.starts_with("test_") && file_name.ends_with(".rb")
            || file_name.ends_with("_test.rb")
            || file_name.ends_with("_spec.rb")
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (Ruby pattern): {file_name}");
            }
            return true;
        }

        // Crystal: spec files conventionally use *_spec.cr
        if file_name.ends_with("_spec.cr") {
            if _debug_mode {
                println!("DEBUG: Test file detected (Crystal pattern): {file_name}");
            }
            return true;
        }

        // Haskell: common Hspec/Tasty/QuickCheck test naming conventions
        if file_name.ends_with("Spec.hs")
            || file_name.ends_with("Spec.lhs")
            || file_name.ends_with("Test.hs")
            || file_name.ends_with("Test.lhs")
            || file_name.starts_with("Test")
                && (file_name.ends_with(".hs") || file_name.ends_with(".lhs"))
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (Haskell pattern): {file_name}");
            }
            return true;
        }

        // PHP: *Test.php, Test*.php
        if file_name.ends_with("Test.php")
            || file_name.starts_with("Test") && file_name.ends_with(".php")
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (PHP pattern): {file_name}");
            }
            return true;
        }

        // Solidity/Foundry: *.t.sol, *Test.sol, Test*.sol
        if file_name.ends_with(".t.sol")
            || file_name.ends_with("Test.sol")
            || file_name.starts_with("Test") && file_name.ends_with(".sol")
        {
            if _debug_mode {
                println!("DEBUG: Test file detected (Solidity pattern): {file_name}");
            }
            return true;
        }
    }

    // Check directory patterns. Use path components so relative paths like
    // test/foo.rb are handled the same as project/test/foo.rb.
    let has_test_dir = path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(
            name.as_ref(),
            "test" | "tests" | "spec" | "specs" | "__tests__" | "__test__"
        )
    });

    if has_test_dir {
        if _debug_mode {
            let path_str = path.to_string_lossy();
            println!("DEBUG: Test file detected (in test directory): {path_str}");
        }
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::is_test_file;
    use std::path::Path;

    #[test]
    fn detects_ruby_test_file_conventions() {
        assert!(is_test_file(Path::new("test/user_service.rb")));
        assert!(is_test_file(Path::new("test_user_service.rb")));
        assert!(is_test_file(Path::new("user_service_test.rb")));
        assert!(is_test_file(Path::new("user_service_spec.rb")));
        assert!(is_test_file(Path::new("spec/models/user_service.rb")));
    }

    #[test]
    fn does_not_overmatch_non_test_ruby_files() {
        assert!(!is_test_file(Path::new("keyword_highlighting.rb")));
        assert!(!is_test_file(Path::new("contest.rb")));
        assert!(!is_test_file(Path::new("latest.rb")));
        assert!(!is_test_file(Path::new("app/services/user_service.rb")));
    }
}
