use std::path::Path;

pub trait DependencyPathClassifier {
    fn classify(&self, absolute_path: &Path) -> Option<String>;
}

mod go;
mod js;
mod rust;

pub use go::GoDep;
pub use js::JsDep;
pub use rust::RustDep;

/// Try all registered classifiers; return the first match.
pub fn classify_absolute_path(absolute: &Path) -> Option<String> {
    let classifiers: [&dyn DependencyPathClassifier; 3] = [&RustDep, &JsDep, &GoDep];
    for cls in classifiers {
        if let Some(dep) = cls.classify(absolute) {
            return Some(dep);
        }
    }
    None
}
