use std::path::PathBuf;

use lsp_types::Uri;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Place {
    /// The path may not exist yet.
    File(PathBuf),
    NoFile,
    UnknownFile,
}

pub(crate) fn of(uri: &Uri) -> Place {
    let scheme_is = |name: &str| uri.scheme().is_some_and(|scheme| scheme.eq_lowercase(name));
    if scheme_is("untitled") {
        return Place::NoFile;
    }
    // Not taken as no file, since a document of another scheme, as in a diff, may be of a file.
    if !scheme_is("file") {
        return Place::UnknownFile;
    }
    let local = uri.authority().is_none_or(|authority| {
        let host = authority.host().as_str();
        authority.userinfo().is_none()
            && authority.port().is_none()
            && (host.is_empty() || host.eq_ignore_ascii_case("localhost"))
    });
    if !local || uri.query().is_some() || uri.fragment().is_some() {
        return Place::UnknownFile;
    }
    local_path(uri.path().as_estr().decode().into_bytes().into_owned())
        .map_or(Place::UnknownFile, Place::File)
}

#[cfg(unix)]
fn local_path(bytes: Vec<u8>) -> Option<PathBuf> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let path = PathBuf::from(OsString::from_vec(bytes));
    path.is_absolute().then_some(path)
}

/// A path of Windows is written after a `/`, as in `/c:/a`.
#[cfg(not(unix))]
fn local_path(bytes: Vec<u8>) -> Option<PathBuf> {
    let text = String::from_utf8(bytes).ok()?;
    let path = PathBuf::from(text.strip_prefix('/')?);
    path.is_absolute().then_some(path)
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::PathBuf;

    use lsp_types::Uri;

    use super::{Place, of};

    fn place(uri: &str) -> Place {
        of(&uri.parse::<Uri>().unwrap())
    }

    #[test]
    fn file_uri_names_file_at_decoded_path() {
        for (uri, path) in [
            ("file:///a/b.nu", "/a/b.nu"),
            ("file://localhost/a/b.nu", "/a/b.nu"),
            ("file://LOCALHOST/a/b.nu", "/a/b.nu"),
            ("FILE:///a/b.nu", "/a/b.nu"),
            ("file:///a%20b/%E6%97%A5.nu", "/a b/日.nu"),
            ("file:///a/%25.nu", "/a/%.nu"),
        ] {
            assert_eq!(place(uri), Place::File(PathBuf::from(path)), "{uri}");
        }
    }

    #[test]
    fn file_uri_with_non_utf8_path_names_file_at_its_bytes() {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let path = PathBuf::from(OsStr::from_bytes(b"/a/\xff.nu"));
        assert_eq!(place("file:///a/%FF.nu"), Place::File(path));
    }

    #[test]
    fn untitled_uri_names_no_file() {
        for uri in ["untitled:Untitled-1", "UNTITLED:/a/b.nu"] {
            assert_eq!(place(uri), Place::NoFile, "{uri}");
        }
    }

    #[test]
    fn uri_of_other_scheme_names_unknown_file() {
        for uri in [
            "git:/a/b.nu?%7B%7D",
            "vscode-notebook-cell:/a.ipynb#x",
            "https://a/b.nu",
        ] {
            assert_eq!(place(uri), Place::UnknownFile, "{uri}");
        }
    }

    #[test]
    fn file_uri_that_names_no_local_path_names_unknown_file() {
        for uri in [
            "file://host/a/b.nu",
            "file://user@localhost/a/b.nu",
            "file://localhost:1/a/b.nu",
            "file:///a/b.nu?x",
            "file:///a/b.nu#x",
            "file:a/b.nu",
        ] {
            assert_eq!(place(uri), Place::UnknownFile, "{uri}");
        }
    }
}
