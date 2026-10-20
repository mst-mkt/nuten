use std::{
    collections::HashMap,
    io,
    num::{NonZeroU8, NonZeroU32},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    IndentStyle, LineEnding, LookupError,
    find::{holds_vcs_dir, join_into},
    glob::Glob,
    lookup::read_regular_file,
    settings::FormatChange,
};

const FILE_NAME: &str = ".editorconfig";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Editorconfig {
    root: bool,
    sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Section {
    glob: Glob,
    pairs: Vec<(String, String)>,
}

impl Editorconfig {
    /// A line it cannot read is left out rather than reported, as every core of editorconfig does.
    fn parse(text: &str) -> Editorconfig {
        let lines: Vec<&str> = text
            .strip_prefix('\u{feff}')
            .unwrap_or(text)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with([';', '#']))
            .collect();
        let headers: Vec<(usize, &str)> = lines
            .iter()
            .enumerate()
            .filter_map(|(at, line)| Some((at, section_name(line)?)))
            .collect();
        let preamble = &lines[..headers.first().map_or(lines.len(), |&(at, _)| at)];
        let ends = headers
            .iter()
            .skip(1)
            .map(|&(at, _)| at)
            .chain([lines.len()]);
        Editorconfig {
            root: preamble
                .iter()
                .rev()
                .filter_map(|line| pair(line))
                .find(|(key, _)| key == "root")
                .is_some_and(|(_, value)| value.eq_ignore_ascii_case("true")),
            sections: headers
                .iter()
                .zip(ends)
                .map(|(&(header, name), end)| Section {
                    glob: Glob::new(name),
                    pairs: lines[header + 1..end]
                        .iter()
                        .filter_map(|line| pair(line))
                        .collect(),
                })
                .collect(),
        }
    }
}

fn section_name(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']')
}

fn pair(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim().to_ascii_lowercase(), value.trim().to_string()))
}

/// The farthest first, so that a nearer file wins.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EditorconfigFiles(Arc<[DirEditorconfig]>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct DirEditorconfig {
    dir: PathBuf,
    editorconfig: Arc<Editorconfig>,
}

impl EditorconfigFiles {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn with(&self, nearer: DirEditorconfig) -> EditorconfigFiles {
        EditorconfigFiles(self.0.iter().cloned().chain([nearer]).collect())
    }

    /// `file` must be canonical.
    pub(crate) fn properties_of(&self, file: &Path) -> Properties {
        let pairs: Vec<(&str, &str)> = self
            .0
            .iter()
            .filter_map(|dir_editorconfig| {
                let path = glob_path(file.strip_prefix(&dir_editorconfig.dir).ok()?)?;
                Some((path, &dir_editorconfig.editorconfig.sections))
            })
            .flat_map(|(path, sections)| {
                sections
                    .iter()
                    .filter(move |section| section.glob.matches(&path))
            })
            .flat_map(|section| &section.pairs)
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        Properties::of(&pairs)
    }
}

