use std::panic;
use std::process;

use incremental_upload::AppResult;
use incremental_upload::application::Application;

fn run() -> AppResult<()> {
    Application::new()?.main()
}

fn main() {
    let ph = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        println!("程序发生错误, 以下为错误详情:");
        ph(panic_info);
        println!("程序发生错误, 以上为错误详情");
        process::exit(1);
    }));

    run().unwrap();
}