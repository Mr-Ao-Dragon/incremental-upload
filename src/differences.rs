pub struct Differences {
    pub old_files: Vec<String>,
    pub old_folders: Vec<String>,
    pub new_files: Vec<String>,
    pub new_folders: Vec<String>,
}

impl Differences {
    pub fn new() -> Differences {
        Differences { 
            old_files: Vec::new(), 
            old_folders: Vec::new(), 
            new_files: Vec::new(), 
            new_folders: Vec::new(),
        }
    }

    pub fn has_differences(&self) -> bool {
        self.old_files.len() +
        self.old_folders.len() +
        self.new_files.len() +
        self.new_folders.len() > 0
    }
}