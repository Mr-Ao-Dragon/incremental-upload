use std::panic;
use std::process;

use backtrace::Backtrace;
use incremental_upload::AppResult;
use incremental_upload::application::App;

fn run() -> AppResult<()> {
    App::new()?.main()
}

fn main() {
    panic::set_hook(Box::new(move |panic_info| {
        println!("\n程序发生错误, 以下为错误详情:");

        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            println!("panic payload: {s:?}");
        }

        println!("{}\n", panic_info);

        let current_backtrace = Backtrace::new();
        println!("{:#?}", current_backtrace);
        
        // ph(panic_info);
        println!("程序发生错误, 以上为错误详情");
        process::exit(1);
    }));

    run().unwrap();
}