/// A name that is not UTF-8 matches no section, since a section names files in UTF-8.
fn glob_path(relative: &Path) -> Option<String> {
    let names: Option<Vec<&str>> = relative
        .components()
        .map(|component| component.as_os_str().to_str())
        .collect();
    Some(names?.join("/"))
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Properties {
    pub(crate) indent_style: Option<IndentStyle>,
    indent_size: Option<IndentSize>,
    tab_width: Option<NonZeroU8>,
    end_of_line: Option<LineEnding>,
    max_line_length: Option<NonZeroU32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndentSize {
    Columns(NonZeroU8),
    Tab,
}

impl Properties {
    fn of(pairs: &[(&str, &str)]) -> Properties {
        let value = |key: &str| {
            pairs
                .iter()
                .rev()
                .find(|&&(other, _)| other == key)
                .map(|&(_, value)| value.to_ascii_lowercase())
                .filter(|value| value != "unset")
        };
        Properties {
            indent_style: value("indent_style").and_then(|style| match style.as_str() {
                "space" => Some(IndentStyle::Space),
                "tab" => Some(IndentStyle::Tab),
                _ => None,
            }),
            indent_size: value("indent_size").and_then(|size| match size.as_str() {
                "tab" => Some(IndentSize::Tab),
                size => size.parse().ok().map(IndentSize::Columns),
            }),
            tab_width: value("tab_width").and_then(|width| width.parse().ok()),
            end_of_line: value("end_of_line").and_then(|ending| match ending.as_str() {
                "lf" => Some(LineEnding::Lf),
                "crlf" => Some(LineEnding::CrLf),
                _ => None,
            }),
            max_line_length: value("max_line_length").and_then(|length| match length.as_str() {
                "off" => Some(NonZeroU32::MAX),
                length => length.parse().ok(),
            }),
        }
    }

    /// `indent_style` is the final one, after a config file and its `overrides`.
    pub(crate) fn changes(&self, indent_style: IndentStyle) -> Vec<FormatChange> {
        let columns = match self.indent_size {
            Some(IndentSize::Columns(columns)) => Some(columns),
            Some(IndentSize::Tab) => self.tab_width,
            None => None,
        };
        // `tab_width` wins with tabs, since a level is then one tab wide.
        let indent_width = match indent_style {
            IndentStyle::Tab => self.tab_width.or(columns),
            IndentStyle::Space => columns,
        };
        [
            self.indent_style.map(FormatChange::IndentStyle),
            indent_width.map(|width| FormatChange::IndentWidth(width.get())),
            self.end_of_line.map(FormatChange::LineEnding),
            self.max_line_length
                .map(|length| FormatChange::LineWidth(length.get())),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

/// An ancestor they share is read once, since a walk looks up every file.
pub(crate) fn find_each<'a>(
    dirs: impl IntoIterator<Item = &'a Path>,
) -> HashMap<&'a Path, Result<EditorconfigFiles, LookupError>> {
    // One buffer, since the search probes several names in every ancestor and each path allocates.
    let mut path = PathBuf::new();
    let mut found = HashMap::new();
    for dir in dirs {
        let searched = search(&mut path, dir, &found);
        found.extend(searched);
    }
    found
}

fn search<'a>(
    path: &mut PathBuf,
    dir: &'a Path,
    found: &HashMap<&Path, Result<EditorconfigFiles, LookupError>>,
) -> Vec<(&'a Path, Result<EditorconfigFiles, LookupError>)> {
    let Climb {
        dirs,
        above,
    } = climb(path, dir, found);
    match above {
        Ok(above) => dirs
            .into_iter()
            .rev()
            .scan(above, |editorconfig_files, (dir, own)| {
                if let Some(own) = own {
                    *editorconfig_files = editorconfig_files.with(own);
                }
                Some((dir, Ok(editorconfig_files.clone())))
            })
            .collect(),
        Err(error) => dirs
            .into_iter()
            .map(|(dir, _)| (dir, Err(error.clone())))
            .collect(),
    }
}

struct Climb<'a> {
    /// Up to where the search stops, the nearest first.
    dirs: Vec<(&'a Path, Option<DirEditorconfig>)>,
    above: Result<EditorconfigFiles, LookupError>,
}

fn climb<'a>(
    path: &mut PathBuf,
    dir: &'a Path,
    found: &HashMap<&Path, Result<EditorconfigFiles, LookupError>>,
) -> Climb<'a> {
    let mut dirs = Vec::new();
    let mut ancestors = dir.ancestors();
    let above = loop {
        let Some(ancestor) = ancestors.next() else {
            break Ok(EditorconfigFiles::default());
        };
        if let Some(known) = found.get(ancestor) {
            break known.clone();
        }
        let own = match read_in(path, ancestor) {
            Ok(own) => own,
            Err(error) => {
                dirs.push((ancestor, None));
                break Err(error);
            },
        };
        let root = own.as_ref().is_some_and(|own| own.editorconfig.root);
        dirs.push((ancestor, own));
        if root {
            break Ok(EditorconfigFiles::default());
        }
        // A file above the root would format the repository by where it is cloned.
        match holds_vcs_dir(path, ancestor) {
            Ok(false) => {},
            Ok(true) => break Ok(EditorconfigFiles::default()),
            Err(error) => break Err(error.into()),
        }
    };
    Climb {
        dirs,
        above,
    }
}

