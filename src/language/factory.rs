use crate::language::c::CLanguage;
use crate::language::cpp::CppLanguage;
use crate::language::csharp::CSharpLanguage;
use crate::language::go::GoLanguage;
use crate::language::java::JavaLanguage;
use crate::language::javascript::JavaScriptLanguage;
use crate::language::language_trait::LanguageImpl;
use crate::language::php::PhpLanguage;
use crate::language::python::PythonLanguage;
use crate::language::ruby::RubyLanguage;
use crate::language::rust::RustLanguage;
use crate::language::swift::SwiftLanguage;
use crate::language::typescript::TypeScriptLanguage;

/// Factory function to get the appropriate language implementation based on file extension
pub fn get_language_impl(extension: &str) -> Option<Box<dyn LanguageImpl>> {
    match extension {
        "rs" => Some(Box::new(RustLanguage::new())),
        "js" | "jsx" => Some(Box::new(JavaScriptLanguage::new())),
        "ts" => Some(Box::new(TypeScriptLanguage::new_typescript())),
        "tsx" => Some(Box::new(TypeScriptLanguage::new_tsx())),
        "py" => Some(Box::new(PythonLanguage::new())),
        "go" => Some(Box::new(GoLanguage::new())),
        "c" | "h" => Some(Box::new(CLanguage::new())),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(Box::new(CppLanguage::new())),
        "java" => Some(Box::new(JavaLanguage::new())),
        "rb" => Some(Box::new(RubyLanguage::new())),
        "php" => Some(Box::new(PhpLanguage::new())),
        "swift" => Some(Box::new(SwiftLanguage::new())),
        "cs" => Some(Box::new(CSharpLanguage::new())),
        _ => None,
    }
}
