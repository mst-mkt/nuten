//! Not a new file renamed over the old, which would split hard links and drop the owner and mode.

use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    iter,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(crate) trait Target {
    type File: Read + Write + Seek + SetLen;

    /// Must read from the start and not cut the file short.
    fn open(&self) -> io::Result<Self::File>;

    /// The backup file must be new and empty.
    fn create_backup(&self) -> io::Result<(Self::File, PathBuf)>;

    fn remove_backup(&self, path: &Path) -> io::Result<()>;
}

/// What `Write` and `Seek` leave out of writing a file over.
pub(crate) trait SetLen {
    fn set_len(&self, len: u64) -> io::Result<()>;
}

pub(crate) struct OnDisk<'a>(pub(crate) &'a Path);

impl Target for OnDisk<'_> {
    type File = File;

    fn open(&self) -> io::Result<File> {
        OpenOptions::new().read(true).write(true).open(self.0)
    }

    fn create_backup(&self) -> io::Result<(File, PathBuf)> {
        let name = self
            .0
            .file_name()
            .ok_or_else(|| io::Error::other("no file name to name a backup by"))?;
        iter::repeat_with(|| next_backup_path(self.0, name))
            .map(|path| create_private(&path).map(|file| (file, path)))
            .find(|created| {
                !matches!(created, Err(error) if error.kind() == io::ErrorKind::AlreadyExists)
            })
            .expect("new paths go on without end")
    }

    fn remove_backup(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
}

impl SetLen for File {
    fn set_len(&self, len: u64) -> io::Result<()> {
        File::set_len(self, len)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Contents {
    Old,
    New,
    Broken,
}

#[derive(Debug)]
pub(crate) struct Failure {
    pub(crate) error: io::Error,
    pub(crate) contents: Contents,
    /// Holds all the old contents unless `contents` is `Old`.
    pub(crate) backup: Option<PathBuf>,
}

pub(crate) fn overwrite<T: Target>(target: &T, old: &[u8], new: &[u8]) -> Result<(), Failure> {
    let untouched = |error| Failure {
        error,
        contents: Contents::Old,
        backup: None,
    };
    // Opened before the backup is made, so that an unwritable file leaves nothing behind.
    let mut file = target.open().map_err(untouched)?;
    // Another writer, such as an editor, may have changed it since, and writing would lose that.
    if !holds(&mut file, old).map_err(untouched)? {
        return Err(untouched(io::Error::other(
            "changed since it was read, so it is left as it is",
        )));
    }
    let backup = match target.create_backup() {
        Ok((backup_file, path)) => Some(back_up(target, backup_file, path, old)?),
        Err(error) if forbids_backup(&error) => None,
        Err(error) => return Err(untouched(error)),
    };
    match write_over(&mut file, new) {
        Ok(()) => match backup {
            Some(path) => target.remove_backup(&path).map_err(|error| Failure {
                error,
                contents: Contents::New,
                backup: Some(path),
            }),
            None => Ok(()),
        },
        Err(error) => Err(roll_back(target, &mut file, old, backup, error)),
    }
}

/// A writable file can sit in a directory that is not, or have a name too long for a suffix.
/// Not an error such as a full disk, which writing in place could meet too and break the file.
fn forbids_backup(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::PermissionDenied
            | io::ErrorKind::ReadOnlyFilesystem
            | io::ErrorKind::InvalidFilename
    )
}

fn back_up<T: Target>(
    target: &T,
    mut backup_file: T::File,
    path: PathBuf,
    old: &[u8],
) -> Result<PathBuf, Failure> {
    let written = backup_file.write_all(old);
    // Closed before it is removed, which Windows needs.
    // Not synced to catch an error only closing reports, since a sync costs milliseconds a file.
    drop(backup_file);
    match written {
        Ok(()) => Ok(path),
        Err(error) => Err(Failure {
            error,
            contents: Contents::Old,
            backup: left_behind(target, path),
        }),
    }
}

fn roll_back<T: Target>(
    target: &T,
    file: &mut T::File,
    old: &[u8],
    backup: Option<PathBuf>,
    error: io::Error,
) -> Failure {
    if write_over(file, old).is_ok() {
        Failure {
            error,
            contents: Contents::Old,
            backup: backup.and_then(|path| left_behind(target, path)),
        }
    } else {
        Failure {
            error,
            contents: Contents::Broken,
            backup,
        }
    }
}

fn left_behind<T: Target>(target: &T, path: PathBuf) -> Option<PathBuf> {
    target.remove_backup(&path).is_err().then_some(path)
}

/// Reading one byte past `contents` is enough to see that the file grew, so no more is read.
fn holds(file: &mut impl Read, contents: &[u8]) -> io::Result<bool> {
    let limit = contents.len() + 1;
    let mut held = Vec::with_capacity(limit);
    file.take(limit as u64).read_to_end(&mut held)?;
    Ok(held == contents)
}

fn write_over(file: &mut (impl Write + Seek + SetLen), contents: &[u8]) -> io::Result<()> {
    file.rewind()?;
    file.write_all(contents)?;
    file.set_len(contents.len() as u64)
}

/// Not ending in `.nu` or `.nuon`, so that formatting the directory skips a backup left behind.
fn next_backup_path(path: &Path, name: &OsStr) -> PathBuf {
    static BACKUPS_NAMED: AtomicUsize = AtomicUsize::new(0);
    let count = BACKUPS_NAMED.fetch_add(1, Ordering::Relaxed);
    let mut backup_name = name.to_os_string();
    backup_name.push(format!(".{}.{count}.nuten-backup", process::id()));
    path.with_file_name(backup_name)
}

fn create_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    // Readable by the owner alone, since the file it backs up may be readable by no one else.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        env, fs,
        io::{self, Read, Seek, SeekFrom, Write},
        iter,
        path::{Path, PathBuf},
        process,
        rc::Rc,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::{Contents, OnDisk, SetLen, Target, overwrite};

