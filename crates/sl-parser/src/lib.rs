use std::collections::BTreeMap;

use roxmltree::{Document, Node};
use sl_core::{
    ParsedChoiceOption, ParsedModule, ParsedScript, ParsedStmt, ParsedVar, ScriptLangError,
    TextSegment, TextTemplate,
};

pub fn parse_modules_from_xml_map(
    sources: &BTreeMap<String, String>,
) -> Result<Vec<ParsedModule>, ScriptLangError> {
    sources
        .values()
        .map(|xml| parse_module_xml(xml))
        .collect::<Result<Vec<_>, _>>()
}

pub fn parse_module_xml(xml: &str) -> Result<ParsedModule, ScriptLangError> {
    let doc = Document::parse(xml)?;
    let root = doc.root_element();
    if root.tag_name().name() != "module" {
        return Err(ScriptLangError::message("root element must be <module>"));
    }
    parse_module(root)
}

fn parse_module(root: Node<'_, '_>) -> Result<ParsedModule, ScriptLangError> {
    let name = required_attr(root, "name", "<module>")?.to_string();
    let mut vars = Vec::new();
    let mut scripts = Vec::new();

    for child in element_children(root) {
        match child.tag_name().name() {
            "var" => vars.push(ParsedVar {
                name: required_attr(child, "name", "<var>")?.to_string(),
                expr: child.text().unwrap_or_default().trim().to_string(),
            }),
            "script" => scripts.push(parse_script(child)?),
            other => {
                return Err(ScriptLangError::message(format!(
                    "unsupported <module> child <{other}> in MVP"
                )));
            }
        }
    }

    Ok(ParsedModule {
        name,
        vars,
        scripts,
    })
}

fn parse_script(node: Node<'_, '_>) -> Result<ParsedScript, ScriptLangError> {
    let name = required_attr(node, "name", "<script>")?.to_string();
    let body = parse_stmt_list(node)?;
    Ok(ParsedScript { name, body })
}

fn parse_stmt_list(parent: Node<'_, '_>) -> Result<Vec<ParsedStmt>, ScriptLangError> {
    let mut body = Vec::new();
    for child in element_children(parent) {
        body.push(parse_stmt(child)?);
    }
    Ok(body)
}

fn parse_stmt(node: Node<'_, '_>) -> Result<ParsedStmt, ScriptLangError> {
    match node.tag_name().name() {
        "temp" => Ok(ParsedStmt::Temp {
            name: required_attr(node, "name", "<temp>")?.to_string(),
            expr: node.text().unwrap_or_default().trim().to_string(),
        }),
        "code" => Ok(ParsedStmt::Code {
            code: node.text().unwrap_or_default().trim().to_string(),
        }),
        "text" => Ok(ParsedStmt::Text {
            template: parse_text_template(node.text().unwrap_or_default().trim())?,
            tag: node.attribute("tag").map(str::to_string),
        }),
        "if" => Ok(ParsedStmt::If {
            when: required_attr(node, "when", "<if>")?.to_string(),
            body: parse_stmt_list(node)?,
        }),
        "choice" => {
            let prompt = node
                .attribute("text")
                .map(parse_text_template)
                .transpose()?;
            let mut options = Vec::new();
            for child in element_children(node) {
                if child.tag_name().name() != "option" {
                    return Err(ScriptLangError::message(
                        "<choice> only supports <option> children in MVP",
                    ));
                }
                options.push(ParsedChoiceOption {
                    text: parse_text_template(required_attr(child, "text", "<option>")?)?,
                    body: parse_stmt_list(child)?,
                });
            }
            Ok(ParsedStmt::Choice { prompt, options })
        }
        "goto" => Ok(ParsedStmt::Goto {
            target_script_ref: required_attr(node, "script", "<goto>")?.to_string(),
        }),
        "end" => Ok(ParsedStmt::End),
        other => Err(ScriptLangError::message(format!(
            "unsupported statement <{other}> in MVP"
        ))),
    }
}

fn element_children<'a>(parent: Node<'a, 'a>) -> impl Iterator<Item = Node<'a, 'a>> {
    parent.children().filter(Node::is_element)
}

fn required_attr<'a>(
    node: Node<'a, 'a>,
    attr: &str,
    context: &str,
) -> Result<&'a str, ScriptLangError> {
    node.attribute(attr)
        .ok_or_else(|| ScriptLangError::message(format!("{context} requires `{attr}`")))
}

