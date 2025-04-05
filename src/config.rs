use dotenv::dotenv;
use once_cell::sync::Lazy;

#[derive(Debug)]
pub struct Config {
    pub log_file: String,
    pub log_level: log::LevelFilter,
    pub use_opening_book: bool,
    pub opening_book_maximum_depth: usize,
    pub transposition_table_size: usize,
}

pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    dotenv().ok();
    Config {
        log_file: std::env::var("LOGFILE")
            .unwrap_or_else(|_| "natto.log".to_string()),
        log_level: match std::env::var("LOGLEVEL").unwrap_or_else(|_| "error".to_string()).to_lowercase().as_str() {
            "trace" => log::LevelFilter::Trace,
            "debug" => log::LevelFilter::Debug,
            "info" => log::LevelFilter::Info,
            "warn" => log::LevelFilter::Warn,
            "error" => log::LevelFilter::Error,
            _ => log::LevelFilter::Error,
        },
        use_opening_book: std::env::var("USE_OPENING_BOOK")
            .unwrap_or_else(|_| "true".to_string())
            .eq_ignore_ascii_case("true"),
        opening_book_maximum_depth: std::env::var("OPENING_BOOK_MAXIMUM_DEPTH")
            .unwrap_or_else(|_| "10".to_string())
            .parse::<usize>()
            .unwrap_or(10),
        transposition_table_size: std::env::var("TRANSPOSITION_TABLE_SIZE")
            .unwrap_or_else(|_| (1 << 25).to_string())
            .parse::<usize>()
            .unwrap_or(1 << 25),
    }
}); 

