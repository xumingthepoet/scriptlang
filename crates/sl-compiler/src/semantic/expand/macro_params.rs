use std::collections::BTreeMap;

use sl_core::{Form, FormItem, ScriptLangError};

use super::macro_env::MacroEnv;
use super::macro_values::MacroValue;
use crate::semantic::env::{
    ExpandEnv, LegacyProtocol, MacroDefinition, MacroParam, MacroParamType,
};
use crate::semantic::error_at;

/// Bind macro invocation arguments to parameters and populate MacroEnv.
pub(super) fn bind_macro_params(
    definition: &MacroDefinition,
    invocation: &Form,
    expand_env: &mut ExpandEnv,
) -> Result<MacroEnv, ScriptLangError> {
    // Extract invocation attributes and content
    let invocation_attrs = extract_invocation_attributes(invocation)?;
    let invocation_content = extract_invocation_content(invocation);

    // Handle params vs legacy protocol
    if let Some(ref params) = definition.params {
        // New explicit params protocol
        bind_explicit_params(
            params,
            &invocation_attrs,
            &invocation_content,
            invocation,
            expand_env,
        )
    } else if let Some(ref legacy) = definition.legacy_protocol {
        // Legacy attributes/content protocol
        bind_legacy_protocol(legacy, &invocation_attrs, &invocation_content, expand_env)
    } else {
        // No params, just create basic MacroEnv
        Ok(MacroEnv::from_invocation(
            expand_env,
            &definition.name,
            invocation_attrs,
            invocation_content,
        ))
    }
}

/// Extract attributes from macro invocation form.
fn extract_invocation_attributes(
    invocation: &Form,
) -> Result<BTreeMap<String, String>, ScriptLangError> {
    let mut attrs = BTreeMap::new();
    for field in &invocation.fields {
        if field.name == "children" {
            continue;
        }
        match &field.value {
            sl_core::FormValue::String(value) => {
                attrs.insert(field.name.clone(), value.clone());
            }
            sl_core::FormValue::Sequence(_) => {
                // Skip sequence fields (only children typically)
            }
        }
    }
    Ok(attrs)
}

