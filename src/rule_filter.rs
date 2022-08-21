use std::io::Error;
use std::io::ErrorKind;

use regex::Regex;

use crate::AppResult;

pub struct RuleFilter {
    pub filters: Vec<(Regex, bool)>
}

impl RuleFilter {
    pub fn new(rules: &Vec<String>) -> AppResult<RuleFilter> {
        // 预编译正则表达式
        let mut regexes_compiled = Vec::<(Regex, bool)>::new();
        for pattern in rules {
            let mut pattern = pattern.to_string();
            let reversed = pattern.starts_with("!");
            if reversed {
                pattern = ((&pattern)[1..]).to_owned();
            }
            let pat = Regex::new(&pattern);
            if pat.is_err() {
                let msg = pat.err().unwrap().to_string() + " (all single-backslashes may be escaped as double for display purpose)";
                return Err(Box::new(Error::new(ErrorKind::InvalidInput, msg)));
            }
            regexes_compiled.push((pat.unwrap(), reversed));
        }

        Ok(RuleFilter { filters: regexes_compiled })
    }

    pub fn test_any(&self, text: &str, if_empty: bool) -> bool {
        if self.filters.is_empty() {
            return if_empty;
        }

        self.filters.iter().any(|(reg, reversed)| reg.is_match(text) != *reversed)
    }
    
    pub fn test_all(&self, text: &str, if_empty: bool) -> bool {
        if self.filters.is_empty() {
            return if_empty;
        }

        self.filters.iter().all(|(reg, reversed)| reg.is_match(text) != *reversed)
    }
}