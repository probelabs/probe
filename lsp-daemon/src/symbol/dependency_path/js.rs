use super::DependencyPathClassifier;
use std::path::Path;

pub struct JsDep;

impl DependencyPathClassifier for JsDep {
    fn classify(&self, absolute_path: &Path) -> Option<String> {
        let p = absolute_path.to_string_lossy();
        if let Some(idx) = p.find("/node_modules/") {
            let after = &p[idx + "/node_modules/".len()..];
            if after.starts_with('@') {
                if let Some((scope, rest1)) = split_first_component(after) {
                    if let Some((pkg, rest2)) = rest1.and_then(|r| split_first_component(r)) {
                        let tail = rest2.unwrap_or("");
                        let name = format!("{}/{}", scope, pkg);
                        return Some(if tail.is_empty() {
                            format!("/dep/js/{}", name)
                        } else {
                            format!("/dep/js/{}/{}", name, tail)
                        });
                    }
                }
            } else if let Some((pkg, rest)) = split_first_component(after) {
                let tail = rest.unwrap_or("");
                return Some(if tail.is_empty() {
                    format!("/dep/js/{}", pkg)
                } else {
                    format!("/dep/js/{}/{}", pkg, tail)
                });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_modules_unscoped() {
        let path = Path::new("/repo/node_modules/lodash/index.js");
        let dep = JsDep.classify(path).unwrap();
        assert!(dep.starts_with("/dep/js/lodash"));
    }

    #[test]
    fn node_modules_scoped() {
        let path = Path::new("/repo/node_modules/@types/node/fs.d.ts");
        let dep = JsDep.classify(path).unwrap();
        assert!(dep.starts_with("/dep/js/@types/node"));
    }
}
