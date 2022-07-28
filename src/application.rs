use std::collections::HashMap;
use std::env;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::process::Command;
use clap::Arg;
use json::JsonValue;
use json::object;
use regex::Regex;
use yaml_rust::YamlLoader;

use crate::AppResult;
use crate::blocking_thread_pool::BlockingThreadPool;
use crate::file::File;
use crate::file_comparer::FileComparer;
use crate::file_comparer::SimpleFileObject;
use crate::subprocess_task::SubprocessTask;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Application {
    arg_debug: bool,
    arg_deep_debug: bool,
    arg_dryrun: bool,
    source_dir: File,

    workdir: File,
    global_vars: HashMap<String, String>,
    
    config_source_dir: String,
    config_state_file: String,
    config_overlay_mode: bool,
    config_fast_comparison: bool,
    config_use_local_state: bool,
    config_use_remote_state: bool,
    config_threads: u32,
    config_file_filters: Vec<String>,

    _config_encoding: String,
    config_download_state: String,
    config_upload_state: String,
    config_delete_file: String,
    config_delete_dir: String,
    config_upload_file: String,
    config_upload_dir: String,
}

impl Application {
    pub fn new() -> AppResult<Application> {
        let command = clap::Command::new(APP_NAME)
            .version(VERSION)
            .arg(Arg::new("config")
                .short('c')
                .long("config")
                .takes_value(true)
                .help("specify a other config file"))
            .arg(Arg::new("debug")
                .long("debug")
                .help("show command line before executing"))
            .arg(Arg::new("deep-debug")
                .long("deep-debug")
                .help("enable deep debugging info"))
            .arg(Arg::new("dry-run")
                .long("dry-run")
                .help("run but do not execute any commands actually"));
            
        let matches = command.get_matches();
    
        let arg_config = matches.value_of("config").expect("the config file muse be supplied.").to_owned();
        let arg_debug = matches.is_present("debug");
        let arg_deep_debug = matches.is_present("deep-debug");
        let arg_dryrun = matches.is_present("dry-run");

        // println!("arg_debug: {}, arg_dryrun: {}", arg_debug, arg_dryrun);

        // 检查参数
        let config_file = File::new(&arg_config).unwrap();

        // 检查参数
        if !config_file.is_file() {
            return Err(Box::new(Error::new(ErrorKind::NotFound, String::from("the config file is not a file"))))
        }

        let config_contents = config_file.read()?;
        let doc = (&YamlLoader::load_from_str(&config_contents)?[0]).clone();

        // 读取配置文件
        let config_source_dir = doc["source-dir"].as_str().expect("the config field 'source-dir must be supplied'").to_owned();
        let config_state_file = doc["state-file"].as_str().map_or_else(|| ".state.json", |v| v).to_owned();
        let config_overlay_mode = doc["overlay-mode"].as_bool().map_or_else(|| false, |v| v);
        let config_fast_comparison = doc["fast-comparison"].as_bool().map_or_else(|| false, |v| v);
        let config_use_local_state = doc["use-local-state"].as_bool().map_or_else(|| false, |v| v);
        let config_use_remote_state = doc["use-remote-state"].as_bool().map_or_else(|| true, |v| v);
        let config_threads = doc["threads"].as_i64().map_or_else(|| 1, |v| v as u32);
        let config_file_filters = doc["file-filters"]
            .as_vec()
            .map_or_else(|| Vec::new(), |v| (v).to_vec())
            .iter()
            .map(|v| v.as_str().unwrap_or("")
            .to_owned())
            .collect::<Vec<String>>();
        let config_variables = doc["variables"].clone();
        let config_command = doc["commands"].clone();
        let config_workdir = config_command["_workdir"].as_str().map_or_else(|| "", |v| v);
        let config_encoding = config_command["_encoding"].as_str().map_or_else(|| "utf-8", |v| v).to_owned();
        let config_download_state = config_command["download-state"].as_str().map_or_else(|| "", |v| v).to_owned();
        let config_upload_state = config_command["upload-state"].as_str().map_or_else(|| "", |v| v).to_owned();
        let config_delete_file = config_command["delete-file"].as_str().map_or_else(|| "", |v| v).to_owned();
        let config_delete_dir = config_command["delete-dir"].as_str().map_or_else(|| "", |v| v).to_owned();
        let config_upload_file = config_command["upload-file"].as_str().map_or_else(|| "", |v| v).to_owned();
        let config_upload_dir = config_command["make-dir"].as_str().map_or_else(|| "", |v| v).to_owned();

        // 检查参数
        let config_source_dir = if config_source_dir.ends_with("/") {
            &config_source_dir[0..config_source_dir.len() - 1]
        } else {
            &config_source_dir[..]
        }.to_owned();

        // 替换变量
        let mut temp_vars: HashMap<String, String> = HashMap::new();
        if let Some(vars) = config_variables.as_hash() {
            for (k, v) in vars {
                temp_vars.insert(k.as_str().unwrap().to_owned(), v.as_str().unwrap().to_owned());
            }
        }
        let config_source_dir = Application::replace_variables(&config_source_dir, &temp_vars);

        let source_dir = File::new(&config_source_dir).unwrap();
        if !source_dir.is_dir() {
            return Err(Box::new(Error::new(ErrorKind::NotFound, String::from(format!("the source directory is not a dir: {}", config_source_dir)))))
        }

        let workdir = (if config_workdir.len() > 0 {
            File::new(config_workdir)
        } else {
            File::from(env::current_dir().expect("field to get Current Work Directory."))
        }).expect("field to get Current Work Directory.");

        // 全局变量
        let mut global_vars: HashMap<String, String> = HashMap::new();
        global_vars.insert("asource".to_string(), source_dir.path().to_owned());
        global_vars.insert("rsource".to_string(), source_dir.relativized_by(&workdir).to_owned());
        global_vars.insert("source".to_string(), source_dir.relativized_by(&workdir).to_owned());
        if let Some(vars) = config_variables.as_hash() {
            for (k, v) in vars {
                let k = k.as_str().unwrap().to_owned();
                let v = v.as_str().unwrap().to_owned();
                global_vars.insert(k, v);
            }
        }
        if arg_debug {
            println!("全局变量: {:?}", global_vars);
        }
        
        Ok(Self {
            arg_debug,
            arg_deep_debug,
            arg_dryrun,
            source_dir,

            workdir,
            global_vars,

            config_source_dir,
            config_state_file,
            config_overlay_mode,
            config_fast_comparison,
            config_use_local_state,
            config_threads,
            config_file_filters,
            config_use_remote_state,
            
            _config_encoding: config_encoding,
            config_download_state,
            config_upload_state,
            config_delete_file,
            config_delete_dir,
            config_upload_file,
            config_upload_dir,
        })
    }

