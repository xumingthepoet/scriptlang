use std::collections::BTreeMap;

use regex::Regex;
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
    let regex = Regex::new(r"\$\{([^}]*)\}")
        .map_err(|err| ScriptLangError::message(format!("template regex failed: {err}")))?;
    let mut segments = Vec::new();
    let mut cursor = 0usize;

    for captures in regex.captures_iter(source) {
        let whole = captures
            .get(0)
            .ok_or_else(|| ScriptLangError::message("template match is missing"))?;
        if whole.start() > cursor {
            segments.push(TextSegment::Literal(
                source[cursor..whole.start()].to_string(),
            ));
        }
        let expr = captures
            .get(1)
            .ok_or_else(|| ScriptLangError::message("template expression is missing"))?
            .as_str()
            .trim();
        segments.push(TextSegment::Expr(expr.to_string()));
        cursor = whole.end();
    }

    if cursor < source.len() {
        segments.push(TextSegment::Literal(source[cursor..].to_string()));
    }
    if segments.is_empty() {
        segments.push(TextSegment::Literal(source.to_string()));
    }

    Ok(TextTemplate { segments })
}
