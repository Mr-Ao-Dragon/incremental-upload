use std::collections::HashMap;

use yaml_rust::Yaml;
use yaml_rust::YamlLoader;

use crate::AppResult;
use crate::utils::replace_variables;

pub struct AppConfig {
    pub source_dir: String,
    pub state_file: String,
    pub overlay_mode: bool,
    pub fast_comparison: bool,
    pub use_local_state: bool,
    pub use_remote_state: bool,
    pub state_indent: u32,
    pub threads: u32,
    pub command_workdir: String,
    pub file_filters: Vec<String>,
    pub variables: HashMap<String, String>,
    pub start_up: Vec<Vec<String>>,
    pub clean_up: Vec<Vec<String>>,
    pub download_state: Vec<Vec<String>>,
    pub upload_state: Vec<Vec<String>>,
    pub delete_file: Vec<Vec<String>>,
    pub delete_dir: Vec<Vec<String>>,
    pub upload_file: Vec<Vec<String>>,
    pub upload_dir: Vec<Vec<String>>,
}

impl AppConfig {
    pub fn parse_from_yaml_string(string: String) -> AppResult<AppConfig> {
        // 读取配置文件
        let doc = YamlLoader::load_from_str(&string)?;
        let doc = (&doc[0]).clone();
        let source_dir = doc["source-dir"].as_str().expect("the config field 'source-dir' must be present").to_owned();
        let state_file = doc["state-file"].as_str().unwrap_or(".state.json").to_owned();
        let overlay_mode = doc["overlay-mode"].as_bool().unwrap_or(false);
        let fast_comparison = doc["fast-comparison"].as_bool().unwrap_or(false);
        let use_local_state = doc["use-local-state"].as_bool().unwrap_or(false);
        let use_remote_state = doc["use-remote-state"].as_bool().unwrap_or(true);
        let state_indent = doc["state-indent"].as_i64().map_or_else(|| 0, |v| v as u32);
        let threads = doc["threads"].as_i64().map_or_else(|| 1, |v| v as u32);
        let command_workdir = doc["command-workdir"].as_str().unwrap_or("").to_owned();
        let file_filters: Vec<String> = doc["file-filters"]
            .as_vec()
            .map_or_else(|| Vec::new(), |f| f.iter().map(|v| v.as_str().unwrap_or("").to_owned()).collect());
        let variables = doc["variables"].clone();
        let command_node = &doc["commands"];
        let start_up = AppConfig::parse_as_command_line(&command_node["start-up"]);
        let clean_up = AppConfig::parse_as_command_line(&command_node["clean-up"]);
        let download_state = AppConfig::parse_as_command_line(&command_node["download-state"]);
        let upload_state = AppConfig::parse_as_command_line(&command_node["upload-state"]);
        let delete_file = AppConfig::parse_as_command_line(&command_node["delete-file"]);
        let delete_dir = AppConfig::parse_as_command_line(&command_node["delete-dir"]);
        let upload_file = AppConfig::parse_as_command_line(&command_node["upload-file"]);
        let upload_dir = AppConfig::parse_as_command_line(&command_node["making-dir"]);

        // 全局变量
        let variables: HashMap<String, String> = variables.as_hash().map_or_else(|| HashMap::new(), |v| {
            v.iter().map(|e| (e.0.as_str().unwrap().to_owned(), e.1.as_str().unwrap().to_owned())).collect::<HashMap<String, String>>()
        });

        // 替换变量
        let source_dir = replace_variables(&source_dir, &variables);

        Ok(AppConfig {
            source_dir,
            state_file,
            overlay_mode,
            fast_comparison,
            use_local_state,
            use_remote_state,
            state_indent,
            threads,
            command_workdir,
            file_filters,
            variables,
            start_up,
            clean_up,
            download_state,
            upload_state,
            delete_file,
            delete_dir,
            upload_file,
            upload_dir,
        })
    }

    fn parse_as_command_line(yaml: &Yaml) -> Vec<Vec<String>> {
        if !yaml.is_array() {
            let line = yaml.as_str().unwrap_or("").to_owned();
            return if line.is_empty() { vec![] } else { vec![vec![line]] };
        }

        let yaml: &Vec<Yaml> = yaml.as_vec().unwrap();
        let mut array: Vec<Vec<String>> = Vec::new();

        for child in yaml {
            if !child.is_array() {
                let line = child.as_str().unwrap_or("").to_owned();
                if !line.is_empty() {
                    array.push(vec![line]);
                }
            } else {
                array.push(child.as_vec().unwrap().iter().map(|v| v.as_str().unwrap_or("").to_owned()).collect());
            }
        }
        
        array
    }
}