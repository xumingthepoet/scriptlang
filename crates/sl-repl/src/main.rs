use rustyline::{DefaultEditor, error::ReadlineError};
use sl_repl::{ExecutionResult, ExecutionState, InspectTarget, ReplSession};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut editor = DefaultEditor::new()?;
    let mut session = ReplSession::new()?;
    let mut buffer = String::new();

    while !session.is_exited() {
        let prompt = if buffer.is_empty() {
            "sl-repl> "
        } else {
            "...> "
        };
        let line = match editor.readline(prompt) {
            Ok(line) => line,
            Err(ReadlineError::Interrupted) => {
                if buffer.is_empty() {
                    println!();
                } else {
                    buffer.clear();
                    eprintln!("error: current xml fragment discarded");
                }
                continue;
            }
            Err(ReadlineError::Eof) => {
                if buffer.trim().is_empty() {
                    break;
                }
                if !xml_fragment_is_balanced(&buffer) {
                    eprintln!("error: incomplete xml fragment");
                    break;
                }
                let input = std::mem::take(&mut buffer);
                if let Err(error) = editor.add_history_entry(input.trim()) {
                    eprintln!("error: failed to store history entry: {error}");
                }
                match session.submit_xml(&input) {
                    Ok(result) => print_submission(result),
                    Err(error) => eprintln!("error: {error}"),
                }
                continue;
            }
            Err(error) => return Err(error.into()),
        };

        if !buffer.is_empty() || !line.trim_start().starts_with(':') {
            buffer.push_str(&line);
            buffer.push('\n');
            if !xml_fragment_is_balanced(&buffer) {
                continue;
            }
            let input = std::mem::take(&mut buffer);
            if let Err(error) = editor.add_history_entry(input.trim()) {
                eprintln!("error: failed to store history entry: {error}");
            }
            match session.submit_xml(&input) {
                Ok(result) => print_submission(result),
                Err(error) => eprintln!("error: {error}"),
            }
            continue;
        }

        let trimmed = line.trim();
        if !trimmed.is_empty()
            && let Err(error) = editor.add_history_entry(trimmed)
        {
            eprintln!("error: failed to store history entry: {error}");
        }
        match handle_command(&mut session, trimmed) {
            Ok(Some(output)) if !output.is_empty() => println!("{output}"),
            Ok(_) => {}
            Err(error) => eprintln!("error: {error}"),
        }
    }

    Ok(())
}

fn handle_command(
    session: &mut ReplSession,
    input: &str,
) -> Result<Option<String>, sl_core::ScriptLangError> {
    let (command, arg) = split_command(input);
    match command {
        ":help" => Ok(Some(
            [
                ":help",
                ":load PATH",
                ":ast",
                ":semantic",
                ":ir",
                ":bindings",
                ":modules",
                ":choose INDEX",
                ":quit",
            ]
            .join("\n"),
        )),
        ":load" => {
            let path =
                arg.ok_or_else(|| sl_core::ScriptLangError::message("`:load` requires a path"))?;
            let loaded = session.load_path(path)?;
            if loaded.modules.is_empty() {
                Ok(Some("loaded 0 modules".to_string()))
            } else {
                Ok(Some(format!("loaded {}", loaded.modules.join(", "))))
            }
        }
        ":ast" => Ok(Some(session.inspect(InspectTarget::Ast))),
        ":semantic" => Ok(Some(session.inspect(InspectTarget::Semantic))),
        ":ir" => Ok(Some(session.inspect(InspectTarget::Ir))),
        ":bindings" => Ok(Some(session.inspect(InspectTarget::Bindings))),
        ":modules" => Ok(Some(session.inspect(InspectTarget::Modules))),
        ":choose" => {
            let raw_index = arg
                .ok_or_else(|| sl_core::ScriptLangError::message("`:choose` requires an index"))?;
            let index = raw_index.parse::<usize>().map_err(|_| {
                sl_core::ScriptLangError::message(format!("invalid choice index `{raw_index}`"))
            })?;
            print_execution(&session.choose(index)?);
            Ok(None)
        }
        ":quit" => {
            session.quit();
            Ok(None)
        }
        other => Err(sl_core::ScriptLangError::message(format!(
            "unknown repl command `{other}`"
        ))),
    }
}

