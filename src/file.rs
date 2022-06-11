use std::fs;
use std::fs::ReadDir;
use std::io::Error;
use std::io::Result;
use std::path::PathBuf;
use std::io::ErrorKind;
use std::time::SystemTime;

pub struct FileObjIterator<'a>(&'a FileObj, ReadDir);

impl FileObjIterator<'_> {
    fn new(file_obj: &FileObj, rd: ReadDir) -> FileObjIterator {
        FileObjIterator(file_obj, rd)
    }
}

impl Iterator for FileObjIterator<'_> {
    type Item = Result<FileObj>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.1.next().map(|f| -> Result<FileObj> {
            let mut cloned = self.0.get_raw().clone();
            cloned.push(f?.file_name());
            FileObj::from(cloned)
        })?)
    }
}

pub struct FileObj {
    pub raw: PathBuf
}

impl FileObj {
    pub fn new(path: &str) -> Result<FileObj> {
        let path_cloned = path.to_string();
        let pathbuf = PathBuf::from(path_cloned);

        FileObj::check_invalid_utf8(&pathbuf, || path.to_string())?;

        Ok(FileObj { raw: pathbuf })
    }

    pub fn from(pathbuf: PathBuf) -> Result<FileObj> {
        FileObj::check_invalid_utf8(&pathbuf, || pathbuf.to_string_lossy().to_string())?;

        Ok(FileObj { raw: pathbuf })
    }

    fn check_invalid_utf8<'a, F>(pathbuf: &PathBuf, raw_path: F) -> Result<()> where F: FnOnce() -> String {
        pathbuf.to_str().ok_or_else(|| Error::new(
            ErrorKind::NotFound, 
            "path contains invalid utf-8 chars: ".to_string() + &raw_path()
        ))?;

        Ok(())
    }

    pub fn is_dir(&self) -> bool {
        self.raw.is_dir()
    }

    pub fn is_file(&self) -> bool {
        self.raw.is_file()
    }

    pub fn is_symlink(&self) -> bool {
        self.raw.is_symlink()
    }

    pub fn exists(&self) -> bool {
        self.raw.exists()
    }

    pub fn path(&self) -> &str {
        self.raw.to_str().unwrap()
    }

    pub fn get_raw(&self) -> &PathBuf {
        &self.raw
    }

    pub fn mv(&self, destination: &str) -> Result<()> {
        let dest = FileObj::new(destination)?;

        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("source path: ") + self.path()
            ));
        }

        if dest.exists() {
            return Err(Error::new(
                ErrorKind::AlreadyExists, 
                String::from("destination path: ") + dest.path()
            ));
        }

        if self.is_dir() != dest.is_dir() {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                String::from("the source and the destination has different types(is file/is dir)") + dest.path()
            ));
        }

        fs::rename(self.path(), dest.path())?;

        Ok(())
    }

    pub fn cp(&self, destination: &str) -> Result<()> {
        let dest = FileObj::new(destination)?;

        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("source path: ") + self.path()
            ));
        }

        if dest.exists() {
            return Err(Error::new(
                ErrorKind::AlreadyExists, 
                String::from("destination path: ") + dest.path()
            ));
        }

        fs::copy(self.path(), dest.path())?;

        Ok(())
    }

    pub fn mkdirs(&self) -> Result<()> {
        if self.is_dir() {
            return Ok(());
        }

        fs::create_dir_all(self.path())
    }

    pub fn rm(&self) -> Result<()> {
        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("source path: ") + self.path()
            ));
        }

        if self.is_dir() {
            fs::remove_dir_all(self.path())?;
        } else {
            fs::remove_file(self.path())?;
        }

        Ok(())
    }

    pub fn modified(&self) -> Result<u64> {
        Ok(self.raw.metadata()?
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs())
    }

    pub fn created(&self) -> Result<u64> {
        Ok(self.raw.metadata()?
            .created()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs())
    }

    pub fn write(&self, contents: &str) -> Result<()> {
        fs::write(self.path(), contents)
    }

    pub fn read(&self) -> Result<String> {
        fs::read_to_string(self.path())
    }

    pub fn name(&self) -> &str {
        self.raw.file_name()
            .map_or_else(|| "..", |v| v.to_str().expect(""))
    }

    pub fn length(&self) -> Result<u64> {
        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("source path: ") + self.path()
            ));
        }

        if self.is_dir() {
            return Err(Error::new(
                ErrorKind::PermissionDenied, 
                String::from("source path: ") + self.path()
            ));
        }

        let meta = fs::metadata(self.raw.as_path())?;
        Ok(meta.len())
    }

    pub fn append(&self, name: &str) -> Result<FileObj> {
        let mut pathbuf = self.raw.clone();
        pathbuf.push(name);

        FileObj::check_invalid_utf8(&pathbuf, || name.to_string())?;

        Ok(FileObj { raw: pathbuf })
    }

    pub fn files(&self) -> Result<FileObjIterator> {
        Ok(FileObjIterator::new(self, fs::read_dir(self.raw.clone())?))
    }


    
}

impl Clone for FileObj {
    fn clone(&self) -> FileObj {
        FileObj { raw: self.raw.clone() }
    }
}