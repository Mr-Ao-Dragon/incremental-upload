use json::JsonValue;
use json::object;

use crate::differences::Differences;
use crate::file::File;
use crate::simple_file::DirData;
use crate::simple_file::SimpleFile;
use crate::utils::get_basename;
use crate::utils::get_dirname;

pub struct State {
    pub files: DirData
}

impl State {
    pub fn from_json_array(directory: &JsonValue) -> State {
        fn gen(directory: &JsonValue) -> Vec<SimpleFile> {
            let mut files: Vec<SimpleFile> = Vec::new();
            for f in directory.members() {
                let name = f["name"].as_str();
                if let Some(name) = name {
                    if f.has_key("children") { 
                        let children = gen(&f["children"]);
                        files.push(SimpleFile::new_directory(name, children));
                    } else {
                        let length = f["length"].as_u64();
                        let hash = f["hash"].as_str();
                        let modified = f["modified"].as_u64();
                        if length.is_some() && hash.is_some() && modified.is_some() {
                            let length = length.unwrap();
                            let hash = hash.unwrap();
                            let modified = modified.unwrap();
                            files.push(SimpleFile::new_file(name, length, hash, modified));
                        }
                    }
                }
            }
            files
        }
        
        State { files: DirData::new(gen(directory)) }
    }

    pub fn to_json_array(&self) -> JsonValue {
        fn gen(dir: &DirData) -> JsonValue {
            let mut array = JsonValue::new_array();
            for f in &dir.files {
                let fname = f.name.to_owned();
                if let Some(f) = f.as_file() {
                    array.push(object! {
                        name: fname,
                        length: f.length,
                        hash: f.sha1.to_owned(),
                        modified: f.modified,
                    }).unwrap();
                } else if let Some(f) = f.as_dir() {
                    array.push(object! {
                        name: fname,
                        children: gen(&f)
                    }).unwrap();
                }
            }

            array
        }

        gen(&self.files)
    }

    pub fn update_from_differences(&mut self, differences: &Differences, sourcedir: &File) {
        for f in &differences.old_files {
            self.files.remove_file(f);
            // println!("remove file: {}", f);
        }

        for f in &differences.old_folders {
            self.files.remove_file(f);
            // println!("remove dir: {}", f);
        }

        for f in &differences.new_folders {
            let parent = get_dirname(f);
            let filename = get_basename(f);
            // println!("new dir: {}", f);

            let dir: &mut DirData = if parent.is_some() {
                self.files.get_file_mut(parent.unwrap()).unwrap().as_dir_mut().unwrap()
            } else {
                &mut self.files
            };
            
            dir.files.push(SimpleFile::new_directory(filename, Vec::new()));
        }

        for f in &differences.new_files {
            let parent = get_dirname(f);
            let filename = get_basename(f);

            // println!("new file: {}", f);

            let dir: &mut DirData = if parent.is_some() {
                self.files.get_file_mut(parent.unwrap()).unwrap().as_dir_mut().unwrap()
            } else {
                &mut self.files
            };

            let file = sourcedir.append(f).unwrap();
            let length = file.length().unwrap();
            let sha1 = file.sha1().unwrap();
            let modified = file.modified().unwrap();

            dir.files.push(SimpleFile::new_file(filename, length, &sha1, modified))
        }
    }
}

impl Clone for State {
    fn clone(&self) -> Self {
        Self { files: self.files.clone() }
    }
}
