use regex::Regex;

use crate::differences::Differences;
use crate::file::File;
use crate::file_state::State;
use crate::hash_cache::HashCache;
use crate::simple_file::FileData;
use crate::simple_file::SimpleFile;

use std::io::Error;
use std::io::Result;

pub struct FileComparer<'a> {
    pub base_path: File,
    pub compare_func: Box<dyn Fn(&FileData, &File, &str, bool, &HashCache) -> bool>,
    pub hash_cache: &'a HashCache,
    pub fast_comparison: bool,
    pub filters: Vec<Regex>,
    pub differences: Differences,
}

impl FileComparer<'_> {
    pub fn new<'a, F>(base_path: &File, compare_func: F, hash_cache: &'a HashCache, fast_comparison: bool, filters: Vec<Regex>) -> FileComparer<'a>
        where F : Fn(&FileData, &File, &str, bool, &HashCache) -> bool + 'static
    {
        FileComparer { 
            base_path: base_path.clone(), 
            compare_func: Box::new(compare_func),
            hash_cache,
            fast_comparison,
            filters,
            differences: Differences::new(),
        }
    }

    /// 只扫描新增的文件(不包括被删除的)
    /// 
    /// directory: 要进行扫描的目录<br/>
    /// contrast: 用来对照的目录
    fn find_new_files(&mut self, directory: &SimpleFile, contrast: &File) -> Result<()> {
        let directory = directory.as_dir().unwrap();
        for t in contrast.files()? {
            let t = t?;

            if !directory.contains_file(t.name()) { // 文件不存在
                let sf: Option<SimpleFile> = if t.is_dir() {
                    Some(SimpleFile::from_real_directory(&t)?)
                } else if t.is_file() {
                    Some(SimpleFile::from_real_file(&t)?)
                } else {
                    None
                };

                if let Some(sf) = sf {
                    self.add_new(&sf, &t)?;
                }
            } else { // 文件存在的话要进行进一步判断
                let corresponding = directory.get_file(t.name())
                    .ok_or_else(|| Error::new(std::io::ErrorKind::NotFound, "not found: ".to_string() + t.name()))?;

                if t.is_dir() {
                    if corresponding.is_file() {
                        // 先删除旧的再获取新的
                        self.add_old(corresponding, &contrast.relativized_by(&self.base_path))?;
                        self.add_new(corresponding, &t)?;
                    } else if corresponding.is_dir() {
                        self.find_new_files(&corresponding, &t)?;
                    }
                } else {
                    if corresponding.is_file() {
                        if !(self.compare_func)(&corresponding.as_file().unwrap(), &t, &t.relativized_by(&self.base_path), self.fast_comparison, self.hash_cache) {
                            // 先删除旧的再获取新的
                            self.add_old(corresponding, &contrast.relativized_by(&self.base_path))?;
                            self.add_new(corresponding, &t)?;
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
    /// contrast: 用来对照的目录
    fn find_old_files<'a>(&mut self, directory: &SimpleFile, contrast: &File) -> Result<()> {
        for f in &directory.as_dir().unwrap().files {
            let corresponding = contrast.append(&f.name)?;

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
    /// missing: 缺失的文件对象(文件/目录)<br/>
    /// template: 对照模板(文件/目录)
    fn add_new<'a>(&mut self, missing: &SimpleFile, contrast: &File) -> Result<()> {
        if missing.is_dir() != contrast.is_dir() {
            return Err(Error::new(std::io::ErrorKind::InvalidData, "ambiguous file type"));
        }

        if let Some(missing) = missing.as_dir() {
            let folder = contrast.relativized_by(&self.base_path).to_string();

            if !self.differences.new_folders.contains(&folder) && folder != "." {
                // 过滤文件
                if self.filter(&folder) {
                    self.differences.new_folders.push(folder);
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
                        self.differences.new_files.push(path.to_string())
                    }
                }
            }
        } else if let Some(_missing) = missing.as_file() {
            let path = contrast.relativized_by(&self.base_path);
            // 过滤文件
            if self.filter(&path) {
                self.differences.new_files.push(path)
            }
        }

        Ok(())
    }

    /// 添加需要删除的文件/目录
    /// 
    /// file: 删除的文件(文件/目录)<br/>
    /// dir: file所在的目录(文件/目录)
    fn add_old<'a>(&mut self, existing: &SimpleFile, directory: &str) -> Result<()>{
        let path = directory.to_string() + (if directory.len() > 0 { "/" } else { "" }) + &existing.name;
        let path = if path.starts_with("./") { &path[2..] } else { &path[..] };

        if let Some(existing) = existing.as_dir() {
            for u in &existing.files {
                if u.is_dir() {
                    self.add_old(&u, path)?;
                } else if u.is_file() {
                    let path = path.to_string() + "/" + &u.name;
                    let path = if path.starts_with("./") { &path[2..] } else { &path[..] };

                    // 过滤文件
                    if self.filter(&path) {
                        self.differences.old_files.push(path.to_string());
                    }
                }
            }

            // 过滤文件
            if self.filter(&path) {
                self.differences.old_folders.push(path.to_string());
            }
        } else if let Some(_existing) = existing.as_file() {
            // 过滤文件
            if self.filter(&path) {
                self.differences.old_files.push(path.to_string());
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

    pub fn compare(&mut self, directory: &File, contrast: &State) -> Result<()> {
        self.find_new_files(&SimpleFile::new_directory("no_name", contrast.clone().files.files), directory)?;
        self.find_old_files(&SimpleFile::new_directory("no_name", contrast.clone().files.files), directory)?;

        Ok(())
    }

}

// impl Deref for FileComparer {
//     type Target = Differences;

//     fn deref(&self) -> &Self::Target {
//         &self.differences
//     }
// }

// impl DerefMut for FileComparer {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.differences
//     }
// }