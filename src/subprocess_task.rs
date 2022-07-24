use std::io::Error;
use std::io::ErrorKind;
use std::process::Command;
use std::io::Result;
use encoding_rs::UTF_8;

pub struct SubprocessTask{
    pub subprocess: Option<Command>,
    pub command: String,
    pub prog: String,
    pub args: Vec<String>,
    pub divided: Vec<String>,
    pub debug: bool,
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

                    let stderr = UTF_8.decode(stderr).0.replace("\r\n", "\n").replace("\r", "\n").trim().replace("\n", "\n|");
                    let stdout = UTF_8.decode(stdout).0.replace("\r\n", "\n").replace("\r", "\n").trim().replace("\n", "\n|");

                    println!("\n命令执行失败，以下是详细信息：");
                    println!("0.raw : {}", self.command);
                    println!("1.file: {:?}", self.prog);
                    println!("2.args: {:?}", self.args);

                    if stdout.trim().len() > 0 {
                        println!("=====stdout=====\n|{}", stdout.trim());
                    }

                    if stderr.trim().len() > 0 {
                        println!("=====stderr=====\n|{}", stderr.trim());
                    }

                    if stdout.trim().len() > 0 || stderr.trim().len() > 0 {
                        println!("================");
                    }

                    if c != 0 {
                        return Err(Error::new(ErrorKind::Other, format!("process exited with code: {}.", c)));
                    }
                }
            }
        }
        Ok(())
    }
}
