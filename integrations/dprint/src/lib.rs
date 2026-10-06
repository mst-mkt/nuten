use dprint_core::{
    configuration::{ConfigKeyMap, GlobalConfiguration, get_unknown_property_diagnostics},
    plugins::{
        CheckConfigUpdatesMessage, ConfigChange, FileMatchingInfo, FormatError, FormatResult,
        PluginInfo, PluginResolveConfigurationResult, SyncFormatRequest, SyncHostFormatRequest,
        SyncPluginHandler,
    },
};
use nuten_line_index::located;
use serde::Serialize;

#[expect(
    clippy::empty_structs_with_brackets,
    reason = "dprint reads the resolved config as an object, and a unit struct is `null`"
)]
#[derive(Debug, Clone, Serialize)]
pub struct Configuration {}

#[derive(Debug)]
pub struct Nuten;

impl SyncPluginHandler<Configuration> for Nuten {
    fn resolve_config(
        &mut self,
        config: ConfigKeyMap,
        _global_config: &GlobalConfiguration,
    ) -> PluginResolveConfigurationResult<Configuration> {
        PluginResolveConfigurationResult {
            config: Configuration {},
            diagnostics: get_unknown_property_diagnostics(config),
            file_matching: FileMatchingInfo {
                file_extensions: vec!["nu".to_string(), "nuon".to_string()],
                file_names: Vec::new(),
                additive: false,
            },
        }
    }

    fn plugin_info(&mut self) -> PluginInfo {
        PluginInfo {
            name: "nuten".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_key: "nuten".to_string(),
            help_url: env!("CARGO_PKG_REPOSITORY").to_string(),
            config_schema_url: String::new(),
            update_url: None,
        }
    }

    fn license_text(&mut self) -> String {
        include_str!("../../../LICENSE").to_string()
    }

    fn check_config_updates(
        &self,
        _message: CheckConfigUpdatesMessage,
    ) -> Result<Vec<ConfigChange>, FormatError> {
        Ok(Vec::new())
    }

