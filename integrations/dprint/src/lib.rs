use std::{fmt::Display, ops::RangeInclusive, str::FromStr};

use dprint_core::{
    configuration::{
        ConfigKeyMap, ConfigurationDiagnostic, GlobalConfiguration, NewLineKind,
        get_nullable_value, get_unknown_property_diagnostics,
    },
    plugins::{
        CheckConfigUpdatesMessage, ConfigChange, FileMatchingInfo, FormatError, FormatResult,
        PluginInfo, PluginResolveConfigurationResult, SyncFormatRequest, SyncHostFormatRequest,
        SyncPluginHandler,
    },
};
use nuten_format::{Commas, IndentStyle, LineEnding};
use nuten_line_index::located;
use serde::{Serialize, Serializer};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    line_width: u32,
    use_tabs: bool,
    indent_width: u8,
    new_line_kind: NewLineKind,
    #[serde(serialize_with = "commas_name")]
    commas: Commas,
}

impl Configuration {
    fn format_config(&self) -> nuten_format::Config {
        nuten_format::Config {
            line_width: self.line_width,
            indent_style: if self.use_tabs {
                IndentStyle::Tab
            } else {
                IndentStyle::Space
            },
            indent_width: self.indent_width,
            line_ending: match self.new_line_kind {
                NewLineKind::Auto => LineEnding::Auto,
                NewLineKind::LineFeed => LineEnding::Lf,
                NewLineKind::CarriageReturnLineFeed => LineEnding::CrLf,
            },
            commas: self.commas,
        }
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde passes the field by reference"
)]
fn commas_name<S: Serializer>(commas: &Commas, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(commas.name())
}

#[derive(Debug)]
pub struct Nuten;

