use std::collections::HashMap;

pub struct VariableReplace {
    pub variables: HashMap<String, String>
}

impl VariableReplace {
    pub fn new() -> VariableReplace {
        VariableReplace { variables: HashMap::new() }
    }

    pub fn add(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_owned(), value.to_owned());
    }

    pub fn apply(&self, text: &str) -> String {
        let mut result = text.to_owned();
        let mut replaced;
        for _i in 0..1000 {
            replaced = false;
    
            for k in self.variables.keys() {
                let pattern = "$".to_string() + k;
                let new = result.replace(&pattern[..], &self.variables[k][..]);
                replaced |= result != new;
                result = new;
            }
            
            if !replaced {
                break;
            }
        }
        result
    }
}

impl Clone for VariableReplace {
    fn clone(&self) -> Self {
        Self { variables: self.variables.clone() }
    }
}