fn read_in(path: &mut PathBuf, dir: &Path) -> Result<Option<DirEditorconfig>, LookupError> {
    join_into(path, dir, FILE_NAME);
    match read_regular_file(path) {
        Ok(text) => Ok(Some(DirEditorconfig {
            dir: dir.to_path_buf(),
            editorconfig: Arc::new(Editorconfig::parse(&text)),
        })),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(LookupError::new(path, error)),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        sync::Arc,
    };

    use super::{DirEditorconfig, Editorconfig, EditorconfigFiles, Properties, find_each};
    use crate::{IndentStyle, LineEnding, settings::FormatChange};

    fn editorconfig_files(files: &[(&str, &str)]) -> EditorconfigFiles {
        EditorconfigFiles(
            files
                .iter()
                .map(|&(dir, text)| DirEditorconfig {
                    dir: dir.into(),
                    editorconfig: Arc::new(Editorconfig::parse(text)),
                })
                .collect(),
        )
    }

    fn properties(files: &[(&str, &str)], file: &str) -> Properties {
        editorconfig_files(files).properties_of(Path::new(file))
    }

    fn file_changes(files: &[(&str, &str)], file: &str) -> Vec<FormatChange> {
        properties(files, file).changes(IndentStyle::Space)
    }

    fn changes(pairs: &[(&str, &str)], indent_style: IndentStyle) -> Vec<FormatChange> {
        Properties::of(pairs).changes(indent_style)
    }

    const TWO: FormatChange = FormatChange::IndentWidth(2);
    const THREE: FormatChange = FormatChange::IndentWidth(3);

    #[test]
    fn root_is_read_only_before_first_section() {
        assert!(Editorconfig::parse("root = true\n[*]\n").root);
        assert!(!Editorconfig::parse("[*]\nroot = true\n").root);
    }

    #[test]
    fn root_value_is_read_ignoring_case() {
        assert!(Editorconfig::parse("root = TRUE\n").root);
        assert!(!Editorconfig::parse("root = false\n").root);
    }

    #[test]
    fn last_root_wins() {
        assert!(!Editorconfig::parse("root = true\nroot = false\n").root);
        assert!(Editorconfig::parse("root = false\nroot = true\n").root);
    }

    #[test]
    fn later_matching_section_wins_over_earlier_one() {
        let text = "[*]\nindent_size = 2\n[*.nu]\nindent_size = 3\n[*.sh]\nindent_size = 2\n";
        assert_eq!(file_changes(&[("/r", text)], "/r/d/x.nu"), [THREE]);
    }

    #[test]
    fn nearer_file_wins_over_farther_one() {
        let files = [
            ("/r", "[*]\nindent_size = 2\nend_of_line = lf\n"),
            ("/r/d", "[*]\nindent_size = 3\n"),
        ];
        assert_eq!(
            file_changes(&files, "/r/d/x.nu"),
            [THREE, FormatChange::LineEnding(LineEnding::Lf)]
        );
    }

    #[test]
    fn section_matches_path_from_directory_of_its_file() {
        let files = [
            ("/r", "[d/*.nu]\nindent_size = 2\n"),
            ("/r/d", "[d/*.nu]\nindent_size = 3\n"),
        ];
        assert_eq!(file_changes(&files, "/r/d/x.nu"), [TWO]);
        assert_eq!(file_changes(&files, "/r/d/d/x.nu"), [THREE]);
    }

    #[test]
    fn unset_takes_back_earlier_value() {
        let files = [
            ("/r", "[*]\nindent_size = 2\n"),
            ("/r/d", "[*]\nindent_size = UNSET\n"),
        ];
        assert_eq!(file_changes(&files, "/r/d/x.nu"), []);
    }

    #[test]
    fn value_after_unset_is_taken() {
        let text = "[*]\nindent_size = 2\nindent_size = unset\nindent_size = 3\n";
        assert_eq!(file_changes(&[("/r", text)], "/r/x.nu"), [THREE]);
    }

    #[test]
    fn keys_are_read_ignoring_case_and_pairs_trimmed() {
        let text = "[*]\n  Indent_Size =  2  \n";
        assert_eq!(file_changes(&[("/r", text)], "/r/x.nu"), [TWO]);
    }

    #[test]
    fn value_runs_to_end_of_line() {
        let text = "[*]\nindent_size = 2 = 3\nend_of_line = lf ; c\n";
        assert_eq!(file_changes(&[("/r", text)], "/r/x.nu"), []);
    }

    #[test]
    fn comment_takes_whole_line() {
        let text = "; c\n# c\n[*]\n  # indent_size = 3\nindent_size = 2\n";
        assert_eq!(file_changes(&[("/r", text)], "/r/x.nu"), [TWO]);
    }

    #[test]
    fn byte_order_mark_is_skipped() {
        let text = "\u{feff}[*]\nindent_size = 2\n";
        assert_eq!(file_changes(&[("/r", text)], "/r/x.nu"), [TWO]);
    }

    #[test]
    fn crlf_line_endings_are_read() {
        let text = "[*]\r\nindent_size = 2\r\n";
        assert_eq!(file_changes(&[("/r", text)], "/r/x.nu"), [TWO]);
    }

    #[test]
    fn line_without_section_or_pair_is_skipped() {
        let text = "[*]\nnothing\nindent_size = 2\n[unclosed\nend_of_line = lf\n";
        assert_eq!(
            file_changes(&[("/r", text)], "/r/x.nu"),
            [TWO, FormatChange::LineEnding(LineEnding::Lf)]
        );
    }

    #[test]
    fn pairs_before_any_section_hold_for_no_file() {
        assert_eq!(file_changes(&[("/r", "indent_size = 2\n")], "/r/x.nu"), []);
    }

    #[test]
    fn known_properties_give_their_changes() {
        assert_eq!(
            changes(
                &[
                    ("indent_style", "Tab"),
                    ("tab_width", "8"),
                    ("end_of_line", "CRLF"),
                    ("max_line_length", "80"),
                ],
                IndentStyle::Tab
            ),
            [
                FormatChange::IndentStyle(IndentStyle::Tab),
                FormatChange::IndentWidth(8),
                FormatChange::LineEnding(LineEnding::CrLf),
                FormatChange::LineWidth(80),
            ]
        );
    }

    #[test]
    fn indent_width_is_indent_size_with_spaces_and_tab_width_with_tabs() {
        let both = [("indent_size", "2"), ("tab_width", "8")];
        assert_eq!(changes(&both, IndentStyle::Space), [TWO]);
        assert_eq!(
            changes(&both, IndentStyle::Tab),
            [FormatChange::IndentWidth(8)]
        );
    }

    #[test]
    fn indent_width_falls_back_to_indent_size_with_tabs_without_tab_width() {
        assert_eq!(changes(&[("indent_size", "2")], IndentStyle::Tab), [TWO]);
        let pairs = [("indent_size", "2"), ("tab_width", "0")];
        assert_eq!(changes(&pairs, IndentStyle::Tab), [TWO]);
    }

    #[test]
    fn indent_size_of_tab_takes_tab_width() {
        let pairs = [("indent_size", "tab"), ("tab_width", "3")];
        assert_eq!(changes(&pairs, IndentStyle::Space), [THREE]);
        assert_eq!(changes(&[("indent_size", "tab")], IndentStyle::Space), []);
    }

    #[test]
    fn max_line_length_off_lifts_line_width_limit() {
        assert_eq!(
            changes(&[("max_line_length", "OFF")], IndentStyle::Space),
            [FormatChange::LineWidth(u32::MAX)]
        );
    }

    #[test]
    fn tab_width_alone_gives_no_indent_width_with_spaces() {
        assert_eq!(changes(&[("tab_width", "8")], IndentStyle::Space), []);
    }

    #[test]
    fn value_nuten_cannot_take_is_ignored() {
        for (key, value) in [
            ("indent_style", "both"),
            ("indent_size", "0"),
            ("indent_size", "256"),
            ("indent_size", "-1"),
            ("indent_size", "two"),
            ("end_of_line", "cr"),
            ("max_line_length", "0"),
        ] {
            assert!(
                changes(&[(key, value)], IndentStyle::Space).is_empty(),
                "{key} = {value}"
            );
        }
    }

    #[test]
    fn property_nuten_does_not_take_is_ignored() {
        for key in [
            "charset",
            "insert_final_newline",
            "trim_trailing_whitespace",
        ] {
            assert!(
                changes(&[(key, "true")], IndentStyle::Space).is_empty(),
                "{key}"
            );
        }
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> TempDir {
            let dir = env::temp_dir().join(format!("nuten-{name}-{}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            TempDir(fs::canonicalize(&dir).unwrap())
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn find_each_gives_file_of_shared_ancestor_to_each_directory() {
        let root = TempDir::new("editorconfig-shared");
        fs::create_dir_all(root.0.join(".git")).unwrap();
        fs::write(root.0.join(".editorconfig"), "[*]\nindent_size = 2\n").unwrap();
        let dirs: Vec<PathBuf> = ["a", "b", "a/c"]
            .iter()
            .map(|dir| root.0.join(dir))
            .collect();
        for dir in &dirs {
            fs::create_dir_all(dir).unwrap();
        }
        let found = find_each(dirs.iter().map(PathBuf::as_path));
        for dir in &dirs {
            let editorconfig_files = found[dir.as_path()].clone().unwrap();
            let properties = editorconfig_files.properties_of(&dir.join("x.nu"));
            assert_eq!(
                properties.changes(IndentStyle::Space),
                [TWO],
                "{}",
                dir.display()
            );
        }
    }

    #[test]
    fn find_each_gives_error_to_every_directory_below_unreadable_file() {
        let root = TempDir::new("editorconfig-unreadable");
        fs::create_dir_all(root.0.join(".git")).unwrap();
        fs::create_dir_all(root.0.join("d/.editorconfig")).unwrap();
        let dirs: Vec<PathBuf> = ["d/e", "d/f", "d"]
            .iter()
            .map(|dir| root.0.join(dir))
            .collect();
        for dir in &dirs {
            fs::create_dir_all(dir).unwrap();
        }
        let found = find_each(dirs.iter().map(PathBuf::as_path));
        for dir in &dirs {
            assert!(found[dir.as_path()].is_err(), "{}", dir.display());
        }
        assert!(find_each([root.0.as_path()])[root.0.as_path()].is_ok());
    }
}
