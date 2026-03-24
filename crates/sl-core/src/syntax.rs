#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Form {
    pub head: String,
    pub meta: FormMeta,
    pub fields: Vec<FormField>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct FormMeta {
    pub source_name: Option<String>,
    pub start: SourcePosition,
    pub end: SourcePosition,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SourcePosition {
    pub row: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormField {
    pub name: String,
    pub value: FormValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormValue {
    String(String),
    Sequence(Vec<FormItem>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormItem {
    Text(String),
    Form(Form),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextTemplate {
    pub segments: Vec<TextSegment>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextSegment {
    Literal(String),
    Expr(String),
}

#[cfg(test)]
mod tests {
    use super::{
        Form, FormField, FormItem, FormMeta, FormValue, SourcePosition, TextSegment, TextTemplate,
    };

    #[test]
    fn syntax_types_cover_public_form_and_template_shapes() {
        let child = Form {
            head: "text".to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 3, column: 5 },
                end: SourcePosition { row: 3, column: 22 },
                start_byte: 16,
                end_byte: 33,
            },
            fields: vec![FormField {
                name: "children".to_string(),
                value: FormValue::Sequence(vec![FormItem::Text("hello".to_string())]),
            }],
        };
        let form = Form {
            head: "module".to_string(),
            meta: FormMeta {
                source_name: Some("main.xml".to_string()),
                start: SourcePosition { row: 1, column: 1 },
                end: SourcePosition { row: 5, column: 10 },
                start_byte: 0,
                end_byte: 64,
            },
            fields: vec![
                FormField {
                    name: "name".to_string(),
                    value: FormValue::String("main".to_string()),
                },
                FormField {
                    name: "children".to_string(),
                    value: FormValue::Sequence(vec![
                        FormItem::Text("\n  ".to_string()),
                        FormItem::Form(child.clone()),
                        FormItem::Text("\n".to_string()),
                    ]),
                },
            ],
        };
        let template = TextTemplate {
            segments: vec![
                TextSegment::Literal("hello ".to_string()),
                TextSegment::Expr("name".to_string()),
            ],
        };

        assert_eq!(form.head, "module");
        assert!(matches!(
            &form.fields[0],
            FormField { name, value: FormValue::String(value) }
                if name == "name" && value == "main"
        ));
        assert!(matches!(
            &form.fields[1],
            FormField { name, value: FormValue::Sequence(items) }
                if name == "children"
                    && matches!(&items[1], FormItem::Form(node) if node.head == child.head)
        ));
        assert!(matches!(
            &template.segments[..],
            [TextSegment::Literal(text), TextSegment::Expr(expr)]
                if text == "hello " && expr == "name"
        ));
    }
}
