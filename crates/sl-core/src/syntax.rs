#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlForm {
    pub tag: String,
    pub meta: XmlMeta,
    pub fields: Vec<XmlField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlMeta {
    pub source_name: Option<String>,
    pub start: XmlPosition,
    pub end: XmlPosition,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlPosition {
    pub row: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XmlField {
    pub name: String,
    pub value: XmlValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmlValue {
    String(String),
    Content(Vec<XmlContentItem>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum XmlContentItem {
    Text(String),
    Node(XmlForm),
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
        TextSegment, TextTemplate, XmlContentItem, XmlField, XmlForm, XmlMeta, XmlPosition,
        XmlValue,
    };

    #[test]
    fn syntax_types_cover_public_xml_and_template_shapes() {
        let child = XmlForm {
            tag: "text".to_string(),
            meta: XmlMeta {
                source_name: Some("main.xml".to_string()),
                start: XmlPosition { row: 3, column: 5 },
                end: XmlPosition { row: 3, column: 22 },
                start_byte: 16,
                end_byte: 33,
            },
            fields: vec![XmlField {
                name: "content".to_string(),
                value: XmlValue::Content(vec![XmlContentItem::Text("hello".to_string())]),
            }],
        };
        let form = XmlForm {
            tag: "module".to_string(),
            meta: XmlMeta {
                source_name: Some("main.xml".to_string()),
                start: XmlPosition { row: 1, column: 1 },
                end: XmlPosition { row: 5, column: 10 },
                start_byte: 0,
                end_byte: 64,
            },
            fields: vec![
                XmlField {
                    name: "name".to_string(),
                    value: XmlValue::String("main".to_string()),
                },
                XmlField {
                    name: "content".to_string(),
                    value: XmlValue::Content(vec![
                        XmlContentItem::Text("\n  ".to_string()),
                        XmlContentItem::Node(child.clone()),
                        XmlContentItem::Text("\n".to_string()),
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

        assert_eq!(form.tag, "module");
        assert!(matches!(
            &form.fields[0],
            XmlField { name, value: XmlValue::String(value) }
                if name == "name" && value == "main"
        ));
        assert!(matches!(
            &form.fields[1],
            XmlField { name, value: XmlValue::Content(items) }
                if name == "content"
                    && matches!(&items[1], XmlContentItem::Node(node) if node.tag == child.tag)
        ));
        assert!(matches!(
            &template.segments[..],
            [TextSegment::Literal(text), TextSegment::Expr(expr)]
                if text == "hello " && expr == "name"
        ));
    }
}