    const OLD: &[u8] = b"ls|first # a comment longer than the new contents\n";

    /// Empty, shorter than `OLD`, and longer, so that the file is emptied, cut short, and grown.
    const NEW_CONTENTS: [&[u8]; 3] = [
        b"",
        b"ls | first\n",
        b"ls | first # a comment longer than the new contents, and longer still\n",
    ];

    const FORBIDDING: [io::ErrorKind; 3] = [
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::ReadOnlyFilesystem,
        io::ErrorKind::InvalidFilename,
    ];

    const BACKUP: &str = "backup";

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Disk {
        file: Vec<u8>,
        backup: Option<Vec<u8>>,
    }

    #[derive(Debug, Clone, Copy)]
    enum Role {
        File,
        Backup,
    }

    /// A read takes and a write puts at most two bytes.
    /// So a read comes up short, as a real one may, and a cut can fall inside a write.
    struct Fake {
        steps_taken: Cell<usize>,
        fails: Box<dyn Fn(usize) -> bool>,
        backup_error: Option<io::ErrorKind>,
        history: RefCell<Vec<Disk>>,
    }

    struct FakeFile {
        fake: Rc<Fake>,
        role: Role,
        position: usize,
    }

    impl Fake {
        fn new(fails: impl Fn(usize) -> bool + 'static) -> Rc<Fake> {
            Fake::with_backup_error(fails, None)
        }

        fn with_backup_error(
            fails: impl Fn(usize) -> bool + 'static,
            backup_error: Option<io::ErrorKind>,
        ) -> Rc<Fake> {
            let disk = Disk {
                file: OLD.to_vec(),
                backup: None,
            };
            Rc::new(Fake {
                steps_taken: Cell::new(0),
                fails: Box::new(fails),
                backup_error,
                history: RefCell::new(vec![disk]),
            })
        }

        fn disk(&self) -> Disk {
            self.history.borrow().last().unwrap().clone()
        }

        fn take_step<R>(&self, change: impl FnOnce(&mut Disk) -> R) -> io::Result<R> {
            let step = self.steps_taken.get();
            self.steps_taken.set(step + 1);
            if (self.fails)(step) {
                return Err(io::Error::other(format!("step {step} fails")));
            }
            let mut disk = self.disk();
            let result = change(&mut disk);
            self.history.borrow_mut().push(disk);
            Ok(result)
        }
    }

    impl Disk {
        fn contents_mut(&mut self, role: Role) -> &mut Vec<u8> {
            match role {
                Role::File => &mut self.file,
                Role::Backup => self.backup.as_mut().expect("a backup to write"),
            }
        }
    }

