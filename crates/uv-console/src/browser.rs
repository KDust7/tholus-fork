use console::style;
use uv_wasm_compat::io;
use uv_wasm_compat::prompt;

fn refuse(error: prompt::PromptError) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Unsupported, error.to_string())
}

fn ask(message: &str, hint: Option<&str>, suffix: &str) {
    let mut rendered = format!(
        "{} {} {}",
        style("?".to_string()).for_stderr().yellow(),
        style(message).for_stderr().bold(),
        suffix,
    );
    if let Some(hint) = hint {
        rendered.push_str(&format!(
            "\n\n{}{} {hint}",
            style("hint").for_stderr().bold().cyan(),
            style(":").for_stderr().bold()
        ));
    }
    rendered.push('\n');
    io::stderr(&rendered);
}

fn report(message: &str, answer: &str) {
    io::stderr(&format!(
        "{} {} {} {}\n",
        style("✔".to_string()).for_stderr().green(),
        style(message).for_stderr().bold(),
        style("·").for_stderr().black().bright(),
        style(answer).for_stderr().cyan(),
    ));
}

pub(crate) fn confirm(
    message: &str,
    hint: Option<&str>,
    default: bool,
) -> std::io::Result<bool> {
    let suffix = format!(
        "{} {} {}",
        style("[y/n]").for_stderr().black().bright(),
        style("›").for_stderr().black().bright(),
        style(if default { "yes" } else { "no" }).for_stderr().cyan(),
    );
    ask(message, hint, &suffix);
    let response = prompt::confirm(message).map_err(refuse)?;
    report(message, if response { "yes" } else { "no" });
    Ok(response)
}

pub(crate) fn answer(question: &str, secret: bool) -> std::io::Result<String> {
    io::stderr(&format!("{question}\n"));
    let response = prompt::answer(question).map_err(refuse)?;
    report(question, if secret { "********" } else { &response });
    Ok(response)
}
