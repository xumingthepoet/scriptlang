use std::collections::BTreeSet;

use sl_core::{ScriptLangError, TextSegment, TextTemplate};

use super::const_eval::{ConstEnv, ConstLookup};
use super::resolve::{ModuleCatalog, QualifiedConstLookup, ScopeResolver};
use crate::names::resolved_var_placeholder;

pub(crate) fn rewrite_expr_with_consts<R: ConstLookup>(
    source: &str,
    local_env: &ConstEnv,
    resolver: &mut R,
    blocked_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    let mut rewritten = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if ch == '"' || ch == '\'' {
            let end = scan_quoted(bytes, cursor)?;
            rewritten.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }
        if is_ident_start(ch) {
            let (end, segments) = scan_reference_path(source, cursor);
            let raw = &source[cursor..end];
            let first = segments[0].as_str();

            if shadowed_names.contains(first) || is_property_access(bytes, cursor) {
                rewritten.push_str(raw);
                cursor = end;
                continue;
            }

            if segments.len() == 1 {
                let ident = first;
                if is_map_key(source, end) {
                    rewritten.push_str(ident);
                } else if let Some(value) = local_env.get(ident) {
                    rewritten.push_str(&value.to_rhai_literal());
                } else if let Some(value) = resolver.resolve_short_const(ident)? {
                    rewritten.push_str(&value.to_rhai_literal());
                } else if blocked_names.contains(ident) {
                    return Err(ScriptLangError::message(format!(
                        "const `{ident}` cannot be referenced before it is defined"
                    )));
                } else {
                    rewritten.push_str(ident);
                }
            } else {
                let module_path = segments[..segments.len() - 1].join(".");
                let name = segments.last().expect("qualified path");
                if module_path == resolver.current_module() {
                    if let Some(value) = local_env.get(name) {
                        rewritten.push_str(&value.to_rhai_literal());
                    } else {
                        rewritten.push_str(raw);
                    }
                } else {
                    match resolver.resolve_qualified_const(&module_path, name)? {
                        QualifiedConstLookup::Value(value) => {
                            rewritten.push_str(&value.to_rhai_literal());
                        }
                        QualifiedConstLookup::HiddenModule
                        | QualifiedConstLookup::UnknownConst
                        | QualifiedConstLookup::NotModulePath => rewritten.push_str(raw),
                    }
                }
            }

            cursor = end;
            continue;
        }
        rewritten.push(ch);
        cursor += ch.len_utf8();
    }

    Ok(rewritten)
}

pub(crate) fn rewrite_template_with_consts<R: ConstLookup>(
    template: TextTemplate,
    local_env: &ConstEnv,
    resolver: &mut R,
    blocked_names: &BTreeSet<String>,
    shadowed_names: &BTreeSet<String>,
) -> Result<TextTemplate, ScriptLangError> {
    let segments = template
        .segments
        .into_iter()
        .map(|segment| match segment {
            TextSegment::Literal(text) => Ok(TextSegment::Literal(text)),
            TextSegment::Expr(expr) => Ok(TextSegment::Expr(rewrite_expr_with_consts(
                &expr,
                local_env,
                resolver,
                blocked_names,
                shadowed_names,
            )?)),
        })
        .collect::<Result<Vec<_>, ScriptLangError>>()?;
    Ok(TextTemplate { segments })
}

pub(crate) fn rewrite_expr_with_vars(
    source: &str,
    resolver: &ScopeResolver<'_, '_>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    let mut rewritten = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if ch == '"' || ch == '\'' {
            let end = scan_quoted(bytes, cursor)?;
            rewritten.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }

        if is_ident_start(ch) {
            let (end, segments) = scan_reference_path(source, cursor);
            let raw = &source[cursor..end];
            let first = segments[0].as_str();

            if shadowed_names.contains(first) || is_property_access(bytes, cursor) {
                rewritten.push_str(raw);
                cursor = end;
                continue;
            }

            let resolved = if segments.len() == 1 {
                resolver.resolve_short_var_ref(first)?
            } else {
                let module_path = segments[..segments.len() - 1].join(".");
                let name = segments.last().expect("qualified path");
                resolver.resolve_qualified_var_ref(&module_path, name)?
            };

            if let Some(target) = resolved {
                if is_map_key(source, end) {
                    rewritten.push_str(raw);
                } else {
                    rewritten.push_str(&resolved_var_placeholder(&target.qualified_name()));
                }
            } else {
                rewritten.push_str(raw);
            }
            cursor = end;
            continue;
        }

        rewritten.push(ch);
        cursor += ch.len_utf8();
    }

    Ok(rewritten)
}

