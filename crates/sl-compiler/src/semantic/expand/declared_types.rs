use sl_core::{Form, ScriptLangError};

use super::raw_body_text;
use crate::semantic::attr;
use crate::semantic::env::ExpandEnv;
use crate::semantic::error_at;
use crate::semantic::required_attr;
use crate::semantic::types::DeclaredType;

pub(crate) fn expand_const_form(form: &Form, env: &mut ExpandEnv) -> Result<Form, ScriptLangError> {
    if let Some(name) = attr(form, "name").map(str::to_string) {
        let exported = !is_private(form)?;
        if !env.declare_const(name.clone(), exported) {
            let module_name = env.module.module_name.as_deref().unwrap_or("<unknown>");
            return Err(error_at(
                form,
                format!("duplicate const declaration `{module_name}.{name}`"),
            ));
        }
        env.add_const_decl(name, parse_declared_type_form(form)?, raw_body_text(form));
    } else {
        let _ = required_attr(form, "name")?;
    }
    Ok(form.clone())
}

pub(crate) fn parse_declared_type_form(form: &Form) -> Result<DeclaredType, ScriptLangError> {
    parse_declared_type_name(attr(form, "type"), &form.head, |message| {
        form_error(form, message)
    })
}

pub(crate) fn parse_declared_type_name(
    type_name: Option<&str>,
    head: &str,
    error: impl FnOnce(String) -> ScriptLangError,
) -> Result<DeclaredType, ScriptLangError> {
    match type_name {
        None => Err(error(format!("<{}> requires `type`", head))),
        Some("array") => Ok(DeclaredType::Array),
        Some("bool") => Ok(DeclaredType::Bool),
        Some("function") => Ok(DeclaredType::Function),
        Some("int") => Ok(DeclaredType::Int),
        Some("object") => Ok(DeclaredType::Object),
        Some("script") => Ok(DeclaredType::Script),
        Some("string") => Ok(DeclaredType::String),
        Some(other) => Err(error(format!("unsupported type `{other}` in MVP"))),
    }
}

fn form_error(form: &Form, message: impl Into<String>) -> ScriptLangError {
    error_at(form, message)
}

fn invalid_bool_attr_error(form: &Form, attr_name: &str, value: &str) -> ScriptLangError {
    error_at(
        form,
        format!("invalid boolean value `{value}` for `{attr_name}`"),
    )
}

/// Check if a form is marked as private.
pub(crate) fn is_private(form: &Form) -> Result<bool, ScriptLangError> {
    match attr(form, "private") {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(invalid_bool_attr_error(form, "private", other)),
    }
}

/// Check if a form is marked as hidden.
pub(crate) fn is_hidden(form: &Form) -> Result<bool, ScriptLangError> {
    match attr(form, "hidden") {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(invalid_bool_attr_error(form, "hidden", other)),
    }
}

#[cfg(test)]
mod tests {
    use sl_core::{FormField, FormValue};

    use super::*;
    use crate::semantic::env::ExpandEnv;
    use crate::semantic::expand::test_helpers::{children, form, text};

    fn attr(name: &str, value: &str) -> FormField {
        FormField {
            name: name.to_string(),
            value: FormValue::String(value.to_string()),
        }
    }

    #[test]
    fn parse_declared_type_name_covers_supported_and_error_paths() {
        assert_eq!(
            parse_declared_type_name(Some("int"), "var", ScriptLangError::message).expect("int"),
            DeclaredType::Int
        );
        assert_eq!(
            parse_declared_type_name(Some("array"), "var", ScriptLangError::message)
                .expect("array"),
            DeclaredType::Array
        );
        assert_eq!(
            parse_declared_type_name(Some("object"), "var", ScriptLangError::message)
                .expect("object"),
            DeclaredType::Object
        );
        assert_eq!(
            parse_declared_type_name(Some("function"), "var", ScriptLangError::message)
                .expect("function"),
            DeclaredType::Function
        );
        assert!(
            parse_declared_type_name(None, "var", ScriptLangError::message)
                .expect_err("missing")
                .to_string()
                .contains("<var> requires `type`")
        );
        assert!(
            parse_declared_type_name(Some("number"), "var", ScriptLangError::message)
                .expect_err("unsupported")
                .to_string()
                .contains("unsupported type `number`")
        );
    }

    #[test]
    fn expand_const_form_tracks_decl_and_rejects_invalid_cases() {
        let mut env = ExpandEnv::default();
        env.begin_module(Some("main".to_string()), None)
            .expect("module");

        let const_decl = form(
            "const",
            vec![
                attr("name", "answer"),
                attr("type", "int"),
                attr("private", "true"),
                children(vec![text("42")]),
            ],
        );
        expand_const_form(&const_decl, &mut env).expect("const");
        assert!(env.module.exports.consts.contains_declared("answer"));
        assert!(!env.module.exports.consts.contains_exported("answer"));
        assert_eq!(
            env.module
                .const_decls
                .get("answer")
                .expect("decl")
                .raw_expr
                .as_deref(),
            Some("42")
        );

        let duplicate = expand_const_form(&const_decl, &mut env).expect_err("duplicate");
        assert!(
            duplicate
                .to_string()
                .contains("duplicate const declaration")
        );

        let invalid_private = form(
            "const",
            vec![
                attr("name", "bad"),
                attr("type", "int"),
                attr("private", "maybe"),
                children(vec![text("1")]),
            ],
        );
        assert!(
            expand_const_form(&invalid_private, &mut env)
                .expect_err("private")
                .to_string()
                .contains("invalid boolean value `maybe`")
        );

        let missing_name = form(
            "const",
            vec![attr("type", "int"), children(vec![text("1")])],
        );
        assert!(
            expand_const_form(&missing_name, &mut env)
                .expect_err("name")
                .to_string()
                .contains("<const> requires `name`")
        );
    }
}
