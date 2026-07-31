use probe_code::language::c::CLanguage;
use probe_code::language::cpp::CppLanguage;
use probe_code::language::crystal::CrystalLanguage;
use probe_code::language::csharp::CSharpLanguage;
use probe_code::language::go::GoLanguage;
use probe_code::language::haskell::HaskellLanguage;
use probe_code::language::html::HtmlLanguage;
use probe_code::language::java::JavaLanguage;
use probe_code::language::javascript::JavaScriptLanguage;
use probe_code::language::language_trait::LanguageImpl;
use probe_code::language::markdown::MarkdownLanguage;
use probe_code::language::php::PhpLanguage;
use probe_code::language::python::PythonLanguage;
use probe_code::language::ruby::RubyLanguage;
use probe_code::language::rust::RustLanguage;
use probe_code::language::solidity::SolidityLanguage;
use probe_code::language::swift::SwiftLanguage;
use probe_code::language::typescript::TypeScriptLanguage;
use probe_code::language::yaml::YamlLanguage;

/// Factory function to get the appropriate language implementation based on file extension
pub fn get_language_impl(extension: &str) -> Option<Box<dyn LanguageImpl>> {
    match extension {
        "rs" => Some(Box::new(RustLanguage::new())),
        "js" | "jsx" => Some(Box::new(JavaScriptLanguage::new())),
        "ts" => Some(Box::new(TypeScriptLanguage::new_typescript())),
        "tsx" => Some(Box::new(TypeScriptLanguage::new_tsx())),
        "py" => Some(Box::new(PythonLanguage::new())),
        "go" => Some(Box::new(GoLanguage::new())),
        "c" => Some(Box::new(CLanguage::new())),
        // `.h` headers are ambiguous but overwhelmingly used for C++ in practice
        // (namespaces, classes, templates). tree-sitter-cpp is a superset of C, so it
        // parses plain C headers correctly too. Routing `.h` to the C grammar caused
        // C++ headers to fail to parse (ERROR root node), which made symbol extraction
        // fall back to a single-line text search instead of the full AST node.
        "cpp" | "cc" | "cxx" | "h" | "hpp" | "hxx" | "hh" => Some(Box::new(CppLanguage::new())),
        "java" => Some(Box::new(JavaLanguage::new())),
        "rb" => Some(Box::new(RubyLanguage::new())),
        "php" => Some(Box::new(PhpLanguage::new())),
        "swift" => Some(Box::new(SwiftLanguage::new())),
        "cs" => Some(Box::new(CSharpLanguage::new())),
        "sol" => Some(Box::new(SolidityLanguage::new())),
        "cr" => Some(Box::new(CrystalLanguage::new())),
        "hs" | "lhs" => Some(Box::new(HaskellLanguage::new())),
        "html" | "htm" => Some(Box::new(HtmlLanguage::new())),
        "md" | "markdown" => Some(Box::new(MarkdownLanguage::new())),
        "yaml" | "yml" => Some(Box::new(YamlLanguage::new())),
        _ => None,
    }
}
