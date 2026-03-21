pub(crate) const RESOLVED_VAR_PREFIX: &str = "__sl_var__(";

pub(crate) fn qualified_member_name(module_name: &str, member_name: &str) -> String {
    format!("{module_name}.{member_name}")
}

pub(crate) fn runtime_global_name(qualified_name: &str) -> String {
    let mut runtime_name = String::from("__sl_global");
    for ch in qualified_name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            runtime_name.push(ch);
        } else {
            runtime_name.push('_');
        }
    }
    runtime_name
}

pub(crate) fn resolved_var_placeholder(qualified_name: &str) -> String {
    format!("{RESOLVED_VAR_PREFIX}{qualified_name})")
}

pub(crate) fn script_literal_key(raw: &str, current_module: &str) -> Option<String> {
    let raw = raw.strip_prefix('@')?;
    if raw.is_empty() {
        return None;
    }
    if let Some((module_name, script_name)) = raw.rsplit_once('.') {
        if module_name.is_empty() || script_name.is_empty() {
            return None;
        }
        Some(qualified_member_name(module_name, script_name))
    } else {
        Some(qualified_member_name(current_module, raw))
    }
}

pub(crate) fn function_literal_key(raw: &str, current_module: &str) -> Option<String> {
    let raw = raw.strip_prefix('#')?;
    if raw.is_empty() {
        return None;
    }
    if let Some((module_name, function_name)) = raw.rsplit_once('.') {
        if module_name.is_empty() || function_name.is_empty() {
            return None;
        }
        Some(qualified_member_name(module_name, function_name))
    } else {
        Some(qualified_member_name(current_module, raw))
    }
}

pub(crate) fn lower_resolved_vars_to_runtime_names(source: &str) -> String {
    let mut lowered = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let ch = bytes[cursor] as char;
        if ch == '"' || ch == '\'' {
            let end = scan_quoted(bytes, cursor);
            lowered.push_str(&source[cursor..end]);
            cursor = end;
            continue;
        }

        if source[cursor..].starts_with(RESOLVED_VAR_PREFIX) {
            let start = cursor + RESOLVED_VAR_PREFIX.len();
            if let Some(end_offset) = source[start..].find(')') {
                let end = start + end_offset;
                lowered.push_str(&runtime_global_name(&source[start..end]));
                cursor = end + 1;
                continue;
            }
        }

        lowered.push(ch);
        cursor += ch.len_utf8();
    }

    lowered
}

fn scan_quoted(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor += 2,
            ch if ch == quote => return cursor + 1,
            _ => cursor += 1,
        }
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::{
        function_literal_key, lower_resolved_vars_to_runtime_names, resolved_var_placeholder,
        runtime_global_name, script_literal_key,
    };

    #[test]
    fn helpers_encode_and_lower_resolved_var_placeholders() {
        let local = resolved_var_placeholder("main.answer");
        let imported = resolved_var_placeholder("m1.shared");
        let source = format!(r#"{local} += "{imported}" + {imported};"#);

        assert_eq!(
            lower_resolved_vars_to_runtime_names(&source),
            format!(
                r#"{} += "{imported}" + {};"#,
                runtime_global_name("main.answer"),
                runtime_global_name("m1.shared"),
            )
        );
    }

    #[test]
    fn script_literal_key_supports_short_and_qualified_forms() {
        assert_eq!(
            script_literal_key("@loop", "main"),
            Some("main.loop".to_string())
        );
        assert_eq!(
            script_literal_key("@other.loop", "main"),
            Some("other.loop".to_string())
        );
        assert_eq!(script_literal_key("@", "main"), None);
        assert_eq!(script_literal_key("@main.", "main"), None);
        assert_eq!(script_literal_key("@.loop", "main"), None);
    }

    #[test]
    fn function_literal_key_supports_short_and_qualified_forms() {
        assert_eq!(
            function_literal_key("#pick", "main"),
            Some("main.pick".to_string())
        );
        assert_eq!(
            function_literal_key("#other.pick", "main"),
            Some("other.pick".to_string())
        );
        assert_eq!(function_literal_key("#", "main"), None);
        assert_eq!(function_literal_key("#main.", "main"), None);
        assert_eq!(function_literal_key("#.pick", "main"), None);
    }
}
