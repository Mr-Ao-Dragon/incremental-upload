use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use crate::file::File;

pub struct HashCache {
    sourcedir: File,
    cache: Arc<Mutex<Cell<HashMap<String, String>>>>
}

impl HashCache {
    pub fn new(sourcedir: &File) -> HashCache {
        HashCache { sourcedir: sourcedir.to_owned(), cache: Arc::new(Mutex::new(Cell::new(HashMap::new()))) }
    }

    pub fn get_hash(&self, relative_path: &str, debug_mode: bool) -> String {
        let mut map = self.cache.lock().unwrap();
        let map = map.get_mut();
        
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