    fn command_split(&self, command: &str) -> Vec<String> {
        let mut split = Vec::<String>::new();
        let divided = command.split(" ").collect::<Vec<&str>>();
    
        let mut escaping = false;
        let mut buf = "".to_string();
    
        for d in divided {
            let s = d.starts_with("\"");
            let e = d.ends_with("\"");
    
            if s && !e {
                escaping = true;
                buf += &d[1..];
            } else if e && !s {
                escaping = false;
                buf += &(" ".to_string() + &d[..d.len() - 1]);
                split.push(buf.to_owned());
                buf.clear();
            } else {
                if escaping {
                    buf += &(" ".to_string() + d);
                } else {
                    let s_index = if s { 1 } else { 0 };
                    let e_index = if e { d.len() - 1 } else { d.len() - 0 };
                    split.push(d[s_index .. e_index].to_owned());
                }
            }
        }
    
        split
    }

    fn build_subprocess(&self, command: &str, vars: &HashMap<String, String>) -> SubprocessTask {
        let workdir: &File = &self.workdir;
        let debug: bool = self.arg_debug;
        let dry_run: bool = self.arg_dryrun;

        if command.is_empty() {
            return SubprocessTask {
                subprocess: None,
                command: "".to_string(),
                prog: "".to_string(),
                args: vec![],
                divided: vec![],
                debug: false,
                deep_debug: self.arg_deep_debug,
            };
        }

        let command = Application::replace_variables(command, &vars);
        let workdir = Application::replace_variables(workdir.path(), &vars);

        if debug {
            println!("> {}", command);
        }

        let divided = self.command_split(&command);
        let prog = divided.first().unwrap().clone();
        let args = if divided.len() > 0 { divided[1..].to_vec() } else { vec![] };

        if dry_run {
            return SubprocessTask {
                subprocess: None,
                command: "".to_string(),
                prog: "".to_string(),
                args: vec![],
                divided: vec![],
                debug: false,
                deep_debug: self.arg_deep_debug,
            };
        }
        
        let mut subprocess = Command::new(prog.to_owned());

        subprocess.env("PATH", workdir.to_owned());
        subprocess.args(args.to_owned());
        subprocess.current_dir(workdir.to_owned());

        SubprocessTask::new(Some(subprocess), command, prog, args, divided, debug, self.arg_deep_debug)
    }