impl SyncPluginHandler<Configuration> for Nuten {
    fn resolve_config(
        &mut self,
        config: ConfigKeyMap,
        global_config: &GlobalConfiguration,
    ) -> PluginResolveConfigurationResult<Configuration> {
        let (config, diagnostics) = resolve(config, global_config);
        PluginResolveConfigurationResult {
            config,
            diagnostics,
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
            config_schema_url: concat!(
                "https://plugins.dprint.dev/mst-mkt/nuten/",
                env!("CARGO_PKG_VERSION"),
                "/schema.json"
            )
            .to_string(),
            update_url: Some("https://plugins.dprint.dev/mst-mkt/nuten/latest.json".to_string()),
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
        let formatted =
            format_text(&src, &request.config.format_config()).map_err(FormatError::new)?;
        Ok((formatted != src).then(|| formatted.into_bytes()))
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
dprint_core::generate_plugin_code!(Nuten, Nuten, Configuration);

fn resolve(
    mut config: ConfigKeyMap,
    global: &GlobalConfiguration,
) -> (Configuration, Vec<ConfigurationDiagnostic>) {
    let defaults = nuten_format::Config::default();
    let mut diagnostics = Vec::new();
    let resolved = Configuration {
        line_width: get_integer(
            &mut config,
            "lineWidth",
            global.line_width,
            defaults.line_width,
            &nuten_format::Config::LINE_WIDTHS,
            &mut diagnostics,
        ),
        use_tabs: get_setting(
            &mut config,
            "useTabs",
            global.use_tabs,
            defaults.indent_style == IndentStyle::Tab,
            &mut diagnostics,
        ),
        indent_width: get_integer(
            &mut config,
            "indentWidth",
            global.indent_width,
            defaults.indent_width,
            &nuten_format::Config::INDENT_WIDTHS,
            &mut diagnostics,
        ),
        new_line_kind: get_setting(
            &mut config,
            "newLineKind",
            global.new_line_kind,
            new_line_kind(defaults.line_ending),
            &mut diagnostics,
        ),
        commas: get_commas(&mut config, "commas", defaults.commas, &mut diagnostics),
    };
    diagnostics.extend(get_unknown_property_diagnostics(config));
    (resolved, diagnostics)
}

enum Written<T> {
    Absent,
    Unreadable,
    Value(T),
}

fn get_written<T: FromStr>(
    config: &mut ConfigKeyMap,
    key: &str,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> Written<T>
where
    T::Err: Display,
{
    let written = config.get(key).is_some_and(|value| !value.is_null());
    match get_nullable_value(config, key, diagnostics) {
        Some(value) => Written::Value(value),
        None if written => Written::Unreadable,
        None => Written::Absent,
    }
}

fn get_setting<T: FromStr>(
    config: &mut ConfigKeyMap,
    key: &str,
    global: Option<T>,
    default: T,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> T
where
    T::Err: Display,
{
    match get_written(config, key, diagnostics) {
        Written::Value(value) => value,
        Written::Unreadable => default,
        Written::Absent => global.unwrap_or(default),
    }
}

/// Read as `i64`, so that a value `T` cannot hold gets the same message as one out of `range`.
fn get_integer<T: Copy + Display + PartialOrd + Into<i64> + TryFrom<i64>>(
    config: &mut ConfigKeyMap,
    key: &str,
    global: Option<T>,
    default: T,
    range: &RangeInclusive<T>,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> T {
    let limits = format!("an integer from {} to {}", range.start(), range.end());
    let within = |value: i64| {
        T::try_from(value)
            .ok()
            .filter(|value| range.contains(value))
    };
    match (get_written::<i64>(config, key, diagnostics), global) {
        (Written::Value(value), _) => within(value).unwrap_or_else(|| {
            diagnostics.push(diagnostic(key, format!("Takes {limits}")));
            default
        }),
        (Written::Unreadable, _) | (Written::Absent, None) => default,
        (Written::Absent, Some(global)) => within(global.into()).unwrap_or_else(|| {
            let message = format!("Takes {limits}, which the global {key} of {global} is not");
            diagnostics.push(diagnostic(key, message));
            default
        }),
    }
}

fn get_commas(
    config: &mut ConfigKeyMap,
    key: &str,
    default: Commas,
    diagnostics: &mut Vec<ConfigurationDiagnostic>,
) -> Commas {
    match get_written::<String>(config, key, diagnostics) {
        Written::Value(name) => Commas::of_name(&name).unwrap_or_else(|| {
            let choices: Vec<_> = Commas::ALL
                .iter()
                .map(|commas| format!("`{}`", commas.name()))
                .collect();
            let message = format!("Takes one of {}", choices.join(", "));
            diagnostics.push(diagnostic(key, message));
            default
        }),
        Written::Unreadable | Written::Absent => default,
    }
}

fn new_line_kind(line_ending: LineEnding) -> NewLineKind {
    match line_ending {
        LineEnding::Auto => NewLineKind::Auto,
        LineEnding::Lf => NewLineKind::LineFeed,
        LineEnding::CrLf => NewLineKind::CarriageReturnLineFeed,
    }
}

fn diagnostic(key: &str, message: String) -> ConfigurationDiagnostic {
    ConfigurationDiagnostic {
        property_name: key.to_string(),
        message,
    }
}

fn format_text(src: &str, config: &nuten_format::Config) -> Result<String, String> {
    nuten_format::format(src, config).map_err(|errors| {
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
    use std::{collections::BTreeSet, path::Path};

    use dprint_core::{
        configuration::{ConfigKeyMap, ConfigKeyValue, GlobalConfiguration, NewLineKind},
        plugins::{
            CheckConfigUpdatesMessage, FileMatchingInfo, FormatConfigId, FormatRange, FormatResult,
            NullCancellationToken, PluginInfo, SyncFormatRequest, SyncPluginHandler,
        },
    };
    use nuten_format::{Commas, IndentStyle, LineEnding};
    use serde_json::{Map, Value, json};

    use super::{Configuration, Nuten, resolve};

    fn settings<const N: usize>(entries: [(&str, ConfigKeyValue); N]) -> ConfigKeyMap {
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect()
    }

    fn global_settings() -> GlobalConfiguration {
        GlobalConfiguration {
            line_width: Some(80),
            use_tabs: Some(true),
            indent_width: Some(8),
            new_line_kind: Some(NewLineKind::CarriageReturnLineFeed),
        }
    }

    fn resolved(config: ConfigKeyMap, global: &GlobalConfiguration) -> nuten_format::Config {
        resolve(config, global).0.format_config()
    }

    fn diagnostics(config: ConfigKeyMap, global: &GlobalConfiguration) -> Vec<(String, String)> {
        resolve(config, global)
            .1
            .into_iter()
            .map(|diagnostic| (diagnostic.property_name, diagnostic.message))
            .collect()
    }

    fn diagnosed_keys(config: ConfigKeyMap, global: &GlobalConfiguration) -> Vec<String> {
        diagnostics(config, global)
            .into_iter()
            .map(|(key, _)| key)
            .collect()
    }

    fn serialized(config: ConfigKeyMap) -> Map<String, Value> {
        let configuration = resolve(config, &GlobalConfiguration::default()).0;
        match serde_json::to_value(configuration).unwrap() {
            Value::Object(fields) => fields,
            value => panic!("not an object: {value}"),
        }
    }

    fn schema_properties() -> Map<String, Value> {
        let schema: Value = serde_json::from_str(include_str!("../schema.json")).unwrap();
        schema["properties"].as_object().unwrap().clone()
    }

    fn schema_choices(key: &str) -> Vec<String> {
        schema_properties()[key]["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .map(|choice| choice["const"].as_str().unwrap().to_string())
            .collect()
    }

    fn format_with(config: &Configuration, bytes: &[u8], range: FormatRange) -> FormatResult {
        let request = SyncFormatRequest {
            file_path: Path::new("script.nu"),
            file_bytes: bytes.to_vec(),
            config_id: FormatConfigId::from_raw(0),
            config,
            range,
            token: &NullCancellationToken,
        };
        Nuten.format(request, |_| unreachable!("nuten formats no embedded text"))
    }

    fn format_bytes(bytes: &[u8], range: FormatRange) -> FormatResult {
        let config = resolve(ConfigKeyMap::new(), &GlobalConfiguration::default()).0;
        format_with(&config, bytes, range)
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
    fn plugin_formats_with_resolved_settings() {
        let settings = settings([
            ("lineWidth", ConfigKeyValue::from_i32(20)),
            ("indentWidth", ConfigKeyValue::from_i32(2)),
            ("commas", ConfigKeyValue::from_str("always")),
        ]);
        let config = resolve(settings, &GlobalConfiguration::default()).0;
        let formatted = format_with(&config, b"[alpha bravo charlie]\n", None).unwrap();
        assert_eq!(
            formatted.map(|bytes| String::from_utf8(bytes).unwrap()),
            Some("[\n  alpha,\n  bravo,\n  charlie\n]\n".to_string())
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
    fn empty_config_resolves_to_defaults_of_nuten_without_diagnostics() {
        assert_eq!(
            resolved(ConfigKeyMap::new(), &GlobalConfiguration::default()),
            nuten_format::Config::default()
        );
        assert!(diagnostics(ConfigKeyMap::new(), &GlobalConfiguration::default()).is_empty());
    }

    #[test]
    fn each_setting_resolves_into_its_field_of_nuten_config() {
        let config = settings([
            ("lineWidth", ConfigKeyValue::from_i32(60)),
            ("useTabs", ConfigKeyValue::from_bool(true)),
            ("indentWidth", ConfigKeyValue::from_i32(2)),
            ("newLineKind", ConfigKeyValue::from_str("crlf")),
            ("commas", ConfigKeyValue::from_str("trailing")),
        ]);
        assert_eq!(
            resolved(config, &GlobalConfiguration::default()),
            nuten_format::Config {
                line_width: 60,
                indent_style: IndentStyle::Tab,
                indent_width: 2,
                line_ending: LineEnding::CrLf,
                commas: Commas::Trailing,
            }
        );
    }

    #[test]
    fn nuten_section_wins_over_global_settings() {
        let config = settings([
            ("lineWidth", ConfigKeyValue::from_i32(60)),
            ("useTabs", ConfigKeyValue::from_bool(false)),
            ("indentWidth", ConfigKeyValue::from_i32(2)),
            ("newLineKind", ConfigKeyValue::from_str("lf")),
        ]);
        assert_eq!(
            resolved(config, &global_settings()),
            nuten_format::Config {
                line_width: 60,
                indent_style: IndentStyle::Space,
                indent_width: 2,
                line_ending: LineEnding::Lf,
                ..nuten_format::Config::default()
            }
        );
    }

    #[test]
    fn global_settings_win_over_defaults_of_nuten() {
        assert_eq!(
            resolved(ConfigKeyMap::new(), &global_settings()),
            nuten_format::Config {
                line_width: 80,
                indent_style: IndentStyle::Tab,
                indent_width: 8,
                line_ending: LineEnding::CrLf,
                ..nuten_format::Config::default()
            }
        );
    }

    #[test]
    fn invalid_setting_is_reported_and_left_to_default_of_nuten_not_global_setting() {
        let config = settings([
            ("lineWidth", ConfigKeyValue::from_i32(0)),
            ("useTabs", ConfigKeyValue::from_str("yes")),
            ("indentWidth", ConfigKeyValue::from_i32(256)),
            ("newLineKind", ConfigKeyValue::from_str("system")),
            ("commas", ConfigKeyValue::from_str("Inline")),
        ]);
        assert_eq!(
            resolved(config.clone(), &global_settings()),
            nuten_format::Config::default()
        );
        assert_eq!(
            diagnosed_keys(config, &global_settings()),
            [
                "lineWidth",
                "useTabs",
                "indentWidth",
                "newLineKind",
                "commas"
            ]
        );
    }

    #[test]
    fn unreadable_setting_is_reported_and_left_to_default_of_nuten_not_global_setting() {
        let config = settings([
            ("lineWidth", ConfigKeyValue::from_str("x")),
            ("indentWidth", ConfigKeyValue::from_bool(true)),
            ("commas", ConfigKeyValue::Array(Vec::new())),
        ]);
        assert_eq!(
            resolved(config.clone(), &global_settings()),
            nuten_format::Config {
                indent_style: IndentStyle::Tab,
                line_ending: LineEnding::CrLf,
                ..nuten_format::Config::default()
            }
        );
        assert_eq!(
            diagnosed_keys(config, &global_settings()),
            ["lineWidth", "indentWidth", "commas"]
        );
    }

    #[test]
    fn null_setting_takes_global_setting_without_diagnostics() {
        let config = settings([
            ("lineWidth", ConfigKeyValue::Null),
            ("useTabs", ConfigKeyValue::Null),
            ("indentWidth", ConfigKeyValue::Null),
            ("newLineKind", ConfigKeyValue::Null),
            ("commas", ConfigKeyValue::Null),
        ]);
        assert_eq!(
            resolved(config.clone(), &global_settings()),
            resolved(ConfigKeyMap::new(), &global_settings())
        );
        assert!(diagnostics(config, &global_settings()).is_empty());
    }

    #[test]
    fn widths_at_ends_of_their_ranges_resolve_without_diagnostics() {
        for (line_width, indent_width) in [(1, 1), (i32::MAX, 255)] {
            let config = settings([
                ("lineWidth", ConfigKeyValue::from_i32(line_width)),
                ("indentWidth", ConfigKeyValue::from_i32(indent_width)),
            ]);
            assert!(diagnostics(config, &GlobalConfiguration::default()).is_empty());
        }
    }

    #[test]
    fn widths_outside_their_ranges_are_reported() {
        for (line_width, indent_width) in [(0, 0), (-1, 256)] {
            let config = settings([
                ("lineWidth", ConfigKeyValue::from_i32(line_width)),
                ("indentWidth", ConfigKeyValue::from_i32(indent_width)),
            ]);
            assert_eq!(
                diagnosed_keys(config, &GlobalConfiguration::default()),
                ["lineWidth", "indentWidth"]
            );
        }
    }

    #[test]
    fn diagnostics_name_range_or_choices_setting_takes() {
        let config = settings([
            ("indentWidth", ConfigKeyValue::from_i32(256)),
            ("commas", ConfigKeyValue::from_str("x")),
        ]);
        assert_eq!(
            diagnostics(config, &GlobalConfiguration::default()),
            [
                (
                    "indentWidth".to_string(),
                    "Takes an integer from 1 to 255".to_string()
                ),
                (
                    "commas".to_string(),
                    "Takes one of `none`, `inline`, `always`, `trailing`".to_string()
                ),
            ]
        );
    }

    #[test]
    fn out_of_range_global_setting_is_reported_as_global_and_left_to_default_of_nuten() {
        let global = GlobalConfiguration {
            line_width: Some(0),
            indent_width: Some(0),
            ..GlobalConfiguration::default()
        };
        assert_eq!(
            resolved(ConfigKeyMap::new(), &global),
            nuten_format::Config::default()
        );
        assert_eq!(
            diagnostics(ConfigKeyMap::new(), &global),
            [
                (
                    "lineWidth".to_string(),
                    "Takes an integer from 1 to 4294967295, which the global lineWidth of 0 is not"
                        .to_string()
                ),
                (
                    "indentWidth".to_string(),
                    "Takes an integer from 1 to 255, which the global indentWidth of 0 is not"
                        .to_string()
                ),
            ]
        );
    }

    #[test]
    fn unknown_key_is_reported() {
        let config = settings([
            ("lineWidth", ConfigKeyValue::from_i32(80)),
            ("line_width", ConfigKeyValue::from_i32(80)),
        ]);
        assert_eq!(
            diagnosed_keys(config, &GlobalConfiguration::default()),
            ["line_width"]
        );
    }

    #[test]
    fn configuration_serializes_with_key_names_and_values_of_dprint() {
        let config = settings([
            ("useTabs", ConfigKeyValue::from_bool(true)),
            ("newLineKind", ConfigKeyValue::from_str("crlf")),
            ("commas", ConfigKeyValue::from_str("trailing")),
        ]);
        assert_eq!(
            Value::Object(serialized(config)),
            json!({
                "lineWidth": 100,
                "useTabs": true,
                "indentWidth": 4,
                "newLineKind": "crlf",
                "commas": "trailing",
            })
        );
    }

    #[test]
    fn schema_lists_dprint_keys_and_every_setting_plugin_resolves() {
        let resolved_keys = serialized(ConfigKeyMap::new())
            .into_iter()
            .map(|(key, _)| key);
        let dprint_keys = ["locked", "associations"].map(str::to_string);
        let expected: BTreeSet<_> = resolved_keys.chain(dprint_keys).collect();
        let keys: BTreeSet<_> = schema_properties()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn schema_defaults_are_resolved_defaults() {
        let properties = schema_properties();
        for (key, value) in serialized(ConfigKeyMap::new()) {
            assert_eq!(properties[&key]["default"], value, "{key}");
        }
    }

    #[test]
    fn schema_limits_widths_to_what_dprint_reads_as_number_and_plugin_accepts() {
        let properties = schema_properties();
        let limits = |key: &str| {
            [
                properties[key]["minimum"].as_i64().unwrap(),
                properties[key]["maximum"].as_i64().unwrap(),
            ]
        };
        let line_widths = nuten_format::Config::LINE_WIDTHS;
        let indent_widths = nuten_format::Config::INDENT_WIDTHS;
        assert_eq!(
            limits("lineWidth"),
            [
                i64::from(*line_widths.start()),
                i64::from(*line_widths.end()).min(i64::from(i32::MAX)),
            ]
        );
        assert_eq!(
            limits("indentWidth"),
            [
                i64::from(*indent_widths.start()),
                i64::from(*indent_widths.end()),
            ]
        );
    }

    #[test]
    fn schema_offers_each_choice_plugin_takes() {
        let commas: Vec<_> = Commas::ALL.iter().map(|commas| commas.name()).collect();
        assert_eq!(schema_choices("commas"), commas);
        let new_line_kinds: Vec<_> = [
            NewLineKind::Auto,
            NewLineKind::LineFeed,
            NewLineKind::CarriageReturnLineFeed,
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        assert_eq!(schema_choices("newLineKind"), new_line_kinds);
    }

    #[test]
    fn plugin_info_names_nuten_and_its_urls() {
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(
            Nuten.plugin_info(),
            PluginInfo {
                name: "nuten".to_string(),
                version: version.to_string(),
                config_key: "nuten".to_string(),
                help_url: "https://github.com/mst-mkt/nuten".to_string(),
                config_schema_url: format!(
                    "https://plugins.dprint.dev/mst-mkt/nuten/{version}/schema.json"
                ),
                update_url: Some(
                    "https://plugins.dprint.dev/mst-mkt/nuten/latest.json".to_string()
                ),
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
