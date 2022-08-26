use std::io::Error;
use std::io::ErrorKind;
use std::process::Command;
use std::io::Result;
use encoding_rs::UTF_8;

use crate::AppResult;
use crate::file::File;
use crate::utils::command_split;
use crate::variable_replace::VariableReplace;

pub struct SubprocessResult {
    pub stdout: String,
    pub stderr: String,
    pub exitcode: i32,
}

pub struct SubprocessTask{
    pub subprocess: Command,
    pub raw_divided: Vec<String>
}

impl SubprocessTask {
    pub fn new(subprocess: Command, divided: Vec<String>) -> SubprocessTask {
        SubprocessTask { subprocess, raw_divided: divided }
    }

    pub fn from_command_line(
        command_devided: &Vec<String>, 
        workdir: &File, 
        vars: &VariableReplace, 
        last_result: Option<&SubprocessResult>
    ) -> AppResult<SubprocessTask> {
        if command_devided.is_empty() {
            return Err(Box::new(Error::new(ErrorKind::InvalidInput, "subprocess command line must be not empty")));
        }

        // apply last_result
        let mut vars = vars.clone();
        if let Some(last_result) = last_result {
            vars.add("last-stdout", &last_result.stdout);
            vars.add("last-stderr", &last_result.stderr);
            vars.add("last-exitcode", &last_result.exitcode.to_string());
        }

        // get owned
        let mut command_devided = command_devided.iter().map(|s| vars.apply(s)).collect::<Vec<String>>();

        // auto line split
        let do_not_split = command_devided[0].starts_with("+");
        if do_not_split {
            command_devided[0] = (&(command_devided[0])[1..]).to_owned();
        }

        if !do_not_split && command_devided.len() == 1 {
            command_devided = command_split(&command_devided[0]);
        }

        let prog_part = command_devided.first().unwrap().clone(); 
        let args_part = if command_devided.len() > 0 { command_devided[1..].to_vec() } else { vec![] };
        let workdir = vars.apply(&workdir.path());

        // build subprocess
        let mut subprocess = Command::new(prog_part);

        let path_separator = if cfg!(target_os = "windows") { ";" } else { ":" };
        let path = subprocess.get_envs().filter_map(|(k, v)| if k == "PATH" { 
            v.map_or_else(|| None, |value| Some(value.to_str().unwrap().to_owned()))
        } else { None }).next();
        subprocess.env("PATH", &((if path.is_some() { path.unwrap() + path_separator } else { "".to_string() }) + &workdir));
        subprocess.args(args_part);
        subprocess.current_dir(workdir);

        Ok(SubprocessTask::new(subprocess, command_devided))
    }

    pub fn execute(&mut self, show_output: bool) -> Result<SubprocessResult> {
        let result = &mut self.subprocess
            .output()
            .expect(&format!("failed to execute command-line: {:?}", self.raw_divided));
    
        let code = result.status.code();

        match code {
            None => return Err(Error::new(ErrorKind::Interrupted, "process was terminated by a signal.")),
            Some(exitcode) => {
                let stderr = &result.stderr;
                let stdout = &result.stdout;
                // let stderr = GB18030.decode(stderr).0;
                // let stdout = GB18030.decode(stdout).0;

                let stderr = UTF_8.decode(stderr).0.replace("\r\n", "\n").replace("\r", "\n").trim().replace("\n", "\n|");
                let stdout = UTF_8.decode(stdout).0.replace("\r\n", "\n").replace("\r", "\n").trim().replace("\n", "\n|");

                if exitcode != 0 {
                    println!("\n命令执行失败，返回码({})，以下是详细信息：", exitcode);
                    println!("command-line : {:?}", self.raw_divided);

                    if stdout.trim().len() > 0 {
                        println!("=====stdout=====\n|{}", stdout.trim());
                    }

                    if stderr.trim().len() > 0 {
                        println!("=====stderr=====\n|{}", stderr.trim());
                    }

                    if stdout.trim().len() > 0 || stderr.trim().len() > 0 {
                        println!("================");
                    }

                    return Err(Error::new(ErrorKind::Other, format!("process exited with code: {}.", exitcode)));
                } else if show_output {
                    if stdout.trim().len() > 0 {
                        println!("=====stdout=====\n|{}", stdout.trim());
                    }

                    if stderr.trim().len() > 0 {
                        println!("=====stderr=====\n|{}", stderr.trim());
                    }

                    if stdout.trim().len() > 0 || stderr.trim().len() > 0 {
                        println!("================");
                    }
                }

                return Ok(SubprocessResult {
                    stdout: stdout.to_owned(),
                    stderr: stderr.to_owned(),
                    exitcode,
                })
            }
        }
    }
}
