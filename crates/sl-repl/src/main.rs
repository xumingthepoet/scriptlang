use std::{env, fs, path::PathBuf};

use rustyline::{DefaultEditor, error::ReadlineError};
use sl_core::ScriptLangError;
use sl_repl::{ExecutionResult, ExecutionState, InspectTarget, ReplSession, SubmissionResult};

#[derive(Debug)]
enum CliMode {
    Interactive,
    Commands(Vec<String>),
    File(PathBuf),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = ReplSession::new()?;
    match parse_cli_mode(env::args().skip(1))? {
        CliMode::Interactive => run_interactive(&mut session)?,
        CliMode::Commands(commands) => run_commands(&mut session, &commands)?,
        CliMode::File(path) => run_file(&mut session, &path)?,
    }
    Ok(())
}

fn run_interactive(session: &mut ReplSession) -> Result<(), Box<dyn std::error::Error>> {
    let mut editor = DefaultEditor::new()?;
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
                submit_and_print(session, &input);
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
            submit_and_print(session, &input);
            continue;
        }

        let trimmed = line.trim();
        if !trimmed.is_empty()
            && let Err(error) = editor.add_history_entry(trimmed)
        {
            eprintln!("error: failed to store history entry: {error}");
        }
        match handle_command(session, trimmed) {
            Ok(Some(output)) if !output.is_empty() => println!("{output}"),
            Ok(_) => {}
            Err(error) => eprintln!("error: {error}"),
        }
    }

    Ok(())
}

fn run_commands(
    session: &mut ReplSession,
    commands: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    for command in commands {
        run_repl_input(session, command)?;
        if session.is_exited() {
            break;
        }
    }
    Ok(())
}

fn run_file(session: &mut ReplSession, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    run_transcript(session, &source)?;
    Ok(())
}

fn run_repl_input(session: &mut ReplSession, input: &str) -> Result<(), ScriptLangError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.starts_with(':') {
        if let Some(output) = handle_command(session, trimmed)?
            && !output.is_empty()
        {
            println!("{output}");
        }
    } else {
        print_submission(session.submit_xml(trimmed)?);
    }
    Ok(())
}

fn run_transcript(session: &mut ReplSession, source: &str) -> Result<(), ScriptLangError> {
    let mut buffer = String::new();
    for line in source.lines() {
        if session.is_exited() {
            break;
        }
        let trimmed = line.trim();
        if buffer.is_empty() && trimmed.starts_with(':') {
            run_repl_input(session, trimmed)?;
            continue;
        }
        if buffer.is_empty() && trimmed.is_empty() {
            continue;
        }
        buffer.push_str(line);
        buffer.push('\n');
        if xml_fragment_is_balanced(&buffer) {
            let input = std::mem::take(&mut buffer);
            run_repl_input(session, &input)?;
        }
    }

    if !buffer.trim().is_empty() {
        if !xml_fragment_is_balanced(&buffer) {
            return Err(ScriptLangError::message(
                "incomplete xml fragment in repl input file",
            ));
        }
        let input = std::mem::take(&mut buffer);
        run_repl_input(session, &input)?;
    }

    Ok(())
}

fn parse_cli_mode<I>(mut args: I) -> Result<CliMode, ScriptLangError>
where
    I: Iterator<Item = String>,
{
    let mut commands = Vec::new();
    let mut file = None::<PathBuf>;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-c" | "--command" => {
                let command = args.next().ok_or_else(|| {
                    ScriptLangError::message(format!("`{arg}` requires a following argument"))
                })?;
                commands.push(command);
            }
            "-f" | "--file" => {
                let path = args.next().ok_or_else(|| {
                    ScriptLangError::message(format!("`{arg}` requires a following path"))
                })?;
                if file.replace(PathBuf::from(path)).is_some() {
                    return Err(ScriptLangError::message(
                        "repl file mode only accepts one path",
                    ));
                }
            }
            "-h" | "--help" => {
                println!("{}", cli_help_text());
                std::process::exit(0);
            }
            other => {
                return Err(ScriptLangError::message(format!(
                    "unknown argument `{other}`\n{}",
                    cli_help_text()
                )));
            }
        }
    }

    if !commands.is_empty() && file.is_some() {
        return Err(ScriptLangError::message(
            "cannot combine `--command` and `--file` modes",
        ));
    }

    if let Some(path) = file {
        Ok(CliMode::File(path))
    } else if commands.is_empty() {
        Ok(CliMode::Interactive)
    } else {
        Ok(CliMode::Commands(commands))
    }
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

fn submit_and_print(session: &mut ReplSession, input: &str) {
    match session.submit_xml(input) {
        Ok(result) => print_submission(result),
        Err(error) => eprintln!("error: {error}"),
    }
}

fn print_submission(result: SubmissionResult) {
    match result {
        SubmissionResult::ContextUpdated => println!("context updated"),
        SubmissionResult::ModuleUpdated { module_name } => {
            println!("module {module_name} updated");
        }
        SubmissionResult::Executed(execution) => print_execution(&execution),
    }
}

fn cli_help_text() -> &'static str {
    "usage: sl-repl [--command INPUT ...] [--file PATH]\n\nmodes:\n  no args               start interactive repl\n  -c, --command INPUT   execute one repl input, may be repeated\n  -f, --file PATH       execute repl inputs from a transcript file\n  -h, --help            show this help"
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

#[cfg(test)]
mod tests {
    use super::{CliMode, parse_cli_mode, run_transcript};
    use sl_repl::ReplSession;
    use std::path::PathBuf;

    #[test]
    fn parse_cli_mode_supports_command_and_file_variants() {
        let command_mode = parse_cli_mode(
            ["--command", "<text>hi</text>"]
                .into_iter()
                .map(str::to_string),
        )
        .expect("command mode should parse");
        match command_mode {
            CliMode::Commands(commands) => assert_eq!(commands, vec!["<text>hi</text>"]),
            _ => panic!("expected command mode"),
        }

        let file_mode = parse_cli_mode(
            ["--file", "runs/session.repl"]
                .into_iter()
                .map(str::to_string),
        )
        .expect("file mode should parse");
        match file_mode {
            CliMode::File(path) => assert_eq!(path, PathBuf::from("runs/session.repl")),
            _ => panic!("expected file mode"),
        }
    }

    #[test]
    fn parse_cli_mode_rejects_mixing_command_and_file() {
        let error = parse_cli_mode(
            [
                "--command",
                "<text>hi</text>",
                "--file",
                "runs/session.repl",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .expect_err("mixed modes should fail");
        assert!(
            error
                .to_string()
                .contains("cannot combine `--command` and `--file` modes")
        );
    }

    #[test]
    fn transcript_runner_supports_commands_and_multiline_xml() {
        let mut session = ReplSession::new().expect("session should build");
        run_transcript(
            &mut session,
            ":help\n<temp name=\"hero\" type=\"int\">1</temp>\n<text>{@hero}</text>\n",
        )
        .expect("transcript should execute");

        let bindings = session.inspect(sl_repl::InspectTarget::Bindings);
        assert!(bindings.contains("hero"));
    }
}