pub(crate) fn rewrite_template_with_vars(
    template: TextTemplate,
    resolver: &ScopeResolver<'_, '_>,
    shadowed_names: &BTreeSet<String>,
) -> Result<TextTemplate, ScriptLangError> {
    let segments = template
        .segments
        .into_iter()
        .map(|segment| match segment {
            TextSegment::Literal(text) => Ok(TextSegment::Literal(text)),
            TextSegment::Expr(expr) => Ok(TextSegment::Expr(rewrite_expr_with_vars(
                &expr,
                resolver,
                shadowed_names,
            )?)),
        })
        .collect::<Result<Vec<_>, ScriptLangError>>()?;
    Ok(TextTemplate { segments })
}

pub(crate) fn rewrite_script_literals(
    source: &str,
    current_module: &str,
    modules: &ModuleCatalog<'_>,
) -> Result<String, ScriptLangError> {
    let mut rewritten = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if ch == '"' || ch == '\'' {
            let end = scan_quoted(bytes, cursor)?;
            rewritten.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }

        if ch == '@' {
            let start = cursor;
            cursor += 1;
            if cursor >= bytes.len() || !is_ident_start(bytes[cursor] as char) {
                return Err(ScriptLangError::message(format!(
                    "invalid script literal `{}`",
                    &source[start..cursor]
                )));
            }
            while cursor < bytes.len() {
                let current = bytes[cursor] as char;
                if is_ident_continue(current) || current == '.' {
                    cursor += 1;
                } else {
                    break;
                }
            }
            let raw = &source[start..cursor];
            let qualified = modules.resolve_script_literal(current_module, raw)?;
            rewritten.push_str(&format!("{qualified:?}"));
            continue;
        }

        rewritten.push(ch);
        cursor += ch.len_utf8();
    }

    Ok(rewritten)
}

pub(crate) fn scan_quoted(bytes: &[u8], start: usize) -> Result<usize, ScriptLangError> {
    let quote = bytes[start];
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            ch if ch == quote => return Ok(cursor + 1),
            _ => cursor += 1,
        }
    }
    Err(ScriptLangError::message("unterminated string literal"))
}

pub(crate) fn scan_reference_path(source: &str, start: usize) -> (usize, Vec<String>) {
    let mut cursor = start;
    let mut segments = Vec::new();
    loop {
        let ident_start = cursor;
        cursor += 1;
        let bytes = source.as_bytes();
        while cursor < bytes.len() && is_ident_continue(bytes[cursor] as char) {
            cursor += 1;
        }
        segments.push(source[ident_start..cursor].to_string());
        if cursor >= bytes.len() || bytes[cursor] != b'.' {
            break;
        }
        let next = cursor + 1;
        if next >= bytes.len() || !is_ident_start(bytes[next] as char) {
            break;
        }
        cursor = next;
    }
    (cursor, segments)
}

pub(crate) fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

pub(crate) fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn is_property_access(bytes: &[u8], ident_start: usize) -> bool {
    let mut cursor = ident_start;
    while cursor > 0 {
        cursor -= 1;
        let ch = bytes[cursor] as char;
        if ch.is_whitespace() {
            continue;
        }
        return ch == '.';
    }
    false
}

fn is_map_key(source: &str, ident_end: usize) -> bool {
    let mut chars = source[ident_end..].chars();
    loop {
        match chars.next() {
            Some(ch) if ch.is_whitespace() => continue,
            Some(':') => return true,
            _ => return false,
        }
    }
}
