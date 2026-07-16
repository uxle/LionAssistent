//! Tool input schemas and argument validation (TCLIS SS5 "Standard Tool
//! Definition", SS7 "Validation Rules").
//!
//! Deliberately not full JSON Schema: a small, closed set of parameter
//! kinds covers what a local desktop assistant's tools actually need,
//! and is simple enough to validate without a schema-validation
//! dependency. Arguments themselves are `serde_json::Value` so tool
//! calls can round-trip through JSON when something produces them that
//! way later (an LLM's structured tool-call output, Phase 4/8).

use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub type ToolArgs = HashMap<String, Value>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    String,
    Integer,
    Float,
    Bool,
    StringArray,
}

impl ParamKind {
    fn matches(&self, value: &Value) -> bool {
        match self {
            ParamKind::String => value.is_string(),
            ParamKind::Integer => value.is_i64() || value.is_u64(),
            ParamKind::Float => value.is_f64() || value.is_i64() || value.is_u64(),
            ParamKind::Bool => value.is_boolean(),
            ParamKind::StringArray => {
                value.is_array()
                    && value
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParamSchema {
    pub name: String,
    pub kind: ParamKind,
    pub required: bool,
}

impl ParamSchema {
    pub fn required(name: impl Into<String>, kind: ParamKind) -> Self {
        Self {
            name: name.into(),
            kind,
            required: true,
        }
    }

    pub fn optional(name: impl Into<String>, kind: ParamKind) -> Self {
        Self {
            name: name.into(),
            kind,
            required: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InputSchema {
    pub params: Vec<ParamSchema>,
}

impl InputSchema {
    pub fn new(params: Vec<ParamSchema>) -> Self {
        Self { params }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    MissingRequired(String),
    WrongType { param: String, expected: ParamKind },
    UnexpectedParam(String),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingRequired(name) => write!(f, "missing required parameter {name:?}"),
            Self::WrongType { param, expected } => {
                write!(f, "parameter {param:?} should be {expected:?}")
            }
            Self::UnexpectedParam(name) => write!(f, "unexpected parameter {name:?}"),
        }
    }
}

impl InputSchema {
    /// TCLIS SS7: schema validation, type validation, required-parameter
    /// validation, all in one pass. Returns every problem found, not
    /// just the first, so a caller can report everything wrong at once.
    pub fn validate(&self, args: &ToolArgs) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        for param in &self.params {
            match args.get(&param.name) {
                None if param.required => {
                    errors.push(ValidationError::MissingRequired(param.name.clone()));
                }
                Some(value) if !param.kind.matches(value) => {
                    errors.push(ValidationError::WrongType {
                        param: param.name.clone(),
                        expected: param.kind,
                    });
                }
                _ => {}
            }
        }

        let known: HashSet<&str> = self.params.iter().map(|p| p.name.as_str()).collect();
        for key in args.keys() {
            if !known.contains(key.as_str()) {
                errors.push(ValidationError::UnexpectedParam(key.clone()));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("text", ParamKind::String),
            ParamSchema::optional("times", ParamKind::Integer),
        ])
    }

    #[test]
    fn valid_args_pass() {
        let mut args = ToolArgs::new();
        args.insert("text".into(), json!("hello"));
        assert!(schema().validate(&args).is_ok());
    }

    #[test]
    fn missing_required_param_fails() {
        let args = ToolArgs::new();
        let errors = schema().validate(&args).unwrap_err();
        assert_eq!(
            errors,
            vec![ValidationError::MissingRequired("text".into())]
        );
    }

    #[test]
    fn wrong_type_fails() {
        let mut args = ToolArgs::new();
        args.insert("text".into(), json!(42));
        let errors = schema().validate(&args).unwrap_err();
        assert_eq!(
            errors,
            vec![ValidationError::WrongType {
                param: "text".into(),
                expected: ParamKind::String
            }]
        );
    }

    #[test]
    fn unexpected_param_fails() {
        let mut args = ToolArgs::new();
        args.insert("text".into(), json!("hi"));
        args.insert("extra".into(), json!(true));
        let errors = schema().validate(&args).unwrap_err();
        assert_eq!(
            errors,
            vec![ValidationError::UnexpectedParam("extra".into())]
        );
    }

    #[test]
    fn missing_optional_param_is_fine() {
        let mut args = ToolArgs::new();
        args.insert("text".into(), json!("hi"));
        assert!(schema().validate(&args).is_ok());
    }

    #[test]
    fn integer_satisfies_a_float_param() {
        let schema = InputSchema::new(vec![ParamSchema::required("n", ParamKind::Float)]);
        let mut args = ToolArgs::new();
        args.insert("n".into(), json!(3));
        assert!(schema.validate(&args).is_ok());
    }

    #[test]
    fn reports_every_error_not_just_the_first() {
        let mut args = ToolArgs::new();
        args.insert("times".into(), json!("not a number"));
        args.insert("bogus".into(), json!(1));
        let errors = schema().validate(&args).unwrap_err();
        assert_eq!(errors.len(), 3); // missing text, wrong-type times, unexpected bogus
    }

    #[test]
    fn string_array_accepts_arrays_of_strings_only() {
        let schema = InputSchema::new(vec![ParamSchema::required("items", ParamKind::StringArray)]);

        let mut ok_args = ToolArgs::new();
        ok_args.insert("items".into(), json!(["a", "b", "c"]));
        assert!(schema.validate(&ok_args).is_ok());

        let mut bad_args = ToolArgs::new();
        bad_args.insert("items".into(), json!(["a", 2, "c"]));
        assert!(schema.validate(&bad_args).is_err());

        let mut not_array_args = ToolArgs::new();
        not_array_args.insert("items".into(), json!("not an array"));
        assert!(schema.validate(&not_array_args).is_err());
    }
}
