use clap::{value_parser, Arg, ArgAction, Command, Parser};
use dotenv::dotenv;
use log::LevelFilter;
use once_cell::sync::Lazy;

#[derive(Parser, Debug, Clone)]
#[clap(author, version, about, long_about = None)]
pub struct Config {
    pub log_file: String,
    pub log_level: LevelFilter,
    pub use_book: bool,
    pub max_book_depth: usize,
    pub hash_size: usize,
}
pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    dotenv().ok();
    let matches = Command::new("Chess Engine")
        .version("1.0")
        .about("A UCI chess engine")
        .arg(Arg::new("log-file").short('f').long("log-file").action(ArgAction::Set)
            .required(false)
            .default_value("./natto.log")
            .help("The full path to the log file")
            .env("ENGINE_LOG_FILE")
        )
        .arg(Arg::new("log-level").short('l').long("log-level").action(ArgAction::Set)
            .required(false)
            .default_value("info")
            .value_parser(["trace", "debug", "info", "warn", "error"])
            .help("The log level")
            .env("ENGINE_LOG_LEVEL")
        )
        .arg(Arg::new("use-book").short('b').long("use-book").action(ArgAction::Set)
            .required(false)
            .default_value("true")
            .value_parser(["true", "false"])
            .action(ArgAction::Set)
            .help("Set to true to use the opening book otherwise false")
            .env("ENGINE_USE_BOOK")
        )
        .arg(Arg::new("max-book-depth").short('d').long("max-book-depth").action(ArgAction::Set)
            .required(false)
            .default_value("10")
            .value_parser(value_parser!(u16).range(1..))
            .help("The maximum full move number of a position that will be considered for the opening book")
            .env("ENGINE_MAX_BOOK_DEPTH")
        )
        .arg(Arg::new("hash-size").short('s').long("hash-size").action(ArgAction::Set)
            .required(false)
            .default_value("1048576")
            .value_parser(is_power_of_two)
            .help("The maximum full move number of a position that will be considered for the opening book")
            .env("ENGINE_HASH_SIZE")
        ).get_matches();

    Config {
        log_file: matches.get_one::<String>("log-file").unwrap().to_string(),
        log_level: match matches.get_one::<String>("log-level").map(String::as_str).unwrap_or("error") {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "info" => LevelFilter::Info,
            "warn" => LevelFilter::Warn,
            "error" => LevelFilter::Error,
            _ => LevelFilter::Error,
        },
        use_book: matches.get_one::<String>("use-book").map_or(true, |v| v == "true"),
        max_book_depth: matches.get_one::<u16>("max-book-depth").copied().unwrap() as usize,
        hash_size: matches.get_one::<String>("hash-size").map(|v| v.parse::<usize>().unwrap()).unwrap()
    }
});

fn is_power_of_two(s: &str) -> Result<String, String> {
    let size: usize = s
        .parse()
        .map_err(|_| format!("`{s}` isn't a number"))?;
    if size.is_power_of_two() {
        Ok(size.to_string())
    } else {
        Err(format!("`{s}` isn't a power of two"))
    }
}

