use sl_core::{Form, ScriptLangError};

pub(crate) fn expand_macros(forms: &[Form]) -> Result<Vec<Form>, ScriptLangError> {
    Ok(forms.to_vec())
}

#[cfg(test)]
mod tests {
    use sl_core::{Form, FormField, FormMeta, FormValue, SourcePosition};

    use super::expand_macros;

    #[test]
    fn expand_macros_is_currently_a_passthrough_stage() {
        let forms = vec![Form {
            head: "module".to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 1, column: 1 },
                end: SourcePosition { row: 1, column: 20 },
                start_byte: 0,
                end_byte: 20,
            },
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("main".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(Vec::new()),
                },
            ],
        }];

        assert_eq!(expand_macros(&forms).expect("expand"), forms);
    }
}
