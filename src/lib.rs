// #![feature(iterator_try_collect)]

pub mod file;
pub mod file_comparer;
pub mod blocking_thread_pool;
pub mod subprocess_task;
pub mod application;

pub type AppResult<R> = std::result::Result<R, Box<dyn std::error::Error>>;