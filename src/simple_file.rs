use crate::file::File;
use crate::hash_cache::HashCache;
use std::io::Result;

pub struct FileData {
    pub length: u64,
    pub sha1: String,
    pub modified: u64,
}

pub struct DirData {
    pub files: Vec<SimpleFile>,
}

pub struct SimpleFile {
    pub name: String,
    file_data: Option<FileData>,
    dir_data: Option<DirData>,
}

impl SimpleFile {
    pub fn new_file(name: &str, length: u64, sha1: &str, modified: u64) -> SimpleFile {
        SimpleFile {
            name: name.to_owned(), 
            file_data: Some(FileData {
                length,
                sha1: sha1.to_owned(), 
                modified,
            }),
            dir_data: None
        }
    }

    pub fn new_directory(name: &str, files: Vec<SimpleFile>) -> SimpleFile {
        SimpleFile {
            name: name.to_owned(),
            file_data: None,
            dir_data: Some(DirData {
                files
            })
        }
    }

    pub fn from_real_file(file: &File, extra: Option<(&HashCache, &File, bool)>) -> Result<SimpleFile> {
        let hash = if let Some(extra) = extra {
            let (hash_cache, base_path, debug_mode) = extra;
            hash_cache.get_hash(&file.relativized_by(base_path), debug_mode)
        } else {
            file.sha1()?
        };
        Ok(SimpleFile::new_file(file.name(), file.length()?, &hash, file.modified()?))
    }

    pub fn from_real_directory(dir: &File, extra: Option<(&HashCache, &File, bool)>) -> Result<SimpleFile> {
        let files = dir.files()?
            .filter_map(|v| v.ok())
            .filter_map(|v: File| -> Option<SimpleFile> {
                if v.is_dir() {
                    Some(SimpleFile::from_real_directory(&v, extra).map_or_else(|_e| None, |v| Some(v))?)
                } else if v.is_file() {
                    Some(SimpleFile::from_real_file(&v, extra).map_or_else(|_e| None, |v| Some(v))?)
                } else {
                    None
                }
            }).collect::<Vec<SimpleFile>>();

        Ok(SimpleFile::new_directory(dir.name(), files))
    }

    pub fn is_file(&self) -> bool {
        self.file_data.is_some()
    }

    pub fn is_dir(&self) -> bool {
        self.dir_data.is_some()
    }

    pub fn as_file(&self) -> Option<&FileData> {
        self.file_data.as_ref()
    }

    pub fn as_dir(&self) -> Option<&DirData> {
        self.dir_data.as_ref()
    }

    pub fn as_file_mut(&mut self) -> Option<&mut FileData> {
        self.file_data.as_mut()
    }

    pub fn as_dir_mut(&mut self) -> Option<&mut DirData> {
        self.dir_data.as_mut()
    }
}

impl Clone for SimpleFile {
    fn clone(&self) -> Self {
        Self { name: self.name.clone(), file_data: self.file_data.clone(), dir_data: self.dir_data.clone() }
    }
}

impl PartialEq for SimpleFile {
    fn eq(&self, other: &Self) -> bool {
        let mut result = self.name == other.name;
        result &= self.is_file() == other.is_file() && self.is_dir() == other.is_dir();

        if result && self.is_file() { 
            let lfd = self.file_data.as_ref().unwrap();
            let rfd = other.file_data.as_ref().unwrap();
            result &= lfd == rfd;
        }

        if result && self.is_dir() { 
            let lfd = self.dir_data.as_ref().unwrap();
            let rfd = other.dir_data.as_ref().unwrap();
            result &= lfd == rfd;
        }

        result
    }
}

impl FileData {
    pub fn new(length: u64, sha1: String, modified: u64,) -> FileData {
        FileData { length, sha1, modified }
    }
}

impl Clone for FileData {
    fn clone(&self) -> Self {
        Self { length: self.length.clone(), sha1: self.sha1.clone(), modified: self.modified.clone() }
    }
}

impl PartialEq for FileData {
    fn eq(&self, other: &Self) -> bool {
        self.length == other.length && self.sha1 == other.sha1 && self.modified == other.modified
    }
}

impl DirData {
    pub fn new(files: Vec<SimpleFile>) -> DirData {
        DirData { files }
    }

    pub fn get_file(&self, relative_path: &str) -> Option<&SimpleFile> {
        let relative_path = relative_path.replace("\\", "/");
        let split = relative_path.split("/").collect::<Vec<&str>>();

        let mut current_dir: &DirData = self;
        for index in 0..split.len() {
            let name = split[index];
            let reach_end = index == split.len() - 1;

            let current = current_dir.files.iter().filter(|f| f.name == name).next()?;
            if !reach_end {
                current_dir = (*(&current.dir_data.as_ref()))?;
            } else {
                return Some(current);
            }
        }

        None
    }

    pub fn get_file_mut(&mut self, relative_path: &str) -> Option<&mut SimpleFile> {
        let relative_path = relative_path.replace("\\", "/");
        let split = relative_path.split("/").collect::<Vec<&str>>();

        let mut current_dir: &mut DirData = self;
        for index in 0..split.len() {
            let name = split[index];
            let reach_end = index == split.len() - 1;

            let current = (&mut current_dir.files).iter_mut().filter(|f| f.name == name).next().unwrap();
            if !reach_end {
                current_dir = (current.dir_data.as_mut()).unwrap();
            } else {
                return Some(current);
            }
        }

        None
    }

    pub fn remove_file(&mut self, relative_path: &str) {
        let relative_path = relative_path.replace("\\", "/");
        let split = relative_path.split("/").collect::<Vec<&str>>();

        let mut current_dir: &mut DirData = self;
        for index in 0..split.len() {
            let name = split[index];
            let reach_end = index == split.len() - 1;

            if !reach_end {
                let current = (&mut current_dir.files).iter_mut().filter(|f| f.name == name).next().unwrap();
                current_dir = (current.dir_data.as_mut()).unwrap();
            } else {
                let idx = current_dir.files.iter().position(move |v| v.name == name).unwrap();
                current_dir.files.remove(idx);
            }
        }
    }

    pub fn contains_file(&self, relative_path: &str) -> bool {
        self.get_file(relative_path).is_some()
    }
}

impl Clone for DirData {
    fn clone(&self) -> Self {
        Self { files: self.files.iter().map(|f| f.clone()).collect() }
    }
}

impl PartialEq for DirData {
    fn eq(&self, other: &Self) -> bool {
        self.files.iter().zip(&other.files).all(|p| p.0 == p.1)
    }
}

