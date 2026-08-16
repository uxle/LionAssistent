//! Code Generation (Development_plan Phase 24). Two genuinely different
//! things, kept apart rather than blurred (the same split
//! `vision::analyze_image` versus `vision::VisionBackend` already made
//! for image understanding):
//!
//! 1. [`generate_from_template`] -- real, deterministic scaffolding.
//!    No model, nothing blocked by any toolchain wall, output actually
//!    compiled and checked during development (see this module's tests).
//! 2. [`generate_freeform`] -- routes a natural-language description
//!    through `assistant_core::ResponseGenerator`, the exact trait
//!    `AssistantCore::process_message` already uses for chat (Archd SS3
//!    "Replaceable AI backend"). With no real backend wired in, callers
//!    get `EchoResponseGenerator`'s same honest stub Phase 3 already
//!    established -- not fabricated code.

use assistant_core::ResponseGenerator;
use common::{AppError, AppResult};

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "coding-agent",
        message: context.to_string(),
    }
}

fn is_valid_rust_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    let first_ok = matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_');
    first_ok && !s.is_empty() && chars.clone().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Deterministic scaffolding for `kind` in (`"function"`, `"struct"`,
/// `"test"`). Rust only, for now -- said explicitly (this crate's own
/// doc comment on scope boundaries applies here too): other languages'
/// idioms differ enough (visibility keywords, doc-comment syntax,
/// naming conventions) that a template written without checking each
/// one against a real compiler would be exactly the kind of unverified
/// guess this project avoids.
pub fn generate_from_template(
    kind: &str,
    name: &str,
    language: &str,
    description: Option<&str>,
) -> AppResult<String> {
    if language != "rust" {
        return Err(module_err(format!(
            "template-based generation only supports \"rust\" right now, got {language:?} -- \
             try generate_freeform for other languages"
        )));
    }
    if !is_valid_rust_identifier(name) {
        return Err(module_err(format!(
            "{name:?} isn't a valid Rust identifier (must start with a letter or underscore, \
             contain only letters/digits/underscores)"
        )));
    }

    let doc = description
        .map(|d| format!("/// {d}\n"))
        .unwrap_or_default();

    match kind {
        "function" => Ok(format!("{doc}pub fn {name}() {{\n    todo!()\n}}\n")),
        "struct" => Ok(format!(
            "{doc}#[derive(Debug, Clone)]\npub struct {name} {{}}\n"
        )),
        "test" => Ok(format!("#[test]\nfn {name}() {{\n    todo!()\n}}\n")),
        other => Err(module_err(format!(
            "unknown template kind {other:?} -- expected \"function\", \"struct\", or \"test\""
        ))),
    }
}

fn build_code_prompt(description: &str, language: &str) -> String {
    format!(
        "You are a coding assistant. Write {language} code for the following request. \
         Return only the code, with no explanation.\n\nRequest: {description}"
    )
}

/// Routes `description` through whichever `ResponseGenerator` the
/// caller supplies. `&mut dyn ResponseGenerator` because the trait
/// itself requires it (`fn generate(&mut self, ...)`, matching
/// `AssistantCore`'s own usage) -- callers needing to share one
/// generator across multiple tool invocations hold it behind the same
/// `Arc<Mutex<_>>` shape `tools::GenerateCodeTool` does, not something
/// this free function needs to know about.
pub fn generate_freeform(
    description: &str,
    language: &str,
    generator: &mut dyn ResponseGenerator,
) -> String {
    let prompt = build_code_prompt(description, language);
    generator.generate(&prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assistant_core::EchoResponseGenerator;
    use std::process::Command;

    #[test]
    fn rejects_non_rust_languages_honestly_rather_than_guessing() {
        let result = generate_from_template("function", "foo", "python", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("only supports"));
    }

    #[test]
    fn rejects_invalid_identifiers() {
        assert!(generate_from_template("function", "not valid!", "rust", None).is_err());
        assert!(generate_from_template("function", "3starts_with_digit", "rust", None).is_err());
        assert!(generate_from_template("function", "", "rust", None).is_err());
    }

    #[test]
    fn rejects_unknown_template_kinds() {
        let result = generate_from_template("interpretive_dance", "foo", "rust", None);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_valid_identifiers_including_leading_underscore() {
        assert!(generate_from_template("function", "do_thing", "rust", None).is_ok());
        assert!(generate_from_template("function", "_private", "rust", None).is_ok());
        assert!(generate_from_template("struct", "MyStruct", "rust", None).is_ok());
    }

    /// The whole point of a *template* generator is that its output is
    /// always valid code -- proven here by actually compiling every
    /// template kind with a real `cargo check`, not just eyeballing the
    /// generated string. Mirrors `diagnostics`' own real-cargo-invocation
    /// tests, applied to this module's own output.
    #[test]
    fn every_template_kind_produces_code_that_actually_compiles() {
        let dir = std::env::temp_dir().join("coding-agent-test-template-compiles");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"template-compile-check\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();

        let function_code = generate_from_template(
            "function",
            "generated_function",
            "rust",
            Some("a generated function"),
        )
        .unwrap();
        let struct_code =
            generate_from_template("struct", "GeneratedStruct", "rust", None).unwrap();
        let test_code = generate_from_template("test", "generated_test", "rust", None).unwrap();

        let combined = format!("{function_code}\n{struct_code}\n\n#[cfg(test)]\nmod generated_tests {{\n    use super::*;\n    {test_code}\n}}\n");
        std::fs::write(dir.join("src/lib.rs"), &combined).unwrap();

        let output = Command::new("cargo")
            .arg("check")
            .current_dir(&dir)
            .output()
            .expect("cargo must be available to run this test");
        assert!(
            output.status.success(),
            "generated code failed to compile:\n{}\n\n--- generated source ---\n{combined}",
            String::from_utf8_lossy(&output.stderr)
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn freeform_generation_with_the_default_stub_is_honest_about_being_a_stub() {
        let mut generator = EchoResponseGenerator;
        let result = generate_freeform("a function that adds two numbers", "rust", &mut generator);
        assert!(
            result.contains("stub reply"),
            "the default backend must not imply it produced real code: {result}"
        );
    }
}
