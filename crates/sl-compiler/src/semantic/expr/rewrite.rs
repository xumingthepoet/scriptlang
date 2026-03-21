use std::collections::{BTreeMap, BTreeSet};

use sl_core::{ScriptLangError, TextSegment, TextTemplate};

use super::scan::{is_ident_start, scan_expr_source, scan_quoted, scan_reference_path};
use super::{ExprKind, SpecialTokenKind};
use crate::names::resolved_var_placeholder;
use crate::semantic::expand::{ConstEnv, ConstLookup, QualifiedConstLookup, ScopeResolver};

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

pub(crate) fn rewrite_expr_function_calls(
    source: &str,
    resolver: &ScopeResolver<'_, '_>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    rewrite_expr_function_calls_inner(source, resolver, shadowed_names)
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

pub(crate) fn rewrite_special_literals(
    source: &str,
    resolver: &mut impl ConstLookup,
) -> Result<String, ScriptLangError> {
    let expr = scan_expr_source(source, ExprKind::Rhai)?;
    let mut rewritten = String::with_capacity(source.len());
    let mut cursor = 0usize;
    for token in expr.tokens {
        rewritten.push_str(&source[cursor..token.start]);
        let raw = &source[token.start..token.end];
        match token.kind {
            SpecialTokenKind::ScriptLiteral => {
                let qualified = resolver.resolve_script_literal(raw)?;
                rewritten.push_str(&format!("{qualified:?}"));
            }
            SpecialTokenKind::FunctionLiteral => {
                let qualified = resolver.resolve_function_literal(raw)?;
                rewritten.push_str(&format!("{qualified:?}"));
            }
            _ => {
                rewritten.push_str(raw);
            }
        }
        cursor = token.end;
    }
    rewritten.push_str(&source[cursor..]);
    Ok(rewritten)
}

pub(crate) fn rewrite_template_special_literals(
    template: TextTemplate,
    resolver: &mut impl ConstLookup,
) -> Result<TextTemplate, ScriptLangError> {
    let segments = template
        .segments
        .into_iter()
        .map(|segment| match segment {
            TextSegment::Literal(text) => Ok(TextSegment::Literal(text)),
            TextSegment::Expr(expr) => Ok(TextSegment::Expr(rewrite_special_literals(
                &expr, resolver,
            )?)),
        })
        .collect::<Result<Vec<_>, ScriptLangError>>()?;
    Ok(TextTemplate { segments })
}

pub(crate) fn rewrite_expr_idents(
    source: &str,
    renames: &BTreeMap<String, String>,
) -> Result<String, ScriptLangError> {
    if renames.is_empty() {
        return Ok(source.to_string());
    }

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

            if is_property_access(bytes, cursor) || is_map_key(source, end) || segments.len() > 1 {
                rewritten.push_str(raw);
            } else if let Some(replacement) = renames.get(first) {
                rewritten.push_str(replacement);
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

fn rewrite_expr_function_calls_inner(
    source: &str,
    resolver: &ScopeResolver<'_, '_>,
    shadowed_names: &BTreeSet<String>,
) -> Result<String, ScriptLangError> {
    let bytes = source.as_bytes();
    let mut rewritten = String::with_capacity(source.len());
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
            let ident_start = cursor;
            let (end, segments) = scan_reference_path(source, cursor);
            let raw = &source[ident_start..end];
            let first = segments[0].as_str();

            let mut open = end;
            while open < bytes.len() && (bytes[open] as char).is_whitespace() {
                open += 1;
            }
            if open < bytes.len() && bytes[open] == b'(' {
                let close = scan_matching_paren(bytes, open)?;
                let inner = &source[open + 1..close - 1];
                let rewritten_inner =
                    rewrite_expr_function_calls_inner(inner, resolver, shadowed_names)?;
                let resolved =
                    if shadowed_names.contains(first) || is_property_access(bytes, ident_start) {
                        None
                    } else if segments.len() == 1 {
                        resolver.resolve_short_function_ref(first)?
                    } else {
                        let module_path = segments[..segments.len() - 1].join(".");
                        let name = segments.last().expect("qualified path");
                        resolver.resolve_qualified_function_ref(&module_path, name)?
                    };

                if let Some(target) = resolved {
                    rewritten.push_str(&format!("__sl_call({target:?}, [{rewritten_inner}])"));
                } else {
                    rewritten.push_str(raw);
                    rewritten.push_str(&source[end..open]);
                    rewritten.push('(');
                    rewritten.push_str(&rewritten_inner);
                    rewritten.push(')');
                }
                cursor = close;
                continue;
            }
        }

        rewritten.push(ch);
        cursor += ch.len_utf8();
    }

    Ok(rewritten)
}

fn scan_matching_paren(bytes: &[u8], open: usize) -> Result<usize, ScriptLangError> {
    let mut cursor = open + 1;
    let mut depth = 1usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' | b'\'' => cursor = scan_quoted(bytes, cursor)?,
            b'(' => {
                depth += 1;
                cursor += 1;
            }
            b')' => {
                depth -= 1;
                cursor += 1;
                if depth == 0 {
                    return Ok(cursor);
                }
            }
            _ => cursor += 1,
        }
    }
    Err(ScriptLangError::message("unterminated function call"))
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use sl_core::{ScriptLangError, TextSegment, TextTemplate};

    use super::*;
    use crate::semantic::expand::{ConstValue, QualifiedConstLookup};

    struct TestResolver {
        current_module: String,
        imported_short_env: BTreeMap<String, ConstValue>,
        visible_modules: BTreeMap<String, ConstEnv>,
    }

    impl ConstLookup for TestResolver {
        fn current_module(&self) -> &str {
            &self.current_module
        }

        fn resolve_short_const(
            &mut self,
            name: &str,
        ) -> Result<Option<ConstValue>, ScriptLangError> {
            Ok(self.imported_short_env.get(name).cloned())
        }

        fn resolve_qualified_const(
            &mut self,
            module_path: &str,
            name: &str,
        ) -> Result<QualifiedConstLookup, ScriptLangError> {
            if let Some(module_env) = self.visible_modules.get(module_path) {
                if let Some(value) = module_env.get(name) {
                    Ok(QualifiedConstLookup::Value(value.clone()))
                } else {
                    Ok(QualifiedConstLookup::UnknownConst)
                }
            } else {
                Ok(QualifiedConstLookup::NotModulePath)
            }
        }

        fn resolve_function_literal(&mut self, raw: &str) -> Result<String, ScriptLangError> {
            let raw = raw.strip_prefix('#').expect("function literal");
            Ok(if raw.contains('.') {
                raw.to_string()
            } else {
                format!("{}.{}", self.current_module, raw)
            })
        }

        fn resolve_script_literal(&mut self, raw: &str) -> Result<String, ScriptLangError> {
            let raw = raw.strip_prefix('@').expect("script literal");
            Ok(if raw.contains('.') {
                raw.to_string()
            } else {
                format!("{}.{}", self.current_module, raw)
            })
        }
    }

    fn resolver() -> TestResolver {
        TestResolver {
            current_module: "main".to_string(),
            imported_short_env: BTreeMap::from([("answer".to_string(), ConstValue::Integer(42))]),
            visible_modules: BTreeMap::from([(
                "helper".to_string(),
                BTreeMap::from([("zero".to_string(), ConstValue::Integer(0))]),
            )]),
        }
    }

    #[test]
    fn rewrite_special_literals_rewrites_scripts_and_functions_but_skips_strings() {
        let mut visible = resolver();
        let rewritten = rewrite_special_literals(
            r##"#pick + @loop + "#{ignored}" + '@skip' + #helper.pick"##,
            &mut visible,
        )
        .expect("rewrite");

        assert_eq!(
            rewritten,
            r##""main.pick" + "main.loop" + "#{ignored}" + '@skip' + "helper.pick""##
        );
    }

    #[test]
    fn rewrite_template_special_literals_only_rewrites_expr_segments() {
        let mut visible = resolver();
        let template = rewrite_template_special_literals(
            TextTemplate {
                segments: vec![
                    TextSegment::Literal("x=".to_string()),
                    TextSegment::Expr("#pick".to_string()),
                    TextSegment::Literal(", y=".to_string()),
                    TextSegment::Expr("@loop".to_string()),
                ],
            },
            &mut visible,
        )
        .expect("template rewrite");

        assert!(matches!(
            &template.segments[0],
            TextSegment::Literal(text) if text == "x="
        ));
        assert!(matches!(
            &template.segments[1],
            TextSegment::Expr(expr) if expr == "\"main.pick\""
        ));
        assert!(matches!(
            &template.segments[3],
            TextSegment::Expr(expr) if expr == "\"main.loop\""
        ));
    }

    #[test]
    fn rewrite_expr_with_consts_covers_blocked_names_and_map_keys() {
        let mut visible = resolver();
        let local_env =
            BTreeMap::from([("name".to_string(), ConstValue::String("neo".to_string()))]);
        let blocked = BTreeSet::from(["later".to_string()]);

        let rewritten = rewrite_expr_with_consts(
            "name + answer + helper.zero + #{answer: answer, later: later}",
            &local_env,
            &mut visible,
            &BTreeSet::new(),
            &BTreeSet::new(),
        )
        .expect("rewrite with map keys");
        assert_eq!(rewritten, "\"neo\" + 42 + 0 + #{answer: 42, later: later}");

        let blocked_error = rewrite_expr_with_consts(
            "later",
            &BTreeMap::new(),
            &mut visible,
            &blocked,
            &BTreeSet::new(),
        )
        .expect_err("blocked");
        assert!(
            blocked_error
                .to_string()
                .contains("cannot be referenced before it is defined")
        );
    }

    #[test]
    fn rewrite_expr_idents_skips_property_access_and_qualified_refs() {
        let rewritten = rewrite_expr_idents(
            r##"temp + obj.temp + helper.temp + "#{temp}""##,
            &BTreeMap::from([("temp".to_string(), "temp__1".to_string())]),
        )
        .expect("rename");

        assert_eq!(
            rewritten,
            r##"temp__1 + obj.temp + helper.temp + "#{temp}""##
        );
    }
}
