use std::collections::BTreeMap;

use roxmltree::{Document, Node, NodeType};
use sl_core::{ScriptLangError, XmlContentItem, XmlField, XmlForm, XmlMeta, XmlPosition, XmlValue};

pub fn parse_modules_from_xml_map(
    sources: &BTreeMap<String, String>,
) -> Result<Vec<XmlForm>, ScriptLangError> {
    sources
        .iter()
        .map(|(source_name, xml)| parse_module_xml_with_source(xml, Some(source_name.as_str())))
        .collect::<Result<Vec<_>, _>>()
}

pub fn parse_module_xml(xml: &str) -> Result<XmlForm, ScriptLangError> {
    parse_module_xml_with_source(xml, None)
}

fn parse_module_xml_with_source(
    xml: &str,
    source_name: Option<&str>,
) -> Result<XmlForm, ScriptLangError> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    if root.tag_name().name() != "module" {
        return Err(ScriptLangError::message("root element must be <module>"));
    }
    Ok(parse_form(&doc, root, source_name))
}

fn parse_form(doc: &Document<'_>, node: Node<'_, '_>, source_name: Option<&str>) -> XmlForm {
    let range = node.range();
    let start = doc.text_pos_at(range.start);
    let end = doc.text_pos_at(range.end);
    let mut fields = node
        .attributes()
        .map(|attr| XmlField {
            name: attr.name().to_string(),
            value: XmlValue::String(attr.value().to_string()),
        })
        .collect::<Vec<_>>();
    fields.push(XmlField {
        name: "content".to_string(),
        value: XmlValue::Content(parse_content(doc, node, source_name)),
    });

    XmlForm {
        tag: node.tag_name().name().to_string(),
        meta: XmlMeta {
            source_name: source_name.map(str::to_string),
            start: XmlPosition {
                row: start.row,
                column: start.col,
            },
            end: XmlPosition {
                row: end.row,
                column: end.col,
            },
            start_byte: range.start,
            end_byte: range.end,
        },
        fields,
    }
}

fn parse_content(
    doc: &Document<'_>,
    node: Node<'_, '_>,
    source_name: Option<&str>,
) -> Vec<XmlContentItem> {
    let mut items = Vec::new();
    for child in node.children() {
        match child.node_type() {
            NodeType::Element => {
                items.push(XmlContentItem::Node(parse_form(doc, child, source_name)));
            }
            NodeType::Text => {
                if let Some(text) = child.text() {
                    items.push(XmlContentItem::Text(text.to_string()));
                }
            }
            _ => {}
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{XmlContentItem, XmlField, XmlForm, XmlValue};

    use super::{parse_module_xml, parse_modules_from_xml_map};

    fn field<'a>(form: &'a XmlForm, name: &str) -> &'a XmlField {
        form.fields
            .iter()
            .find(|field| field.name == name)
            .unwrap_or_else(|| panic!("missing field `{name}`"))
    }

    fn content(form: &XmlForm) -> &[XmlContentItem] {
        match &field(form, "content").value {
            XmlValue::Content(items) => items,
            other => panic!("expected content field, got {other:?}"),
        }
    }

    fn string_field<'a>(form: &'a XmlForm, name: &str) -> &'a str {
        match &field(form, name).value {
            XmlValue::String(value) => value,
            other => panic!("expected string field, got {other:?}"),
        }
    }

    fn text_item(item: &XmlContentItem) -> &str {
        match item {
            XmlContentItem::Text(text) => text,
            other => panic!("expected text item, got {other:?}"),
        }
    }

    fn node_item(item: &XmlContentItem) -> &XmlForm {
        match item {
            XmlContentItem::Node(node) => node,
            other => panic!("expected node item, got {other:?}"),
        }
    }

    #[test]
    fn parse_module_xml_rejects_non_module_root() {
        let error = parse_module_xml("<script name=\"entry\" />").expect_err("should fail");

        assert_eq!(error.to_string(), "root element must be <module>");
    }

    #[test]
    fn parse_module_xml_builds_xml_form_with_ordered_fields_and_meta() {
        let module = parse_module_xml(
            r#"
            <module name="main" flavor="demo">
              before
              <script name="entry"><end /></script>
              after
            </module>
            "#,
        )
        .expect("module should parse");

        assert_eq!(module.tag, "module");
        assert_eq!(module.fields.len(), 3);
        assert_eq!(module.fields[0].name, "name");
        assert_eq!(module.fields[1].name, "flavor");
        assert_eq!(module.fields[2].name, "content");
        assert_eq!(string_field(&module, "name"), "main");
        assert_eq!(string_field(&module, "flavor"), "demo");
        assert_eq!(module.meta.start_byte, 13);
        assert!(module.meta.end_byte > module.meta.start_byte);

        let items = content(&module);
        assert_eq!(text_item(&items[0]).trim(), "before");
        assert_eq!(node_item(&items[1]).tag, "script");
        assert_eq!(text_item(&items[2]).trim(), "after");
    }

    #[test]
    fn parse_module_xml_preserves_nested_content_sequence() {
        let module = parse_module_xml(
            r#"<module name="main"><script name="entry">left<text>tag</text>right</script></module>"#,
        )
        .expect("module should parse");
        let script = node_item(&content(&module)[0]);
        let items = content(script);

        assert_eq!(text_item(&items[0]), "left");
        assert_eq!(node_item(&items[1]).tag, "text");
        assert_eq!(text_item(&items[2]), "right");
    }

    #[test]
    fn parse_module_xml_supports_empty_and_self_closing_elements() {
        let module = parse_module_xml(
            r#"<module name="main"><script name="entry"><end /></script></module>"#,
        )
        .expect("module should parse");
        let script = node_item(&content(&module)[0]);
        let end = node_item(&content(script)[0]);

        assert_eq!(end.tag, "end");
        assert!(content(end).is_empty());
    }

    #[test]
    fn parse_modules_from_xml_map_collects_multiple_modules_and_source_names() {
        let modules = parse_modules_from_xml_map(&BTreeMap::from([
            (
                "a.xml".to_string(),
                r#"<module name="a"><script name="entry"><end /></script></module>"#.to_string(),
            ),
            (
                "b.xml".to_string(),
                r#"<module name="b"><script name="entry"><end /></script></module>"#.to_string(),
            ),
        ]))
        .expect("modules should parse");

        assert_eq!(modules.len(), 2);
        assert_eq!(string_field(&modules[0], "name"), "a");
        assert_eq!(modules[0].meta.source_name.as_deref(), Some("a.xml"));
        assert_eq!(string_field(&modules[1], "name"), "b");
        assert_eq!(modules[1].meta.source_name.as_deref(), Some("b.xml"));
    }

    #[test]
    fn parse_modules_from_xml_map_fails_if_any_module_is_invalid() {
        let error = parse_modules_from_xml_map(&BTreeMap::from([
            (
                "ok.xml".to_string(),
                r#"<module name="a"><script name="entry"><end /></script></module>"#.to_string(),
            ),
            ("bad.xml".to_string(), "<module>".to_string()),
        ]))
        .expect_err("should fail");

        assert!(error.to_string().contains("xml parse error"));
    }
}
