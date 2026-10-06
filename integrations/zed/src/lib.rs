use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree};

const NUTEN: &str = "nuten";

struct Nuten;

impl zed::Extension for Nuten {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        command(language_server_id.as_ref(), worktree.which(NUTEN))
    }
}

zed::register_extension!(Nuten);

fn command(server: &str, nuten: Option<String>) -> Result<zed::Command> {
    let command = nuten.ok_or_else(|| {
        format!(
            "{NUTEN} is not found on PATH; install it, or set `lsp.{server}.binary` \
             to `{{\"path\": \"<path to nuten>\", \"arguments\": [\"lsp\"]}}`"
        )
    })?;

    Ok(zed::Command {
        command,
        args: vec!["lsp".to_string()],
        env: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::command;

    #[test]
    fn nuten_on_path_serves_lsp() {
        let command = command("nuten", Some("/usr/bin/nuten".to_string())).unwrap();
        assert_eq!(command.command, "/usr/bin/nuten");
        assert_eq!(command.args, ["lsp"]);
        assert!(command.env.is_empty());
    }

    #[test]
    fn missing_nuten_error_shows_binary_setting_of_server_with_lsp_argument() {
        let error = command("nuten-dev", None).unwrap_err();
        assert!(error.contains("`lsp.nuten-dev.binary`"), "{error}");
        assert!(error.contains(r#""arguments": ["lsp"]"#), "{error}");
    }
}