    fn replace_variables(text: &str, vars: &HashMap<String, String>) -> String {
        let mut result = text.to_owned();
        let mut replaced;

        for _i in 0..1000 {
            replaced = false;

            for k in vars.keys() {
                let pattern = "$".to_string() + k;
                let new = result.replace(&pattern[..], &vars[k][..]);
                replaced |= result != new;
                result = new;
            }
            
            if !replaced {
                break;
            }
        }

        result
    }

    // 合并全局变量
    fn merge_vars(&self, maps: &mut HashMap<String, String>) {
        for (k, v) in &self.global_vars { 
            maps.insert(k.to_owned(), v.to_owned());
        }
    }

    pub fn load_state_file(&self) -> AppResult<(JsonValue, File)> {
        // 加载状态文件
        let mut replaces: HashMap<String, String> = HashMap::new();
        replaces.insert("source".to_string(), self.config_source_dir.to_string());
        replaces.insert("workdir".to_string(), self.workdir.path().to_string());
        self.merge_vars(&mut replaces);

        let state_file = File::new(&Application::replace_variables(&self.config_state_file, &replaces)[..])?;

        let state = if self.config_use_local_state || self.config_use_remote_state {
            if self.config_use_local_state {
                println!("从本地加载状态文件")
            } else if self.config_use_remote_state {
                println!("从远端更新状态文件");
                self.build_subprocess(&self.config_download_state, &replaces).execute()?;
            }

            if !state_file.exists() {
                println!("未找到任何状态文件!使用默认的空状态!");
                json::JsonValue::new_array()
            } else {
                json::parse(&state_file.read().unwrap()[..])
                .expect(&format!("状态文件无法解析为Json格式: {}", state_file.path())[..])
            }
        } else {
            println!("不加载任何状态文件!使用默认的空状态!");
            json::JsonValue::new_array()
        };
        
        Ok((state, state_file))
    }

    pub fn save_state_file(&self, comparer: &FileComparer, state_file: &File) -> AppResult<()> {
        fn walk_dir(dir: &File) -> Result<JsonValue> {
            let mut array = JsonValue::new_array();

            for f in dir.files()? {
                let f = f?;
                if f.is_file() {
                    array.push(object! {
                        name: f.name(),
                        length: f.length()?,
                        hash: f.sha1()?,
                        modified: f.modified()?
                    }).unwrap();
                } else if f.is_dir() {
                    array.push(object! {
                        name: f.name(),
                        children: walk_dir(&f)?
                    }).unwrap();
                }
            }

            Ok(array)
        }
        
        let has_differences = comparer.old_folders.len() + 
                comparer.old_files.len() + 
                comparer.new_folders.len() + 
                comparer.new_files.len() > 0;
        let update_local_state = self.config_use_local_state;
        let update_remote_state = self.config_use_remote_state;

        if has_differences && (update_local_state || update_remote_state) {
            if update_local_state {
                println!("更新本地状态文件...");
            }
            
            if state_file.exists() {
                state_file.rm()?;
            }
            
            // 计算并更新状态文件
            let state = walk_dir(&self.source_dir)?;
            let file_contents = state.pretty(4);
            state_file.parent()?.unwrap().mkdirs()?;
            state_file.write(&file_contents[..])?;

            // 更新远端状态文件
            if update_remote_state {
                println!("更新远端状态文件...");

                let mut replaces: HashMap<String, String> = HashMap::new();
                replaces.insert("apath".to_string(), state_file.path().to_owned());
                self.merge_vars(&mut replaces);
                self.build_subprocess(&self.config_upload_state, &replaces).execute()?;
            }

            // 不保留本地状态文件
            if !update_local_state {
                if state_file.exists() {
                    state_file.rm()?;
                }
            }
        }

        Ok(())
    }