fn parse_text_template(source: &str) -> Result<TextTemplate, ScriptLangError> {
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    while let Some(start_offset) = source[cursor..].find("${") {
        let start = cursor + start_offset;
        if start > cursor {
            segments.push(TextSegment::Literal(source[cursor..start].to_string()));
        }

        let expr_start = start + 2;
        let Some(end_offset) = source[expr_start..].find('}') else {
            if let Some(TextSegment::Literal(prefix)) = segments.last_mut() {
                prefix.push_str(&source[start..]);
            } else {
                segments.push(TextSegment::Literal(source[start..].to_string()));
            }
            cursor = source.len();
            break;
        };
        let expr_end = expr_start + end_offset;
        segments.push(TextSegment::Expr(
            source[expr_start..expr_end].trim().to_string(),
        ));
        cursor = expr_end + 1;
    }

    if cursor < source.len() {
        segments.push(TextSegment::Literal(source[cursor..].to_string()));
    }
    if segments.is_empty() {
        segments.push(TextSegment::Literal(source.to_string()));
    }

    Ok(TextTemplate { segments })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use sl_core::{ParsedChoiceOption, ParsedStmt, TextSegment, TextTemplate};

    use super::{parse_module_xml, parse_modules_from_xml_map, parse_text_template};

    fn expect_temp(stmt: &ParsedStmt) -> (&str, &str) {
        match stmt {
            ParsedStmt::Temp { name, expr } => (name, expr),
            _ => panic!("expected temp"),
        }
    }

    fn expect_code(stmt: &ParsedStmt) -> &str {
        match stmt {
            ParsedStmt::Code { code } => code,
            _ => panic!("expected code"),
        }
    }

    fn expect_text(stmt: &ParsedStmt) -> (&TextTemplate, Option<&str>) {
        match stmt {
            ParsedStmt::Text { template, tag } => (template, tag.as_deref()),
            _ => panic!("expected text"),
        }
    }

    fn expect_if(stmt: &ParsedStmt) -> (&str, &[ParsedStmt]) {
        match stmt {
            ParsedStmt::If { when, body } => (when, body),
            _ => panic!("expected if"),
        }
    }

    fn expect_choice(stmt: &ParsedStmt) -> (Option<&TextTemplate>, &[ParsedChoiceOption]) {
        match stmt {
            ParsedStmt::Choice { prompt, options } => (prompt.as_ref(), options),
            _ => panic!("expected choice"),
        }
    }

    fn expect_goto(stmt: &ParsedStmt) -> &str {
        match stmt {
            ParsedStmt::Goto { target_script_ref } => target_script_ref,
            _ => panic!("expected goto"),
        }
    }

    fn expect_end(stmt: &ParsedStmt) {
        match stmt {
            ParsedStmt::End => {}
            _ => panic!("expected end"),
        }
    }

    fn expect_literal_segment(segment: &TextSegment) -> &str {
        match segment {
            TextSegment::Literal(text) => text,
            _ => panic!("expected literal"),
        }
    }

    fn expect_expr_segment(segment: &TextSegment) -> &str {
        match segment {
            TextSegment::Expr(text) => text,
            _ => panic!("expected expr"),
        }
    }

    #[test]
    fn parse_module_xml_rejects_non_module_root() {
        let error = parse_module_xml("<script name=\"entry\" />").expect_err("should fail");

        assert_eq!(error.to_string(), "root element must be <module>");
    }

    #[test]
    fn parse_module_xml_requires_module_name() {
        let error = parse_module_xml("<module><script name=\"entry\" /></module>")
            .expect_err("should fail");

        assert_eq!(error.to_string(), "<module> requires `name`");
    }

    #[test]
    fn parse_module_xml_parses_supported_children() {
        let module = parse_module_xml(
            r#"
            <module name="main">
              <var name="answer"> 40 + 2 </var>
              <script name="entry">
                <temp name="x">1</temp>
              </script>
            </module>
            "#,
        )
        .expect("module should parse");

        assert_eq!(module.name, "main");
        assert_eq!(module.vars.len(), 1);
        assert_eq!(module.vars[0].name, "answer");
        assert_eq!(module.vars[0].expr, "40 + 2");
        assert_eq!(module.scripts.len(), 1);
        assert_eq!(module.scripts[0].name, "entry");
        let (name, expr) = expect_temp(&module.scripts[0].body[0]);
        assert_eq!(name, "x");
        assert_eq!(expr, "1");
    }

    #[test]
    fn parse_module_xml_rejects_unsupported_module_child() {
        let error = parse_module_xml(
            r#"
            <module name="main">
              <while />
            </module>
            "#,
        )
        .expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "unsupported <module> child <while> in MVP"
        );
    }

    #[test]
    fn parse_stmt_supports_all_mvp_statement_kinds() {
        let module = parse_module_xml(
            r#"
            <module name="main">
              <script name="entry">
                <temp name="x">1</temp>
                <code>x = x + 1;</code>
                <text tag="line">hello ${x}</text>
                <if when="x > 0">
                  <end />
                </if>
                <choice text="pick ${x}">
                  <option text="left">
                    <goto script="main.left" />
                  </option>
                </choice>
                <goto script="main.done" />
                <end />
              </script>
            </module>
            "#,
        )
        .expect("module should parse");
        let stmts = &module.scripts[0].body;
        let (name, expr) = expect_temp(&stmts[0]);
        assert_eq!(name, "x");
        assert_eq!(expr, "1");

        assert_eq!(expect_code(&stmts[1]), "x = x + 1;");

        let (template, tag) = expect_text(&stmts[2]);
        assert_eq!(tag, Some("line"));
        assert_eq!(template.segments.len(), 2);

        let (when, if_body) = expect_if(&stmts[3]);
        assert_eq!(when, "x > 0");
        assert_eq!(if_body.len(), 1);

        let (prompt, options) = expect_choice(&stmts[4]);
        assert!(prompt.is_some());
        assert_eq!(options.len(), 1);

        assert_eq!(expect_goto(&stmts[5]), "main.done");
        expect_end(&stmts[6]);
    }

    #[test]
    fn parse_choice_rejects_non_option_child() {
        let error = parse_module_xml(
            r#"
            <module name="main">
              <script name="entry">
                <choice>
                  <text>bad</text>
                </choice>
              </script>
            </module>
            "#,
        )
        .expect_err("should fail");

        assert_eq!(
            error.to_string(),
            "<choice> only supports <option> children in MVP"
        );
    }

    #[test]
    fn parse_stmt_requires_expected_attributes() {
        let cases = [
            (
                r#"<module name="m"><script><end /></script></module>"#,
                "<script> requires `name`",
            ),
            (
                r#"<module name="m"><script name="s"><temp>1</temp></script></module>"#,
                "<temp> requires `name`",
            ),
            (
                r#"<module name="m"><script name="s"><if><end /></if></script></module>"#,
                "<if> requires `when`",
            ),
            (
                r#"<module name="m"><script name="s"><goto /></script></module>"#,
                "<goto> requires `script`",
            ),
            (
                r#"<module name="m"><script name="s"><choice><option /></choice></script></module>"#,
                "<option> requires `text`",
            ),
        ];

        for (xml, expected) in cases {
            let error = parse_module_xml(xml).expect_err("should fail");
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn parse_stmt_rejects_unsupported_statement_tag() {
        let error = parse_module_xml(
            r#"
            <module name="main">
              <script name="entry">
                <while />
              </script>
            </module>
            "#,
        )
        .expect_err("should fail");

        assert_eq!(error.to_string(), "unsupported statement <while> in MVP");
    }

    #[test]
    fn parse_text_template_covers_literal_and_expression_shapes() {
        let empty = parse_text_template("").expect("template should parse");
        assert_eq!(empty.segments.len(), 1);
        assert!(expect_literal_segment(&empty.segments[0]).is_empty());

        let literal = parse_text_template("hello").expect("template should parse");
        assert_eq!(literal.segments.len(), 1);
        assert_eq!(expect_literal_segment(&literal.segments[0]), "hello");

        let expr_only = parse_text_template("${ value }").expect("template should parse");
        assert_eq!(expr_only.segments.len(), 1);
        assert_eq!(expect_expr_segment(&expr_only.segments[0]), "value");

        let unclosed = parse_text_template("hello ${name").expect("template should parse");
        assert_eq!(unclosed.segments.len(), 1);
        assert_eq!(
            expect_literal_segment(&unclosed.segments[0]),
            "hello ${name"
        );

        let leading_unclosed = parse_text_template("${name").expect("template should parse");
        assert_eq!(leading_unclosed.segments.len(), 1);
        assert_eq!(
            expect_literal_segment(&leading_unclosed.segments[0]),
            "${name"
        );

        let mixed = parse_text_template("a ${left} b ${ } c").expect("template should parse");
        assert_eq!(mixed.segments.len(), 5);
        assert_eq!(expect_literal_segment(&mixed.segments[0]), "a ");
        assert_eq!(expect_expr_segment(&mixed.segments[1]), "left");
        assert_eq!(expect_literal_segment(&mixed.segments[2]), " b ");
        assert!(expect_expr_segment(&mixed.segments[3]).is_empty());
        assert_eq!(expect_literal_segment(&mixed.segments[4]), " c");
    }

    #[test]
    fn parse_modules_from_xml_map_collects_multiple_modules() {
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
        assert_eq!(modules[0].name, "a");
        assert_eq!(modules[1].name, "b");
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

    #[test]
    fn helper_extractors_panic_on_wrong_variants() {
        let stmt = ParsedStmt::End;

        assert!(std::panic::catch_unwind(|| expect_temp(&stmt)).is_err());
        assert!(std::panic::catch_unwind(|| expect_code(&stmt)).is_err());
        assert!(std::panic::catch_unwind(|| expect_text(&stmt)).is_err());
        assert!(std::panic::catch_unwind(|| expect_if(&stmt)).is_err());
        assert!(std::panic::catch_unwind(|| expect_choice(&stmt)).is_err());
        assert!(std::panic::catch_unwind(|| expect_goto(&stmt)).is_err());

        let wrong = ParsedStmt::Code {
            code: String::new(),
        };
        assert!(std::panic::catch_unwind(|| expect_end(&wrong)).is_err());

        let literal = TextSegment::Literal(String::new());
        assert!(std::panic::catch_unwind(|| expect_expr_segment(&literal)).is_err());

        let expr = TextSegment::Expr(String::new());
        assert!(std::panic::catch_unwind(|| expect_literal_segment(&expr)).is_err());
    }
}
