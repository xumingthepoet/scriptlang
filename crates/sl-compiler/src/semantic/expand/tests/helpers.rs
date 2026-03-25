//! Shared test helpers for the expand module test suite.
//!
//! Provides `meta`, `form`, `form_field`, `children`, `text`, `node`, `child`, and
//! `analyzed` — identical helpers previously duplicated across program.rs,
//! scripts.rs, scope.rs, and declared_types.rs.

use sl_core::{Form, FormField, FormItem, FormMeta, FormValue, SourcePosition};

use crate::semantic::env::ExpandEnv;
use crate::semantic::expand::{analyze_program, expand_raw_forms};

/// Fixed source metadata for test forms.
#[allow(unused)]
pub(crate) fn meta() -> FormMeta {
    FormMeta {
        source_name: Some("main.xml".to_string()),
        start: SourcePosition { row: 1, column: 1 },
        end: SourcePosition { row: 1, column: 20 },
        start_byte: 0,
        end_byte: 20,
    }
}

/// Build a minimal `Form` with the given head and fields.
#[allow(unused)]
pub(crate) fn form(head: &str, fields: Vec<FormField>) -> Form {
    Form {
        head: head.to_string(),
        meta: meta(),
        fields,
    }
}

/// Build a string-valued `FormField` (an attribute).
#[allow(unused)]
pub(crate) fn form_field(name: &str, value: &str) -> FormField {
    FormField {
        name: name.to_string(),
        value: FormValue::String(value.to_string()),
    }
}

/// Build a `"children"` `FormField` wrapping a `Vec<FormItem>`.
#[allow(unused)]
pub(crate) fn children(items: Vec<FormItem>) -> FormField {
    FormField {
        name: "children".to_string(),
        value: FormValue::Sequence(items),
    }
}

/// Wrap a string as a `FormItem::Text`.
#[allow(unused)]
pub(crate) fn text(value: &str) -> FormItem {
    FormItem::Text(value.to_string())
}

/// Wrap a `Form` as a `FormItem::Form`.
#[allow(unused)]
pub(crate) fn child(form: Form) -> FormItem {
    FormItem::Form(form)
}

/// Convenience constructor: build a named node with (k, v) attribute pairs
/// and `Vec<FormItem>` children.
#[allow(unused)]
pub(crate) fn node(head: &str, attrs: Vec<(&str, &str)>, items: Vec<FormItem>) -> Form {
    let mut fields = attrs
        .into_iter()
        .map(|(k, v)| form_field(k, v))
        .collect::<Vec<_>>();
    fields.push(children(items));
    form(head, fields)
}

/// Expand a list of forms and run semantic analysis, returning the program.
#[allow(unused)]
pub(crate) fn analyzed(forms: Vec<Form>) -> crate::semantic::types::SemanticProgram {
    let mut env = ExpandEnv::default();
    let _ = expand_raw_forms(&forms, &mut env).expect("expand");
    analyze_program(&env.program).expect("analyze")
}