/// Extract content (children) from macro invocation form.
fn extract_invocation_content(invocation: &Form) -> Vec<FormItem> {
    invocation
        .fields
        .iter()
        .find_map(|field| match (&field.name[..], &field.value) {
            ("children", sl_core::FormValue::Sequence(items)) => Some(items.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Bind parameters using new explicit protocol.
fn bind_explicit_params(
    params: &[MacroParam],
    invocation_attrs: &BTreeMap<String, String>,
    invocation_content: &[FormItem],
    invocation: &Form,
    expand_env: &mut ExpandEnv,
) -> Result<MacroEnv, ScriptLangError> {
    let mut macro_env = MacroEnv::from_invocation(
        expand_env,
        "",              // Will be set by caller
        BTreeMap::new(), // We'll populate locals directly
        Vec::new(),      // We'll populate content directly
    );

    // Track which invocation attrs have been used
    let mut used_attrs = std::collections::HashSet::new();
    let mut keyword_args = Vec::new();

    for param in params {
        match param.param_type {
            MacroParamType::Keyword => {
                // Collect all unused attributes as keyword arguments
                // This is handled after the loop
            }
            MacroParamType::Ast => {
                // Content parameter
                let content = invocation_content.to_vec();
                macro_env
                    .locals
                    .insert(param.name.clone(), MacroValue::AstItems(content));
            }
            _ => {
                // Regular attribute parameter
                let attr_value = invocation_attrs.get(&param.name);
                used_attrs.insert(param.name.clone());

                let value =
                    convert_param_value(&param.param_type, attr_value, &param.name, invocation)?;

                macro_env.locals.insert(param.name.clone(), value);
            }
        }
    }

    // Collect remaining attributes as keyword arguments (for keyword param)
    let has_keyword_param = params
        .iter()
        .any(|p| p.param_type == MacroParamType::Keyword);
    if has_keyword_param {
        let keyword_param_name = params
            .iter()
            .find(|p| p.param_type == MacroParamType::Keyword)
            .unwrap()
            .name
            .clone();

        for (attr_name, attr_value) in invocation_attrs {
            if !used_attrs.contains(attr_name) {
                // Parse as expression for keyword values
                let value = MacroValue::String(format!("{}:{}", attr_name, attr_value));
                keyword_args.push((attr_name.clone(), value));
            }
        }

        macro_env
            .locals
            .insert(keyword_param_name, MacroValue::Keyword(keyword_args));
    }

    // Set macro name (should be overridden by caller)
    // For now, we'll need to get it from somewhere

    Ok(macro_env)
}

/// Convert parameter value based on type.
fn convert_param_value(
    param_type: &MacroParamType,
    attr_value: Option<&String>,
    param_name: &str,
    invocation: &Form,
) -> Result<MacroValue, ScriptLangError> {
    let value_str = match attr_value {
        Some(s) => s,
        None => {
            return Err(error_at(
                invocation,
                format!("missing required parameter `{}`", param_name),
            ));
        }
    };

    match param_type {
        MacroParamType::Expr => {
            // Expression source - keep as string
            Ok(MacroValue::String(value_str.clone()))
        }
        MacroParamType::Ast => {
            // AST - should not reach here (handled in bind_explicit_params)
            Err(error_at(
                invocation,
                format!(
                    "internal error: AST parameter `{}` should be handled separately",
                    param_name
                ),
            ))
        }
        MacroParamType::String => {
            // String value - keep as is
            Ok(MacroValue::String(value_str.clone()))
        }
        MacroParamType::Bool => {
            // Boolean value - parse string
            match value_str.as_str() {
                "true" => Ok(MacroValue::Bool(true)),
                "false" => Ok(MacroValue::Bool(false)),
                _ => Err(error_at(
                    invocation,
                    format!(
                        "parameter `{}` expected bool, got `{}`",
                        param_name, value_str
                    ),
                )),
            }
        }
        MacroParamType::Int => {
            // Integer value - parse string
            value_str.parse::<i64>().map(MacroValue::Int).map_err(|_| {
                error_at(
                    invocation,
                    format!(
                        "parameter `{}` expected int, got `{}`",
                        param_name, value_str
                    ),
                )
            })
        }
        MacroParamType::Keyword => {
            // Keyword - should not reach here (handled in bind_explicit_params)
            Err(error_at(
                invocation,
                format!(
                    "internal error: keyword parameter `{}` should be handled separately",
                    param_name
                ),
            ))
        }
        MacroParamType::Module => {
            // Module reference - keep as string for now
            // Will be expanded/resolved later
            Ok(MacroValue::String(value_str.clone()))
        }
    }
}

/// Bind parameters using legacy protocol.
fn bind_legacy_protocol(
    legacy: &LegacyProtocol,
    invocation_attrs: &BTreeMap<String, String>,
    invocation_content: &[FormItem],
    expand_env: &mut ExpandEnv,
) -> Result<MacroEnv, ScriptLangError> {
    let mut macro_env = MacroEnv::from_invocation(
        expand_env,
        "", // Will be set by caller
        invocation_attrs.clone(),
        invocation_content.to_vec(),
    );

    // Bind attributes to locals
    for (attr_name, var_name, is_expr) in &legacy.attributes {
        let attr_value = invocation_attrs.get(attr_name);
        let value = if *is_expr {
            // Expression attribute - keep as string
            match attr_value {
                Some(s) => MacroValue::String(s.clone()),
                None => MacroValue::Nil,
            }
        } else {
            // Plain attribute
            match attr_value {
                Some(s) => MacroValue::String(s.clone()),
                None => MacroValue::Nil,
            }
        };
        macro_env.locals.insert(var_name.clone(), value);
    }

    // Bind content to locals
    if let Some((var_name, head_filter)) = &legacy.content {
        let content = match head_filter {
            Some(head) => invocation_content
                .iter()
                .filter(|item| matches!(item, FormItem::Form(form) if form.head == head.as_str()))
                .cloned()
                .collect(),
            None => invocation_content.to_vec(),
        };
        macro_env
            .locals
            .insert(var_name.clone(), MacroValue::AstItems(content));
    }

    Ok(macro_env)
}

#[cfg(test)]
mod tests {
    use sl_core::{FormField, FormMeta, FormValue, SourcePosition};

    use super::*;

    fn meta() -> FormMeta {
        FormMeta {
            source_name: Some("test.xml".to_string()),
            start: SourcePosition { row: 1, column: 1 },
            end: SourcePosition { row: 1, column: 10 },
            start_byte: 0,
            end_byte: 10,
        }
    }

    fn make_form(head: &str, attrs: Vec<(&str, &str)>) -> Form {
        Form {
            head: head.to_string(),
            fields: attrs
                .into_iter()
                .map(|(name, value)| FormField {
                    name: name.to_string(),
                    value: FormValue::String(value.to_string()),
                })
                .collect(),
            meta: meta(),
        }
    }

    #[test]
    fn test_extract_invocation_attributes() {
        let form = make_form("test", vec![("name", "value"), ("other", "123")]);
        let attrs = extract_invocation_attributes(&form).unwrap();
        assert_eq!(attrs.get("name"), Some(&"value".to_string()));
        assert_eq!(attrs.get("other"), Some(&"123".to_string()));
    }

    #[test]
    fn test_convert_param_value_bool() {
        let invocation = make_form("test", vec![]);
        let value = convert_param_value(
            &MacroParamType::Bool,
            Some(&"true".to_string()),
            "flag",
            &invocation,
        )
        .unwrap();
        assert_eq!(value, MacroValue::Bool(true));

        let value = convert_param_value(
            &MacroParamType::Bool,
            Some(&"false".to_string()),
            "flag",
            &invocation,
        )
        .unwrap();
        assert_eq!(value, MacroValue::Bool(false));
    }

    #[test]
    fn test_convert_param_value_int() {
        let invocation = make_form("test", vec![]);
        let value = convert_param_value(
            &MacroParamType::Int,
            Some(&"42".to_string()),
            "num",
            &invocation,
        )
        .unwrap();
        assert_eq!(value, MacroValue::Int(42));
    }

    #[test]
    fn test_convert_param_value_missing() {
        let invocation = make_form("test", vec![]);
        let result = convert_param_value(&MacroParamType::String, None, "name", &invocation);
        assert!(result.is_err());
    }
}
