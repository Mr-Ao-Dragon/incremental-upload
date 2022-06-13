use json::JsonValue;
use regex::Regex;

use crate::file::File;

use std::io::Error;
use std::io::Result;

#[derive(Debug)]
pub struct SimpleFileObject {
    pub name: String,
    pub length: u64,
    pub sha1: String,
    pub files: Vec<SimpleFileObject>,
    pub modified: u64,
}

impl SimpleFileObject {
    fn new(name: &str, length: u64, sha1: &str, tree: Vec<SimpleFileObject>, modified: u64,) -> Result<SimpleFileObject> {
        let sfo = SimpleFileObject {
            name: name.to_string(),
            length: length,
            sha1: sha1.to_string(),
            files: tree,
            modified: modified,
        };

        if sfo.is_file() == sfo.is_dir() {
            return Err(Error::new(std::io::ErrorKind::InvalidData, "ambiguous file type: is it actually a file or a directory?"));
        }

        Ok(sfo)
    }

    pub fn from_file(name: &str, length: u64, sha1: &str, modified: u64,) -> Result<SimpleFileObject> {
        SimpleFileObject::new(name, length, sha1, Vec::new(), modified)
    }

    pub fn from_directory(name: &str, tree: Vec<SimpleFileObject>) -> Result<SimpleFileObject> {
        SimpleFileObject::new(name, 0, "", tree, 0)
    }

    pub fn from_file_object(file: &File) -> Result<SimpleFileObject> {
        if file.is_dir() {
            let mut children = Vec::<SimpleFileObject>::new();

            for f in file.files()? {
                let v = SimpleFileObject::from_file_object(&f?)?;
                children.push(v);
            }
            
            SimpleFileObject::from_directory(file.name(), children)
        } else {
            SimpleFileObject::from_file(file.name(), file.length()?, &file.sha1()?, file.modified()?)
        }
    }

    pub fn is_dir(&self) -> bool {
        self.modified == 0
    }

    pub fn is_file(&self) -> bool {
        self.sha1.len() > 0 && self.modified > 0
    }

    pub fn get_by_name(&self, name: &str) -> Option<SimpleFileObject> {
        for f in &self.files {
            if f.name == name {
                return Some(f.clone());
            }
        }

        None
    }

    pub fn contains(&self, name: &str) -> bool {
        self.get_by_name(name).is_some()
    }

}

impl Clone for SimpleFileObject {
    fn clone(&self) -> Self {
        Self { 
            name: self.name.clone(), 
            length: self.length.clone(),
            sha1: self.sha1.clone(), 
            files: self.files.clone(), 
            modified: self.modified.clone()
        }
    }
}

pub struct FileComparer {
    pub base_path: File,
    pub compare_func: Box<dyn Fn(&SimpleFileObject, &File, &str, bool) -> bool>,
    pub fast_comparison: bool,
    pub filters: Vec<Regex>,

    pub old_files: Vec<String>,
    pub old_folders: Vec<String>,
    pub new_files: Vec<String>,
    pub new_folders: Vec<String>,
}

impl FileComparer {
    pub fn new(base_path: &File, compare_func: Box<dyn Fn(&SimpleFileObject, &File, &str, bool) -> bool>, fast_comparison: bool, filters: Vec<Regex>) -> FileComparer {
        FileComparer { 
            base_path: base_path.clone(), 
            compare_func: compare_func,
            fast_comparison,
            filters,

            old_files: Vec::new(), 
            old_folders: Vec::new(), 
            new_files: Vec::new(), 
            new_folders: Vec::new(),
        }
    }

