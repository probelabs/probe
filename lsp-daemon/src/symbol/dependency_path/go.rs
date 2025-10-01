use super::DependencyPathClassifier;
use std::path::Path;

pub struct GoDep;

impl DependencyPathClassifier for GoDep {
    fn classify(&self, absolute_path: &Path) -> Option<String> {
        let p = absolute_path.to_string_lossy();

        if let Ok(goroot) = std::env::var("GOROOT") {
            let root_src = format!("{}/src/", goroot.trim_end_matches('/'));
            if p.starts_with(&root_src) {
                let tail = &p[root_src.len()..];
                return Some(format!("/dep/go/system/{}", tail));
            }
        }

        if let Ok(gomodcache) = std::env::var("GOMODCACHE") {
            if p.starts_with(&gomodcache) {
                if let Some(rel) = p
                    .strip_prefix(&gomodcache)
                    .map(|s| s.trim_start_matches('/'))
                {
                    return Some(go_module_dep_path(rel));
                }
            }
        }
        if let Ok(gopath) = std::env::var("GOPATH") {
            let moddir = format!("{}/pkg/mod/", gopath.trim_end_matches('/'));
            if p.contains(&moddir) {
                if let Some(rel) = p.split_once(&moddir).map(|(_, r)| r) {
                    return Some(go_module_dep_path(rel));
                }
            }
        }

        None
    }
}

fn go_module_dep_path(rel: &str) -> String {
    // rel typically: "<module path with slashes>@<version>/<subpath>"
    if let Some(at_idx) = rel.rfind('@') {
        let module = &rel[..at_idx];
        let after_at = &rel[at_idx..]; // starts with "@version/..." or "@version"
        let sub = after_at.split_once('/').map(|(_, s)| s).unwrap_or("");
        if sub.is_empty() {
            format!("/dep/go/{}", module)
        } else {
            format!("/dep/go/{}/{}", module, sub)
        }
    } else {
        // Fallback: split at first '/'
        let mut parts = rel.splitn(2, '/');
        let module = parts.next().unwrap_or("");
        let sub = parts.next().unwrap_or("");
        if sub.is_empty() {
            format!("/dep/go/{}", module)
        } else {
            format!("/dep/go/{}/{}", module, sub)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn go_stdlib_maps() {
        env::set_var("GOROOT", "/go/root");
        let path = Path::new("/go/root/src/net/http/server.go");
        let dep = GoDep.classify(path).unwrap();
        assert_eq!(dep, "/dep/go/system/net/http/server.go");
    }

    #[test]
    fn go_modcache_maps() {
        env::set_var("GOMODCACHE", "/mod/cache");
        let path = Path::new("/mod/cache/github.com/gorilla/mux@v1.8.1/router.go");
        let dep = GoDep.classify(path).unwrap();
        assert_eq!(dep, "/dep/go/github.com/gorilla/mux/router.go");
    }
}