    pub fn compare_files(&self, state: JsonValue) -> AppResult<FileComparer> {
        fn compare_func(remote: &SimpleFileObject, local: &File, _path: &str, fast_comparison: bool) -> bool {
            (fast_comparison && remote.modified == local.modified().map_or_else(|_e| 0, |v| v)) || 
            remote.sha1 == local.sha1().map_or_else(|_e| "".to_string(), |v| v)
        }
        
        // 预编译正则表达式
        let mut regexes_compiled = Vec::<Regex>::new();
        for pattern in &self.config_file_filters {
            let pat = Regex::new(&pattern[..]);
            if pat.is_err() {
                return Err(Box::new(Error::new(ErrorKind::InvalidInput, "fail to compile the regex: ".to_string() + &pattern)));
            }
            regexes_compiled.push(pat.unwrap());
        }
        
        // 计算差异
        let mut comparer = FileComparer::new(&self.source_dir, Box::new(compare_func), self.config_fast_comparison, regexes_compiled);
        println!("正在计算文件差异...");
        comparer.compare(&self.source_dir, &state)?;

        Ok(comparer)
    }

    pub fn execute_operations(&self, comparer: &FileComparer) -> AppResult<()> {
        println!(
            "旧文件: {}, 旧目录: {}, 新文件: {}, 新目录: {}", 
            comparer.old_files.len(),
            comparer.old_folders.len(),
            comparer.new_files.len(),
            comparer.new_folders.len(),
        );
        
        // 删除文件
        let pool = BlockingThreadPool::new(self.config_threads as usize);
        let filtered_old_files = comparer.old_files
            .iter()
            .filter_map(|e| if !self.config_overlay_mode || comparer.new_files.contains(e) { None } else { Some(&e[..]) })
            .collect::<Vec<&str>>();
        let total = filtered_old_files.len();
        let mut done = 0;
        for f in filtered_old_files {
            let mut replaces: HashMap<String, String> = HashMap::new();
            replaces.insert("rpath".to_string(), f.to_owned());
            self.merge_vars(&mut replaces);

            done += 1;
            println!("删除文件({}/{}): {}", done, total, f);
            let mut sp = self.build_subprocess(&self.config_delete_file, &replaces);

            pool.execute(move || sp.execute().unwrap())
        }
        drop(pool);

        // 删除目录
        let pool = BlockingThreadPool::new(self.config_threads as usize);
        let total = &comparer.old_folders.len();
        let mut done = 0;
        for f in &comparer.old_folders {
            let mut replaces: HashMap<String, String> = HashMap::new();
            replaces.insert("rpath".to_string(), f.to_owned());
            self.merge_vars(&mut replaces);

            done += 1;
            println!("删除目录({}/{}): {}", done, total, f);
            let mut sp = self.build_subprocess(&self.config_delete_dir, &replaces);

            pool.execute(move || sp.execute().unwrap())
        }
        drop(pool);

        // 创建目录
        let pool = BlockingThreadPool::new(self.config_threads as usize);
        let total = &comparer.new_folders.len();
        let mut done = 0;
        for f in &comparer.new_folders {
            let mut replaces: HashMap<String, String> = HashMap::new();
            replaces.insert("rpath".to_string(), f.to_owned());
            self.merge_vars(&mut replaces);

            done += 1;
            println!("新目录({}/{}): {}", done, total, f);
            let mut sp = self.build_subprocess(&self.config_upload_dir, &replaces);

            pool.execute(move || sp.execute().unwrap())
        }
        drop(pool);

        // 上传文件
        let pool = BlockingThreadPool::new(self.config_threads as usize);
        let total = &comparer.new_files.len();
        let mut done = 0;
        for f in &comparer.new_files {
            let mut replaces: HashMap<String, String> = HashMap::new();
            replaces.insert("apath".to_string(), self.source_dir.append(&f)?.path().to_owned());
            replaces.insert("rpath".to_string(), f.to_owned());
            self.merge_vars(&mut replaces);

            done += 1;
            println!("新文件({}/{}): {}", done, total, f);
            let mut sp = self.build_subprocess(&self.config_upload_file, &replaces);

            pool.execute(move || sp.execute().unwrap())
        }
        drop(pool);

        Ok(())
    }

    pub fn main(&mut self) -> AppResult<()> {
        // 加载远端文件状态
        let (state, state_file) = self.load_state_file()?;
        
        // 对比文件
        let comparer = self.compare_files(state)?;

        // 执行远端读写操作
        self.execute_operations(&comparer)?;
        
        // 更新状态文件
        self.save_state_file(&comparer, &state_file)?;

        Ok(())
    }
}