    impl Read for FakeFile {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let (role, position) = (self.role, self.position);
            let read = self.fake.take_step(|disk| {
                let rest = disk.contents_mut(role).get(position..).unwrap_or_default();
                let taken = rest.len().min(bytes.len()).min(2);
                bytes[..taken].copy_from_slice(&rest[..taken]);
                taken
            })?;
            self.position += read;
            Ok(read)
        }
    }

    impl Write for FakeFile {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let (role, position) = (self.role, self.position);
            let written = self.fake.take_step(|disk| {
                let contents = disk.contents_mut(role);
                let put = &bytes[..bytes.len().min(2)];
                let end = position + put.len();
                if contents.len() < end {
                    contents.resize(end, 0);
                }
                contents[position..end].copy_from_slice(put);
                put.len()
            })?;
            self.position += written;
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for FakeFile {
        fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
            let SeekFrom::Start(position) = to else {
                unimplemented!("seeking from anywhere but the start")
            };
            self.fake.take_step(|_| ())?;
            self.position = usize::try_from(position).unwrap();
            Ok(position)
        }
    }

    impl SetLen for FakeFile {
        fn set_len(&self, len: u64) -> io::Result<()> {
            let role = self.role;
            self.fake.take_step(|disk| {
                disk.contents_mut(role)
                    .resize(usize::try_from(len).unwrap(), 0);
            })
        }
    }

    impl Target for Rc<Fake> {
        type File = FakeFile;

        fn open(&self) -> io::Result<FakeFile> {
            self.take_step(|_| ())?;
            Ok(FakeFile {
                fake: Rc::clone(self),
                role: Role::File,
                position: 0,
            })
        }

        fn create_backup(&self) -> io::Result<(FakeFile, PathBuf)> {
            if let Some(kind) = self.backup_error {
                return Err(io::Error::from(kind));
            }
            self.take_step(|disk| disk.backup = Some(Vec::new()))?;
            let file = FakeFile {
                fake: Rc::clone(self),
                role: Role::Backup,
                position: 0,
            };
            Ok((file, PathBuf::from(BACKUP)))
        }

        fn remove_backup(&self, path: &Path) -> io::Result<()> {
            assert_eq!(path, Path::new(BACKUP));
            self.take_step(|disk| disk.backup = None)
        }
    }

    fn steps_to_write(new: &[u8], backup_error: Option<io::ErrorKind>) -> usize {
        let fake = Fake::with_backup_error(|_| false, backup_error);
        overwrite(&fake, OLD, new).unwrap();
        fake.steps_taken.get()
    }

    fn failures(steps: usize) -> impl Iterator<Item = Box<dyn Fn(usize) -> bool>> {
        (0..steps).flat_map(|at| {
            let ways: [Box<dyn Fn(usize) -> bool>; 2] = [
                Box::new(move |step| step == at),
                Box::new(move |step| step >= at),
            ];
            ways
        })
    }

    fn backup_errors() -> impl Iterator<Item = Option<io::ErrorKind>> {
        [None].into_iter().chain(FORBIDDING.map(Some))
    }

    #[test]
    fn overwrite_leaves_new_contents_and_no_backup() {
        for backup_error in backup_errors() {
            for new in NEW_CONTENTS {
                let fake = Fake::with_backup_error(|_| false, backup_error);
                overwrite(&fake, OLD, new).unwrap();
                let expected = Disk {
                    file: new.to_vec(),
                    backup: None,
                };
                assert_eq!(fake.disk(), expected, "{backup_error:?}");
            }
        }
    }

    #[test]
    fn overwrite_cut_off_at_any_step_leaves_file_whole_or_old_contents_in_backup() {
        for new in NEW_CONTENTS {
            let never: Box<dyn Fn(usize) -> bool> = Box::new(|_| false);
            for fails in iter::once(never).chain(failures(steps_to_write(new, None))) {
                let fake = Fake::new(fails);
                overwrite(&fake, OLD, new).ok();
                for disk in fake.history.borrow().iter() {
                    assert!(
                        disk.file == OLD || disk.file == new || disk.backup.as_deref() == Some(OLD),
                        "{disk:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn overwrite_failing_at_any_step_reports_what_file_and_backup_hold() {
        for backup_error in backup_errors() {
            for new in NEW_CONTENTS {
                for fails in failures(steps_to_write(new, backup_error)) {
                    let fake = Fake::with_backup_error(fails, backup_error);
                    let failure = overwrite(&fake, OLD, new).unwrap_err();
                    let disk = fake.disk();
                    match failure.contents {
                        Contents::Old => assert_eq!(disk.file, OLD),
                        Contents::New => assert_eq!(disk.file, new),
                        Contents::Broken if backup_error.is_none() => {
                            assert_eq!(disk.backup.as_deref(), Some(OLD));
                        },
                        Contents::Broken => {},
                    }
                    assert_eq!(failure.backup.is_some(), disk.backup.is_some(), "{disk:?}");
                }
            }
        }
    }

    #[test]
    fn overwrite_failing_at_single_step_leaves_file_whole_and_backup_only_if_removing_it_fails() {
        for new in NEW_CONTENTS {
            for at in 0..steps_to_write(new, None) {
                let fake = Fake::new(move |step| step == at);
                let failure = overwrite(&fake, OLD, new).unwrap_err();
                assert_ne!(failure.contents, Contents::Broken, "step {at}");
                let removing_fails = failure.contents == Contents::New;
                assert_eq!(fake.disk().backup.is_some(), removing_fails, "step {at}");
            }
        }
    }

    #[test]
    fn overwrite_without_backup_failing_at_single_step_restores_old_contents() {
        for backup_error in FORBIDDING.map(Some) {
            for new in NEW_CONTENTS {
                for at in 0..steps_to_write(new, backup_error) {
                    let fake = Fake::with_backup_error(move |step| step == at, backup_error);
                    let failure = overwrite(&fake, OLD, new).unwrap_err();
                    assert_eq!(failure.contents, Contents::Old, "step {at}");
                    assert_eq!(fake.disk().file, OLD, "step {at}");
                }
            }
        }
    }

    /// `OLD` cut short, as long as `OLD`, and `OLD` with more after it.
    /// So neither the length nor a start shared with `OLD` tells them from `OLD`.
    fn changes() -> [Vec<u8>; 3] {
        [
            OLD[..OLD.len() - 1].to_vec(),
            OLD.to_ascii_uppercase(),
            [OLD, b"ls\n"].concat(),
        ]
    }

    fn holding(file: Vec<u8>, backup_error: Option<io::ErrorKind>) -> Rc<Fake> {
        let fake = Fake::with_backup_error(|_| false, backup_error);
        fake.history.borrow_mut().push(Disk {
            file,
            backup: None,
        });
        fake
    }

    #[test]
    fn overwrite_leaves_file_alone_when_it_changed_since_being_read() {
        for changed in changes() {
            for new in NEW_CONTENTS {
                let fake = holding(changed.clone(), None);
                let failure = overwrite(&fake, OLD, new).unwrap_err();
                assert_eq!(failure.contents, Contents::Old);
                assert_eq!(failure.backup, None);
                let expected = Disk {
                    file: changed.clone(),
                    backup: None,
                };
                assert_eq!(fake.disk(), expected);
            }
        }
    }

    #[test]
    fn overwrite_leaves_file_alone_when_disk_is_too_full_for_backup() {
        let fake = Fake::with_backup_error(|_| false, Some(io::ErrorKind::StorageFull));
        let failure = overwrite(&fake, OLD, NEW_CONTENTS[1]).unwrap_err();
        assert_eq!(failure.error.kind(), io::ErrorKind::StorageFull);
        assert_eq!(failure.contents, Contents::Old);
        assert_eq!(failure.backup, None);
        let untouched = Disk {
            file: OLD.to_vec(),
            backup: None,
        };
        assert!(fake.history.borrow().iter().all(|disk| *disk == untouched));
        // A file as long as `OLD` that fails the check takes as many steps to read.
        let changed = holding(OLD.to_ascii_uppercase(), None);
        overwrite(&changed, OLD, NEW_CONTENTS[1]).unwrap_err();
        let checked = changed.steps_taken.get();
        assert_eq!(fake.steps_taken.get(), checked, "no step past checking");
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            static COUNT: AtomicUsize = AtomicUsize::new(0);
            let count = COUNT.fetch_add(1, Ordering::Relaxed);
            let dir = env::temp_dir().join(format!("nuten-overwrite-{}-{count}", process::id()));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn on_disk_file_opens_readable_without_being_cut_short_or_created() {
        let dir = TempDir::new();
        let path = dir.0.join("a.nu");
        fs::write(&path, OLD).unwrap();
        let mut held = Vec::new();
        OnDisk(&path)
            .open()
            .unwrap()
            .read_to_end(&mut held)
            .unwrap();
        assert_eq!(held, OLD);
        assert_eq!(fs::read(&path).unwrap(), OLD);
        let missing = dir.0.join("missing.nu");
        let error = OnDisk(&missing).open().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert!(!missing.exists());
    }

    #[test]
    fn on_disk_backups_are_distinct_new_empty_files_beside_file_not_ending_in_nu() {
        let dir = TempDir::new();
        let path = dir.0.join("a.nu");
        fs::write(&path, OLD).unwrap();
        let target = OnDisk(&path);
        let (first, first_path) = target.create_backup().unwrap();
        let (second, second_path) = target.create_backup().unwrap();
        drop((first, second));
        assert_ne!(first_path, second_path);
        for backup in [&first_path, &second_path] {
            assert_eq!(backup.parent(), path.parent());
            assert_eq!(backup.extension().unwrap(), "nuten-backup");
            assert_eq!(fs::read(backup).unwrap(), b"");
        }
    }

    #[cfg(unix)]
    #[test]
    fn on_disk_backup_is_readable_by_owner_alone() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new();
        let path = dir.0.join("a.nu");
        fs::write(&path, OLD).unwrap();
        let (_, backup) = OnDisk(&path).create_backup().unwrap();
        let mode = fs::metadata(backup).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
