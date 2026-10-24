use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    core::HSTRING,
};

/// Worker-only small-file transaction. The exclusive handle serializes each
/// file across processes; rollback remains authoritative until commit finishes.
pub(super) struct File<'a> {
    directory: &'a Path,
    name: &'a str,
    _lock: fs::File,
}

impl<'a> File<'a> {
    pub fn open(directory: &'a Path, name: &'a str) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .share_mode(0)
            .open(directory.join(format!("{name}.lock")))?;
        Ok(Self {
            directory,
            name,
            _lock: lock,
        })
    }

    pub fn read(&self) -> io::Result<String> {
        match read(&self.directory.join(format!("{}.rollback", self.name))) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                read(&self.directory.join(format!("{}.txt", self.name)))
            }
            result => result,
        }
    }

    pub fn commit(&self, previous: &str, desired: &str) -> io::Result<()> {
        let candidate = Temporary::write(self.directory, self.name, desired)?;
        let rollback = self.directory.join(format!("{}.rollback", self.name));
        Temporary::write(self.directory, self.name, previous)?.replace(&rollback)?;
        candidate.replace(&self.directory.join(format!("{}.txt", self.name)))?;
        fs::remove_file(rollback)
    }
}

fn read(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    fs::File::open(path)?.take(4097).read_to_string(&mut text)?;
    if text.len() > 4096 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Saved file is too large",
        ));
    }
    Ok(text)
}

struct Temporary(PathBuf);

impl Temporary {
    fn write(directory: &Path, name: &str, contents: &str) -> io::Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = directory.join(format!("{name}-{}-{stamp}.tmp", std::process::id()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let temporary = Self(path);
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        Ok(temporary)
    }

    fn replace(&self, target: &Path) -> io::Result<()> {
        let source = HSTRING::from_wide(&self.0.as_os_str().encode_wide().collect::<Vec<_>>());
        let target = HSTRING::from_wide(&target.as_os_str().encode_wide().collect::<Vec<_>>());
        // SAFETY: Owned same-directory paths outlive this synchronous worker
        // call. No native handle or mapping escapes it.
        unsafe {
            MoveFileExW(
                &source,
                &target,
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
        .map_err(|error| io::Error::other(error.to_string()))
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
