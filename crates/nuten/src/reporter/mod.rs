pub(crate) mod github;
pub(crate) mod gitlab;

use std::{
    io,
    path::{self, Component, Path, PathBuf},
};

/// Not canonical, since a CI service compares the path with its checkout as written.
fn absolute_as_written(path: &Path) -> io::Result<PathBuf> {
    let absolute = path::absolute(path)?;
    Ok(absolute
        .components()
        .fold(PathBuf::new(), |resolved, component| match component {
            Component::ParentDir => resolved
                .parent()
                .map_or(resolved.clone(), Path::to_path_buf),
            Component::CurDir => resolved,
            component => resolved.join(component),
        }))
}

#[cfg(test)]
mod tests {
    use std::{env, path::Path};

    use super::absolute_as_written;

    #[test]
    fn absolute_as_written_joins_current_directory_and_drops_dots() {
        let current = env::current_dir().unwrap();
        assert_eq!(
            absolute_as_written(Path::new("./a/../b.nu")).unwrap(),
            current.join("b.nu")
        );
        assert_eq!(
            absolute_as_written(Path::new("../b.nu")).unwrap(),
            current.parent().unwrap().join("b.nu")
        );
    }
}
