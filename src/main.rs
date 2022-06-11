use std::env;

use clap::Arg;
use clap::Command;
use clap::arg;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_metadata() {
    println!("v{VERSION}");
}

fn main() {
    print_metadata();

    let matches = Command::new(APP_NAME)
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
            .help("run but do not execute any commands actually"))
        .get_matches();
    
    let arg_config = matches.value_of("config");
    let arg_source = matches.value_of("source-dir").expect("source-dir must be supplied");
    let arg_debug = matches.is_present("debug");
    let arg_dryrun = matches.is_present("dry-run");

    // 检查参数
    let arg_source = if arg_source.ends_with("/") {
        &arg_source[0..arg_source.len() - 2]
    } else {
        &arg_source[..]
    };

    let workdir = env::current_dir().expect("field to get Current Work Directory");
    let source_dir = 



    println!("source_dir is {:?}", matches.value_of("source_dir"));
    println!("config is {:?}", matches.value_of("config"));
}
