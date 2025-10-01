use super::DependencyPathClassifier;
use std::path::Path;

pub struct RustDep;

impl DependencyPathClassifier for RustDep {
    fn classify(&self, absolute_path: &Path) -> Option<String> {
        let p = absolute_path.to_string_lossy();

        // Rust stdlib: .../rustlib/src/rust/library/<crate>/<sub>
        if let Some(idx) = p.find("/rustlib/src/rust/library/") {
            let after = &p[idx + "/rustlib/src/rust/library/".len()..];
            if let Some((crate_name, rest)) = split_first_component(after) {
                let tail = rest.unwrap_or("");
                let dep = if tail.is_empty() {
                    format!("/dep/rust/system/{}", crate_name)
                } else {
                    format!("/dep/rust/system/{}/{}", crate_name, tail)
                };
                return Some(dep);
            }
        }

        // Cargo registry: ~/.cargo/registry/src/<index>/<crate>-<version>/<sub>
        if let Some(idx) = p.find("/registry/src/") {
            let after = &p[idx + "/registry/src/".len()..];
            if let Some((after_index, _)) = split_first_component(after) {
                if let Some((crate_dir, rest)) = split_first_component(
                    after
                        .strip_prefix(after_index)
                        .unwrap_or(after)
                        .trim_start_matches('/'),
                ) {
                    let crate_name = strip_trailing_version(crate_dir);
                    let tail = rest.unwrap_or("");
                    let dep = if tail.is_empty() {
                        format!("/dep/rust/{}", crate_name)
                    } else {
                        format!("/dep/rust/{}/{}", crate_name, tail)
                    };
                    return Some(dep);
                }
            }
        }

        None
    }
}

fn split_first_component(s: &str) -> Option<(&str, Option<&str>)> {
    let mut it = s.splitn(2, '/');
    let first = it.next()?;
    let rest = it.next();
    Some((first, rest))
}

fn strip_trailing_version(crate_dir: &str) -> String {
    if let Some(idx) = crate_dir.rfind('-') {
        let (name, ver) = crate_dir.split_at(idx);
        let ver = &ver[1..];
        if ver.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return name.to_string();
        }
    }
    crate_dir.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_stdlib_maps_to_dep() {
        let path = Path::new("/usr/lib/rustlib/src/rust/library/alloc/src/lib.rs");
        let dep = RustDep.classify(path).unwrap();
        assert!(dep.starts_with("/dep/rust/system/alloc"));
    }

    #[test]
    fn rust_registry_maps_to_dep() {
        let path = Path::new(
            "/home/u/.cargo/registry/src/index.crates.io-6f17d22bba15001f/serde-1.0.210/src/lib.rs",
        );
        let dep = RustDep.classify(path).unwrap();
        assert!(dep.starts_with("/dep/rust/serde"));
    }
}
