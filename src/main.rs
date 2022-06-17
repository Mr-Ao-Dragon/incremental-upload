extern crate yaml_rust;

use std::collections::HashMap;
use std::env;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::process::Command;
use clap::Arg;
use encoding_rs::UTF_8;
use json::JsonValue;
use json::object;
use regex::Regex;
use upload_to_object_storage::file::File;
use upload_to_object_storage::file_comparer::FileComparer;
use upload_to_object_storage::file_comparer::SimpleFileObject;
use upload_to_object_storage::blocking_thread_pool::BlockingThreadPool;
use yaml_rust::YamlLoader;

pub type AppResult<R> = std::result::Result<R, Box<dyn std::error::Error>>;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

struct SubprocessTask{
    subprocess: Option<Command>,
    command: String,
    prog: String,
    args: Vec<String>,
    divided: Vec<String>,
    debug: bool,
}

impl SubprocessTask {
    pub fn new(subprocess: Option<Command>, command: String, prog: String, args: Vec<String>, divided: Vec<String>, debug: bool) -> SubprocessTask {
        SubprocessTask { subprocess, command, prog, args, divided, debug }
    }

    pub fn execute(&mut self) -> Result<()> {
        if self.subprocess.is_none() {
            return Ok(());
        }

        let result = &mut self.subprocess.take().unwrap()
            .output()
            .expect(&format!("failed to execute command-line: {}", self.command)[..]);
    
        let code = result.status.code();

        match code {
            None => return Err(Error::new(ErrorKind::Interrupted, "process was terminated by a signal.")),
            Some(c) => {
                if self.debug || c != 0 {
                    let stderr = &result.stderr[..];
                    let stdout = &result.stdout[..];
                    // let stderr = GB18030.decode(stderr).0;
                    // let stdout = GB18030.decode(stdout).0;

                    let stderr = UTF_8.decode(stderr).0;
                    let stdout = UTF_8.decode(stdout).0;

                    println!("\n命令执行失败，以下是详细信息：");
                    println!("0.raw : {}", self.command);
                    println!("1.file: {:?}", self.prog);
                    println!("2.args: {:?}", self.args);
                    println!("3.last: {:?}", self.divided);

                    println!("-----stdout-----<\n{}>-----stderr-----<\n{}>----------", stdout, stderr);

                    if c != 0 {
                        return Err(Error::new(ErrorKind::Other, format!("process exited with code: {}.", c)));
                    }
                }
            }
        }
        Ok(())
    }
}

struct UTOSApplication {
    arg_source: String,
    arg_debug: bool,
    arg_dryrun: bool,
    source_dir: File,

    workdir: File,
    global_vars: HashMap<String, String>,

    config_state_file: String,
    config_overlay_mode: bool,
    config_fast_comparison: bool,
    config_use_local_state: bool,
    config_upload_state_file: bool,
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

impl<'a> UTOSApplication {
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

