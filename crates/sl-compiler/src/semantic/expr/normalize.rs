use sl_core::ScriptLangError;

pub(crate) fn normalize_expr_escapes(source: &str) -> Result<String, ScriptLangError> {
    let chars = source.chars().collect::<Vec<_>>();
    let mut out = String::with_capacity(source.len());
    let mut index = 0usize;

    while index < chars.len() {
        match chars[index] {
            '\'' => {
                let (encoded, next_index) = parse_single_quoted_string(&chars, index)?;
                out.push_str(&encoded);
                index = next_index;
            }
            '"' => {
                out.push('"');
                index += 1;
                while index < chars.len() {
                    let ch = chars[index];
                    out.push(ch);
                    index += 1;
                    if ch == '\\' {
                        if index < chars.len() {
                            out.push(chars[index]);
                            index += 1;
                        }
                        continue;
                    }
                    if ch == '"' {
                        break;
                    }
                }
            }
            ch if is_expr_token_char(ch) => {
                let start = index;
                index += 1;
                while index < chars.len() && is_expr_token_char(chars[index]) {
                    index += 1;
                }
                let token = chars[start..index].iter().collect::<String>();
                match token.as_str() {
                    "LTE" => out.push_str("<="),
                    "LT" => out.push('<'),
                    "AND" => out.push_str("&&"),
                    _ => out.push_str(&token),
                }
            }
            ch => {
                out.push(ch);
                index += 1;
            }
        }
    }

    Ok(out)
}

fn parse_single_quoted_string(
    chars: &[char],
    start: usize,
) -> Result<(String, usize), ScriptLangError> {
    let mut out = String::from("\"");
    let mut index = start + 1;

    while index < chars.len() {
        match chars[index] {
            '\'' => {
                out.push('"');
                return Ok((out, index + 1));
            }
            '\\' => {
                let Some(next) = chars.get(index + 1).copied() else {
                    return Err(ScriptLangError::message(
                        "unterminated escape in single-quoted expr string",
                    ));
                };
                match next {
                    '\'' => out.push('\''),
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    'n' => out.push_str("\\n"),
                    'r' => out.push_str("\\r"),
                    't' => out.push_str("\\t"),
                    '0' => out.push_str("\\0"),
                    _ => {
                        out.push('\\');
                        out.push(next);
                    }
                }
                index += 2;
            }
            '"' => {
                out.push_str("\\\"");
                index += 1;
            }
            ch => {
                out.push(ch);
                index += 1;
            }
        }
    }

    Err(ScriptLangError::message(
        "unterminated single-quoted expr string",
    ))
}

fn is_expr_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::normalize_expr_escapes;

    #[test]
    fn normalize_expr_escapes_rewrites_minimal_scriptlang_operator_tokens() {
        let normalized =
            normalize_expr_escapes("hp LTE 10 AND ready AND LT_count == 0").expect("normalize");
        assert_eq!(normalized, "hp <= 10 && ready && LT_count == 0");
    }

    #[test]
    fn normalize_expr_escapes_keeps_tokens_inside_strings() {
        let normalized =
            normalize_expr_escapes(r#"flag AND note == "LT AND LTE" AND other == 'LT'"#)
                .expect("normalize");
        assert_eq!(
            normalized,
            r#"flag && note == "LT AND LTE" && other == "LT""#
        );
    }

    #[test]
    fn normalize_expr_escapes_rewrites_single_quotes_to_double_quoted_rhai_strings() {
        let normalized = normalize_expr_escapes(r#"'a\'b' AND name == 'R"in'"#).expect("normalize");
        assert_eq!(normalized, "\"a'b\" && name == \"R\\\"in\"");
    }
}