fn split_command(input: &str) -> (&str, Option<&str>) {
    match input.split_once(char::is_whitespace) {
        Some((command, rest)) => (command, Some(rest.trim()).filter(|rest| !rest.is_empty())),
        None => (input, None),
    }
}

fn print_submission(result: sl_repl::SubmissionResult) {
    match result {
        sl_repl::SubmissionResult::ContextUpdated => println!("context updated"),
        sl_repl::SubmissionResult::ModuleUpdated { module_name } => {
            println!("module {module_name} updated");
        }
        sl_repl::SubmissionResult::Executed(execution) => print_execution(&execution),
    }
}

fn print_execution(result: &ExecutionResult) {
    for event in &result.events {
        match event {
            sl_core::StepEvent::Text { text, tag } => match tag {
                Some(tag) => println!("[{tag}] {text}"),
                None => println!("{text}"),
            },
        }
    }
    match &result.state {
        ExecutionState::Ready => {
            if result.events.is_empty() {
                println!("ok");
            }
        }
        ExecutionState::SuspendedChoice { prompt, items } => {
            if let Some(prompt) = prompt {
                println!("{prompt}");
            }
            for (index, item) in items.iter().enumerate() {
                println!("{index}: {item}");
            }
        }
        ExecutionState::Exited => {
            if result.events.is_empty() {
                println!("exited");
            }
        }
    }
}

fn xml_fragment_is_balanced(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut cursor = 0usize;
    let mut saw_root = false;
    let mut stack = Vec::<String>::new();

    while cursor < bytes.len() {
        if bytes[cursor] != b'<' {
            cursor += 1;
            continue;
        }

        if input[cursor..].starts_with("<!--") {
            let Some(end) = input[cursor + 4..].find("-->") else {
                return false;
            };
            cursor += 4 + end + 3;
            continue;
        }

        if input[cursor..].starts_with("<?") {
            let Some(end) = input[cursor + 2..].find("?>") else {
                return false;
            };
            cursor += 2 + end + 2;
            continue;
        }

        if input[cursor..].starts_with("<![CDATA[") {
            let Some(end) = input[cursor + 9..].find("]]>") else {
                return false;
            };
            cursor += 9 + end + 3;
            continue;
        }

        let Some(tag_end) = find_tag_end(input, cursor + 1) else {
            return false;
        };
        let raw = input[cursor + 1..tag_end].trim();
        if raw.is_empty() {
            return false;
        }

        if let Some(stripped) = raw.strip_prefix('/') {
            let name = parse_tag_name(stripped.trim());
            let Some(expected) = stack.pop() else {
                return false;
            };
            if expected != name {
                return false;
            }
        } else if !raw.starts_with('!') {
            let self_closing = raw.ends_with('/');
            let name = parse_tag_name(raw.trim_end_matches('/').trim());
            if name.is_empty() {
                return false;
            }
            saw_root = true;
            if !self_closing {
                stack.push(name.to_string());
            }
        }

        cursor = tag_end + 1;
    }

    saw_root && stack.is_empty()
}

fn find_tag_end(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    let mut cursor = start;
    let mut quote = None::<u8>;

    while cursor < bytes.len() {
        let current = bytes[cursor];
        if let Some(active) = quote {
            if current == active {
                quote = None;
            }
        } else if current == b'\'' || current == b'"' {
            quote = Some(current);
        } else if current == b'>' {
            return Some(cursor);
        }
        cursor += 1;
    }

    None
}

fn parse_tag_name(raw: &str) -> &str {
    raw.split_whitespace().next().unwrap_or("")
}
