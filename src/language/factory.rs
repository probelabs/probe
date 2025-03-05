use crate::language::language_trait::LanguageImpl;
use crate::language::rust::RustLanguage;
use crate::language::javascript::JavaScriptLanguage;
use crate::language::typescript::TypeScriptLanguage;
use crate::language::python::PythonLanguage;
use crate::language::go::GoLanguage;
use crate::language::c::CLanguage;
use crate::language::cpp::CppLanguage;
use crate::language::java::JavaLanguage;
use crate::language::ruby::RubyLanguage;
use crate::language::php::PhpLanguage;

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
        _ => None,
    }
}