    /// 只扫描新增的文件(不包括被删除的)
    /// 
    /// directory: 要进行扫描的目录
    /// 
    /// contrast: 用来对照的目录
    pub fn find_new_files(&mut self, directory: &SimpleFileObject, contrast: &File) -> Result<()> {
        for t in contrast.files()? {
            let t = t?;

            // 过滤文件
            // let relative_path = t.relativized_by(&self.base_path);
            // if !self.filter(&relative_path) {
            //     continue;
            // }

            if !directory.contains(t.name()) { // 文件不存在
                self.add_new(&SimpleFileObject::from_file_object(&t)?, &t)?;
            } else { // 文件存在的话要进行进一步判断
                let corresponding = directory.get_by_name(t.name())
                    .ok_or_else(|| Error::new(std::io::ErrorKind::NotFound, "not found: ".to_string() + t.name()))?;

                if t.is_dir() {
                    if corresponding.is_file() {
                        // 先删除旧的再获取新的
                        self.add_old(&corresponding, &contrast.relativized_by(&self.base_path))?;
                        self.add_new(&corresponding, &t)?;
                    } else {
                        self.find_new_files(&corresponding, &t)?;
                    }
                } else {
                    if corresponding.is_file() {
                        if !(self.compare_func)(&corresponding, &t, &t.relativized_by(&self.base_path), self.fast_comparison) {
                            // 先删除旧的再获取新的
                            self.add_old(&corresponding, &contrast.relativized_by(&self.base_path))?;
                            self.add_new(&corresponding, &t)?;
                        }
                    } else {
                        // 先删除旧的再获取新的
                        self.add_old(&corresponding, &contrast.relativized_by(&self.base_path))?;
                        self.add_new(&corresponding, &t)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// 只扫描需要删除的文件
    /// 
    /// directory: 要进行扫描的目录
    /// 
    /// contrast: 用来对照的目录
    pub fn find_old_files(&mut self, directory: &SimpleFileObject, contrast: &File) -> Result<()> {
        for f in &directory.files {
            let corresponding = contrast.append(&f.name)?;

            // 过滤文件
            // let relative_path = corresponding.relativized_by(&self.base_path);
            // if !self.filter(&relative_path) {
            //     continue;
            // }

            if corresponding.exists() {
                // 如果两边都是目录，递归并进一步判断
                if f.is_dir() && corresponding.is_dir() {
                    self.find_old_files(&f, &corresponding)?;
                }
                // 其它情况均由findMissingFiles进行处理了，这里不需要重复计算
            } else { // 如果远程端没有有这个文件，就直接删掉好了
                self.add_old(&f, &contrast.relativized_by(&self.base_path))?;
            }
        }
        Ok(())
    }

    /// 添加需要传输的文件
    /// 
    /// missing: 缺失的文件对象(文件/目录)
    /// 
    /// template: 对照模板(文件/目录)
    pub fn add_new(&mut self, missing: &SimpleFileObject, contrast: &File) -> Result<()> {
        if missing.is_dir() != contrast.is_dir() {
            return Err(Error::new(std::io::ErrorKind::InvalidData, "ambiguous file type"));
        }

        if missing.is_dir() {
            let folder = contrast.relativized_by(&self.base_path).to_string();

            if !self.new_folders.contains(&folder) && folder != "." {
                // 过滤文件
                if self.filter(&folder) {
                    self.new_folders.push(folder);
                }
            }

            for m in &missing.files {
                let corresponding = contrast.append(&m.name)?;

                if m.is_dir() {
                    self.add_new(&m, &corresponding)?;
                } else {
                    let path = corresponding.relativized_by(&self.base_path);
                    // 过滤文件
                    if self.filter(&path) {
                        self.new_files.push(path.to_string())
                    }
                }
            }
        } else {
            let path = contrast.relativized_by(&self.base_path);
            // 过滤文件
            if self.filter(&path) {
                self.new_files.push(path)
            }
        }

        Ok(())
    }

    /// 添加需要删除的文件/目录
    /// 
    /// file: 删除的文件(文件/目录)
    /// 
    /// dir: file所在的目录(文件/目录)
    pub fn add_old(&mut self, existing: &SimpleFileObject, directory: &str) -> Result<()>{
        let path = directory.to_string() + (if directory.len() > 0 { "/" } else { "" }) + &existing.name;
        let path = if path.starts_with("./") { &path[2..] } else { &path[..] };

        if existing.is_dir() {
            for u in &existing.files {
                if u.is_dir() {
                    self.add_old(&u, path)?;
                } else {
                    let path = path.to_string() + "/" + &u.name;
                    let path = if path.starts_with("./") { &path[2..] } else { &path[..] };

                    // 过滤文件
                    if self.filter(&path) {
                        self.old_files.push(path.to_string());
                    }
                }
            }

            // 过滤文件
            if self.filter(&path) {
                self.old_folders.push(path.to_string());
            }
        } else {
            // 过滤文件
            if self.filter(&path) {
                self.old_files.push(path.to_string());
            }
        }

        Ok(())
    }

    fn filter<'a>(&self, test: &str) -> bool {
        if self.filters.is_empty() {
            return true;
        }
        self.filters.iter().any(|p| p.is_match(test))
    }

    fn json_array_to_sfos(&self, directory: &JsonValue) -> Result<Vec<SimpleFileObject>> {
        let mut files: Vec<SimpleFileObject> = Vec::new();

        for f in directory.members() {
            let name = f["name"].as_str().expect("找不到 name 属性");

            if f.has_key("children") { 
                let children = self.json_array_to_sfos(&f["children"])?;
                files.push(SimpleFileObject::from_directory(name, children)?);
            } else {
                let length = f["length"].as_u64().expect("找不到 length 属性");
                let hash = f["hash"].as_str().expect("找不到 hash 属性");
                let modified = f["modified"].as_u64().expect("找不到 modified 属性");
                files.push(SimpleFileObject::from_file(name, length, hash, modified)?)
            }
        }

        Ok(files)
    }

    pub fn compare(&mut self, directory: &File, contrast: &JsonValue) -> Result<()> {
        let root_dir = SimpleFileObject::from_directory("no_name", self.json_array_to_sfos(contrast)?)?;

        self.find_new_files(&root_dir, directory)?;
        self.find_old_files(&root_dir, directory)?;

        Ok(())
    }

}