    fn print_metadata() {
        println!("{APP_NAME} v{VERSION}");
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
            };
        }

        let command = self.replace_variables(command, &vars);
        let workdir = self.replace_variables(workdir.path(), &vars);

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
            };
        }
        
        let mut subprocess = Command::new(prog.to_owned());

        subprocess.env("PATH", workdir.to_owned());
        subprocess.args(args.to_owned());
        subprocess.current_dir(workdir.to_owned());

        // thread::sleep(Duration::new(2, 0));
        // println!("bbb");
        // self.pool.borrow_mut().exec(move || {
        //     println!("aaa");
        //     Self::start_subprocess(subprocess, command, prog, args, divided, debug).unwrap();
        // }, false).unwrap();

        // self.pool.borrow_mut().sync_block(move || {
        //     Self::start_subprocess(subprocess, command, prog, args, divided, debug).unwrap();
        // }).unwrap();

        SubprocessTask::new(Some(subprocess), command, prog, args, divided, debug)
    }

    fn replace_variables(&self, text: &str, vars: &HashMap<String, String>) -> String {
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

    pub fn new() -> AppResult<UTOSApplication> {
        Self::print_metadata();

        let command = clap::Command::new(APP_NAME)
            .version(VERSION)
            .arg(Arg::new("source-dir")
                .help("specify source directory to upload"))
            .arg(Arg::new("config")
                .short('c')
                .long("config")
                .takes_value(true)
                .help("specify a other config file"))
            .arg(Arg::new("debug")
                .long("debug")
                .help("show command line before executing"))
            .arg(Arg::new("dry-run")
                .long("dry-run")
                .help("run but do not execute any commands actually"));
            
        let matches = command.get_matches();
    
        let arg_config = matches.value_of("config").expect("the config file muse be supplied.").to_owned();
        let arg_source = matches.value_of("source-dir").expect("source-dir must be supplied.").to_owned();
        let arg_debug = matches.is_present("debug");
        let arg_dryrun = matches.is_present("dry-run");

        // println!("arg_debug: {}, arg_dryrun: {}", arg_debug, arg_dryrun);

        // 检查参数
        let arg_source = if arg_source.ends_with("/") {
            &arg_source[0..arg_source.len() - 1]
        } else {
            &arg_source[..]
        }.to_owned();
        let source_dir = File::new(&arg_source).unwrap();
        let config_file = File::new(&arg_config).unwrap();

        // 检查参数
        if !config_file.is_file() {
            return Err(Box::new(Error::new(ErrorKind::NotFound, String::from("the config file is not a file"))))
        }

        if !source_dir.is_dir() {
            return Err(Box::new(Error::new(ErrorKind::NotFound, String::from("the source directory is not a dir"))))
        }

        let config_contents = config_file.read()?;
        let doc = (&YamlLoader::load_from_str(&config_contents)?[0]).clone();

        // 读取配置文件
        let config_state_file = doc["state-file"].as_str().map_or_else(|| ".state.json", |v| v).to_owned();
        let config_overlay_mode = doc["overlay-mode"].as_bool().map_or_else(|| false, |v| v);
        let config_fast_comparison = doc["fast-comparison"].as_bool().map_or_else(|| false, |v| v);
        let config_use_local_state = doc["use-local-state"].as_bool().map_or_else(|| false, |v| v);
        let config_threads = doc["threads"].as_i64().map_or_else(|| 1, |v| v as u32);
        let config_file_filters = doc["file-filters"]
            .as_vec()
            .map_or_else(|| Vec::new(), |v| (v).to_vec())
            .iter()
            .map(|v| v.as_str().unwrap_or("")
            .to_owned())
            .collect::<Vec<String>>();
        let config_upload_state_file = doc["use-remote-state"].as_bool().map_or_else(|| true, |v| v);
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

        // let mut thread_pool = ThreadPool::new(config_threads as usize);
        // thread_pool.toggle_blocking(true);
        // thread_pool.toggle_auto_scale(false);
        // thread_pool.set_exec_timeout(None);

        
        Ok(Self {
            arg_source,
            arg_debug,
            arg_dryrun,
            source_dir,

            workdir,
            global_vars,

            config_state_file,
            config_overlay_mode,
            config_fast_comparison,
            config_use_local_state,
            config_threads,
            config_file_filters,
            config_upload_state_file,
            
            _config_encoding: config_encoding,
            config_download_state,
            config_upload_state,
            config_delete_file,
            config_delete_dir,
            config_upload_file,
            config_upload_dir,
        })
    }

    pub fn load_state_file(&self) -> AppResult<(JsonValue, File)> {
        // 加载状态文件
        let mut replaces: HashMap<String, String> = HashMap::new();
        replaces.insert("source".to_string(), self.arg_source.to_string());
        replaces.insert("workdir".to_string(), self.workdir.path().to_string());
        self.merge_vars(&mut replaces);

        let state_file = File::new(&self.replace_variables(&self.config_state_file, &replaces)[..])?;

        if !self.config_use_local_state {
            println!("从远端更新状态文件");
            self.build_subprocess(&self.config_download_state, &replaces).execute()?;
        } else {
            println!("从本地加载状态文件")
        }

        let state = if state_file.exists() { 
            json::parse(&state_file.read().unwrap()[..])
                .expect(&format!("状态文件无法解析为Json格式: {}", state_file.path())[..])
        } else { 
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
        let update_remote_state = self.config_upload_state_file;

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
        let mut pool = BlockingThreadPool::new(self.config_threads as usize);
        let filtered_old_files = comparer.old_files
            .iter()
            .filter_map(|e| if !self.config_overlay_mode || comparer.new_files.contains(e) { Some(&e[..]) } else { None })
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
        pool.terminate_and_wait();

        // 删除目录
        let mut pool = BlockingThreadPool::new(self.config_threads as usize);
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
        pool.terminate_and_wait();

        // 创建目录
        let mut pool = BlockingThreadPool::new(self.config_threads as usize);
        let total = &comparer.new_folders.len();
        let mut done = 0;
        for f in &comparer.new_folders {
            let mut replaces: HashMap<String, String> = HashMap::new();
            replaces.insert("apath".to_string(), self.source_dir.append(&f)?.path().to_owned());
            replaces.insert("rpath".to_string(), f.to_owned());
            self.merge_vars(&mut replaces);

            done += 1;
            println!("新目录({}/{}): {}", done, total, f);
            let mut sp = self.build_subprocess(&self.config_upload_dir, &replaces);

            pool.execute(move || sp.execute().unwrap())
        }
        pool.terminate_and_wait();

        // 上传文件
        let mut pool = BlockingThreadPool::new(self.config_threads as usize);
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
        pool.terminate_and_wait();

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

fn main() -> AppResult<()> {
    UTOSApplication::new()?.main()?;
    Ok(())
}