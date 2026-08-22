//! Embedded plugin templates (Development_plan Phase 29 "Templates")
//! and the substitution engine [`generator.rs`](crate::generator) runs
//! them through.
//!
//! Deliberately plain `{{key}}` string replacement, not a templating
//! engine dependency: three small files with a dozen placeholders
//! between them don't need conditionals, loops, or partials -- pulling
//! in a templating crate for this would be exactly the kind of
//! unrelated dependency DIG SS11 warns against adding without real
//! need. The three files are real, checked-in files under
//! `templates/`, embedded via `include_str!` rather than written as
//! Rust string literals, so they can be read, diffed, and edited as
//! what they actually are -- Cargo.toml/manifest.toml/Rust source --
//! not escaped text buried in a `.rs` file.

pub const CARGO_TOML_TEMPLATE: &str = include_str!("../templates/Cargo.toml.template");
pub const MANIFEST_TOML_TEMPLATE: &str = include_str!("../templates/manifest.toml.template");
pub const LIB_RS_TEMPLATE: &str = include_str!("../templates/lib.rs.template");

/// Replaces every `{{key}}` in `template` with the matching value from
/// `substitutions`, in a single left-to-right pass.
///
/// Panics only never -- an unmatched `{{...}}` left in the output is a
/// real, silent bug (a missing substitution key), so callers check for
/// that explicitly (see `generator::render`'s own `debug_assert`)
/// rather than this function guessing at what to do about it.
pub fn render_template(template: &str, substitutions: &[(&str, &str)]) -> String {
    let mut output = template.to_string();
    for (key, value) in substitutions {
        output = output.replace(&format!("{{{{{key}}}}}"), value);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_every_occurrence_of_a_placeholder() {
        let out = render_template("{{name}} says hi to {{name}}", &[("name", "Lion")]);
        assert_eq!(out, "Lion says hi to Lion");
    }

    #[test]
    fn leaves_unrelated_text_untouched() {
        let out = render_template("plain text, no placeholders", &[("name", "Lion")]);
        assert_eq!(out, "plain text, no placeholders");
    }

    #[test]
    fn a_substitution_key_with_no_matching_placeholder_is_harmless() {
        let out = render_template("hello", &[("unused", "value")]);
        assert_eq!(out, "hello");
    }

    #[test]
    fn the_three_embedded_templates_are_non_empty_real_files() {
        assert!(CARGO_TOML_TEMPLATE.contains("[package]"));
        assert!(MANIFEST_TOML_TEMPLATE.contains("id ="));
        assert!(LIB_RS_TEMPLATE.contains("impl Plugin for"));
    }
}
