use std::fs;
use std::fs::ReadDir;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Error;
use std::io::Result;
use std::path::PathBuf;
use std::io::ErrorKind;
use std::time::SystemTime;

use hex::ToHex;
use path_absolutize::Absolutize;
use relative_path::RelativePath;
use sha1::{Sha1, Digest};

pub struct DirectoryIterator<'a>(&'a File, ReadDir);

impl DirectoryIterator<'_> {
    fn new(file_obj: &File, rd: ReadDir) -> DirectoryIterator {
        DirectoryIterator(file_obj, rd)
    }
}

impl Iterator for DirectoryIterator<'_> {
    type Item = Result<File>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.1.next().map(|f| -> Result<File> {
            let mut cloned = self.0.get_raw().clone();
            cloned.push(f?.file_name());
            Ok(File::from(cloned))
        })?)
    }
}

/// 文件工具类
pub struct File {
    pub raw: PathBuf
}

impl File {
    pub fn new(path: &str) -> File {
        let _path = path.to_owned();
        let _path = PathBuf::from(_path);
        let _path = if _path.is_absolute() { 
            _path
        } else {
            _path.absolutize().unwrap().to_path_buf()
        };

        File::check_invalid_utf8(&_path, || path.to_string()).unwrap();

        File { raw: _path }
    }

    pub fn from(pathbuf: PathBuf) -> File {
        File::check_invalid_utf8(&pathbuf, || pathbuf.to_string_lossy().to_string()).unwrap();

        File { raw: pathbuf }
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

    pub fn path(&self) -> String {
        self.raw.to_str().unwrap().replace("\\", "/")
    }

    pub fn get_raw(&self) -> &PathBuf {
        &self.raw
    }

    pub fn relative(&self, path: &File) -> String {
        path.relativized_by(self)
    }

    pub fn relativized_by(&self, base: &File) -> String {
        let base = &base.path().replace("\\", "/");
        let path = &self.path().replace("\\", "/");

        let base = RelativePath::new(base);
        let path = RelativePath::new(path);

        base.relative(path).to_string().replace("\\", "/")
    }

    pub fn mv(&self, destination: &str) -> Result<()> {
        let dest = File::new(destination);

        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("source path: ") + &self.path()
            ));
        }

        if dest.exists() {
            return Err(Error::new(
                ErrorKind::AlreadyExists, 
                String::from("destination path: ") + &dest.path()
            ));
        }

        if self.is_dir() != dest.is_dir() {
            return Err(Error::new(
                ErrorKind::PermissionDenied,
                String::from("the source and the destination has different types(is file/is dir)") + &dest.path()
            ));
        }

        fs::rename(self.path(), dest.path())?;

        Ok(())
    }

    pub fn cp(&self, destination: &str) -> Result<()> {
        let dest = File::new(destination);

        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("source path: ") + &self.path()
            ));
        }

        if dest.exists() {
            return Err(Error::new(
                ErrorKind::AlreadyExists, 
                String::from("destination path: ") + &dest.path()
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
                String::from("source path: ") + &self.path()
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
        if self.exists() {
            return Err(Error::new(
                ErrorKind::AlreadyExists, 
                String::from("failed to write file: ") + &self.path()
            ));
        }
        
        fs::write(self.path(), contents)
    }

    pub fn read(&self) -> Result<String> {
        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("failed tp open file: ") + &self.path()
            ));
        }

        fs::read_to_string(self.path())
    }

    pub fn name(&self) -> &str {
        self.raw.file_name().unwrap().to_str().unwrap()
    }

    pub fn length(&self) -> Result<u64> {
        if !self.exists() {
            return Err(Error::new(
                ErrorKind::NotFound, 
                String::from("source path: ") + &self.path()
            ));
        }

        if self.is_dir() {
            return Err(Error::new(
                ErrorKind::PermissionDenied, 
                String::from("source path: ") + &self.path()
            ));
        }

        let meta = fs::metadata(self.raw.as_path())?;
        Ok(meta.len())
    }

    pub fn parent(&self) -> Result<Option<File>> {
        let mut pathbuf = self.raw.clone();
        if pathbuf.pop() {
            Ok(Some(File::from(pathbuf)))
        } else {
            Ok(None)
        }
    }

    pub fn append(&self, relative_path_or_name: &str) -> Result<File> {
        let mut pathbuf = self.raw.clone();
        pathbuf.push(relative_path_or_name);

        File::check_invalid_utf8(&pathbuf, || relative_path_or_name.to_string())?;

        Ok(File { raw: pathbuf })
    }

    pub fn files(&self) -> Result<DirectoryIterator> {
        Ok(DirectoryIterator::new(self, fs::read_dir(self.raw.clone())?))
    }

    pub fn sha1(&self) -> Result<String> {
        let mut hasher = Sha1::new();

        let file_len = self.length()?;
        let kb = 1024;
        let mb = 1024 * 1024;
        let buffer_size = if file_len < 1 * mb {
            32 * kb
        } else if file_len < 1 * mb {
            64 * kb
        } else if file_len < 2 * mb {
            128 * kb
        } else if file_len < 4 * mb {
            256 * kb
        } else if file_len < 8 * mb {
            512 * kb
        } else if file_len < 16 * mb {
            1 * mb
        } else if file_len < 32 * mb {
            4 * mb
        } else if file_len < 64 * mb {
            8 * mb
        } else if file_len < 128 * mb {
           32  * mb
        } else if file_len < 256 * mb {
           64 * mb
        } else {
           256 * mb
        };
        
        let f = std::fs::File::open(self.path())?;
        let mut reader = BufReader::with_capacity(buffer_size.try_into().unwrap(), f);
        let mut reads;

        loop {
            let buf = reader.fill_buf()?;
            reads = buf.len();
            hasher.update(&buf[0..reads]);
            reader.consume(reads);
            if reads == 0 {
                break;
            }
        }

        let bytes = &hasher.finalize()[..];
        let sha1 = bytes.encode_hex::<String>();
        
        Ok(sha1)
    }
    
}

impl Clone for File {
    fn clone(&self) -> File {
        File { raw: self.raw.clone() }
    }
}