    fn format(
        &mut self,
        request: SyncFormatRequest<'_, Configuration>,
        _format_with_host: impl FnMut(SyncHostFormatRequest<'_>) -> FormatResult,
    ) -> FormatResult {
        if request.range.is_some() {
            return Ok(None);
        }
        let src = String::from_utf8(request.file_bytes)?;
        let formatted = format_text(&src).map_err(FormatError::new)?;
        Ok((formatted != src).then(|| formatted.into_bytes()))
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
dprint_core::generate_plugin_code!(Nuten, Nuten, Configuration);

fn format_text(src: &str) -> Result<String, String> {
    nuten_format::format(src, &nuten_format::Config::default()).map_err(|errors| {
        located(
            src,
            errors
                .errors
                .iter()
                .map(|error| (error.range.start().into(), error.kind)),
        )
        .into_iter()
        .map(|(line_column, kind)| format!("{}:{}: {kind}", line_column.line, line_column.column))
        .collect::<Vec<_>>()
        .join("\n")
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use dprint_core::{
        configuration::{ConfigKeyMap, ConfigKeyValue, GlobalConfiguration},
        plugins::{
            CheckConfigUpdatesMessage, FileMatchingInfo, FormatConfigId, FormatRange, FormatResult,
            NullCancellationToken, PluginInfo, SyncFormatRequest, SyncPluginHandler,
        },
    };

    use super::{Configuration, Nuten};

    fn format_bytes(bytes: &[u8], range: FormatRange) -> FormatResult {
        let request = SyncFormatRequest {
            file_path: Path::new("script.nu"),
            file_bytes: bytes.to_vec(),
            config_id: FormatConfigId::from_raw(0),
            config: &Configuration {},
            range,
            token: &NullCancellationToken,
        };
        Nuten.format(request, |_| unreachable!("nuten formats no embedded text"))
    }

    fn formatted(text: &str) -> Option<String> {
        format_bytes(text.as_bytes(), None)
            .unwrap()
            .map(|bytes| String::from_utf8(bytes).unwrap())
    }

    fn error(text: &str) -> String {
        format_bytes(text.as_bytes(), None).unwrap_err().to_string()
    }

    fn error_positions(text: &str) -> Vec<String> {
        error(text)
            .lines()
            .map(|line| line.split_once(": ").unwrap().0.to_string())
            .collect()
    }

    fn unknown_keys(config: ConfigKeyMap) -> Vec<String> {
        Nuten
            .resolve_config(config, &GlobalConfiguration::default())
            .diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.property_name)
            .collect()
    }

    #[test]
    fn plugin_returns_formatted_text_of_file() {
        assert_eq!(formatted("ls|sort\n"), Some("ls | sort\n".to_string()));
    }

    #[test]
    fn plugin_returns_nothing_for_formatted_file() {
        assert_eq!(formatted("ls | sort\n"), None);
    }

    #[test]
    fn plugin_formats_with_default_line_width_indent_width_and_commas() {
        let word = "x".repeat(84);
        assert_eq!(formatted(&format!("[alpha, bravo, {word}]\n")), None);
        assert_eq!(
            formatted(&format!("[alpha, bravo, {word}x]\n")),
            Some(format!("[\n    alpha\n    bravo\n    {word}x\n]\n"))
        );
    }

    #[test]
    fn plugin_keeps_crlf_line_ending() {
        assert_eq!(formatted("ls|sort\r\n"), Some("ls | sort\r\n".to_string()));
    }

    #[test]
    fn plugin_formats_nothing_on_range_request() {
        assert_eq!(format_bytes(b"ls|sort\n", Some(0..2)).unwrap(), None);
    }

    #[test]
    fn plugin_error_names_line_and_column_of_each_parse_error_on_its_own_line() {
        assert_eq!(
            error_positions("ls\nfoo && bar\nfoo || bar\n"),
            ["2:5", "3:5"]
        );
    }

    #[test]
    fn plugin_error_carries_message_of_parse_error() {
        let text = "foo && bar\n";
        let errors = nuten_format::format(text, &nuten_format::Config::default()).unwrap_err();
        assert_eq!(error(text), format!("1:5: {}", errors.errors[0].kind));
    }

    #[test]
    fn plugin_fails_on_file_not_in_utf8() {
        assert!(format_bytes(&[0xff], None).is_err());
    }

    #[test]
    fn plugin_claims_files_by_nu_and_nuon_extensions_only() {
        let resolved = Nuten.resolve_config(ConfigKeyMap::new(), &GlobalConfiguration::default());
        assert_eq!(
            resolved.file_matching,
            FileMatchingInfo {
                file_extensions: vec!["nu".to_string(), "nuon".to_string()],
                file_names: Vec::new(),
                additive: false,
            }
        );
    }

    #[test]
    fn plugin_reports_no_unknown_key_for_empty_config() {
        assert!(unknown_keys(ConfigKeyMap::new()).is_empty());
    }

    #[test]
    fn plugin_reports_every_key_as_unknown() {
        let config = ConfigKeyMap::from([
            ("lineWidth".to_string(), ConfigKeyValue::from_i32(80)),
            ("commas".to_string(), ConfigKeyValue::from_str("always")),
        ]);
        assert_eq!(unknown_keys(config), ["lineWidth", "commas"]);
    }

    #[test]
    fn configuration_serializes_to_empty_object() {
        assert_eq!(serde_json::to_string(&Configuration {}).unwrap(), "{}");
    }

    #[test]
    fn plugin_info_names_nuten_with_its_version_and_repository() {
        assert_eq!(
            Nuten.plugin_info(),
            PluginInfo {
                name: "nuten".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                config_key: "nuten".to_string(),
                help_url: "https://github.com/mst-mkt/nuten".to_string(),
                config_schema_url: String::new(),
                update_url: None,
            }
        );
    }

    #[test]
    fn plugin_changes_no_config_on_update() {
        let message = CheckConfigUpdatesMessage {
            old_version: Some("0.0.0".to_string()),
            config: ConfigKeyMap::new(),
        };
        assert!(Nuten.check_config_updates(message).unwrap().is_empty());
    }

    #[test]
    fn plugin_license_is_mit_license() {
        assert!(Nuten.license_text().starts_with("MIT License\n"));
    }
}
