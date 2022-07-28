use clap::Arg;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct AppOptions {
    pub config: String,
    pub debug: bool,
    pub dryrun: bool,
}

impl AppOptions {
    pub fn parse_from_command_line() -> AppOptions {
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
            .arg(Arg::new("dry-run")
                .long("dry-run")
                .help("run but do not execute any commands actually"));
            
        let matches = command.get_matches();

        let arg_config = matches.value_of("config").unwrap_or_else(|| "config.yml").to_owned();
        let arg_debug = matches.is_present("debug");
        let arg_dryrun = matches.is_present("dry-run");

        AppOptions {
            config: arg_config,
            debug: arg_debug,
            dryrun: arg_dryrun,
        }
    }
}