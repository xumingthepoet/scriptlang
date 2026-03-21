use sl_core::{Form, ScriptLangError};

use super::{raw_body_text, string_attr};
use crate::semantic::env::ExpandEnv;
use crate::semantic::error_at;
use crate::semantic::required_attr;
use crate::semantic::types::DeclaredType;

pub(crate) fn expand_const_form(form: &Form, env: &mut ExpandEnv) -> Result<Form, ScriptLangError> {
    if let Some(name) = string_attr(form, "name").map(str::to_string) {
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
    parse_declared_type_name(string_attr(form, "type"), &form.head, |message| {
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

fn is_private(form: &Form) -> Result<bool, ScriptLangError> {
    match string_attr(form, "private") {
        None => Ok(false),
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(error_at(
            form,
            format!("invalid boolean value `{other}` for `private`"),
        )),
    }
}
