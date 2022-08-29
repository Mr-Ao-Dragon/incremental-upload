use std::cell::RefCell;
use std::collections::HashMap;

use crate::file::File;

pub struct HashCache {
    sourcedir: File,
    cache: RefCell<HashMap<String, String>>
}

impl HashCache {
    pub fn new(sourcedir: &File) -> HashCache {
        HashCache { sourcedir: sourcedir.to_owned(), cache: RefCell::new(HashMap::new()) }
    }

    pub fn get_hash(&self, relative_path: &str, debug_mode: bool) -> String {
        let mut map = self.cache.borrow_mut();
        if !map.contains_key(relative_path) {
            let file = self.sourcedir.append(relative_path).unwrap();
            if debug_mode {
                println!("hash cache miss: {}", relative_path);
            }
            map.insert(relative_path.to_owned(), file.sha1().unwrap());
        } else {
            if debug_mode {
                println!("hash cache hit: {}", relative_path);
            }
        }

        map.get(relative_path).unwrap().to_owned()
    }
}