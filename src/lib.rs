pub mod file;
pub mod file_comparer;
pub mod blocking_thread_pool;
pub mod subprocess_task;
pub mod application;
pub mod app_config;
pub mod app_options;
pub mod utils;
pub mod variable_replace;
pub mod simple_file;
pub mod file_state;
pub mod differences;
pub mod hash_cache;
pub mod rule_filter;

pub type AppResult<R> = std::result::Result<R, Box<dyn std::error::Error>>;