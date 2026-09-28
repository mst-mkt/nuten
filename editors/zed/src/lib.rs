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
