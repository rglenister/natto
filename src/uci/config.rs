use clap::{value_parser, Arg, ArgAction, ArgGroup, Command, Parser};
use dotenv::dotenv;
use log::LevelFilter;
use once_cell::sync::Lazy;
use std::sync::{Mutex, RwLock};

pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const BUILD_DATE: &str = env!("BUILD_DATE");
pub const GIT_HASH: &str = env!("GIT_HASH");

pub fn full_version_string() -> String {
    format!("{NAME} {VERSION} (git {GIT_HASH}, {BUILD_DATE}) by {AUTHORS}")
}

pub fn get_log_file() -> String {
    CONFIG.log_file.clone()
}

pub fn get_log_level() -> LevelFilter {
    CONFIG.log_level
}

pub fn get_version() -> bool {
    CONFIG.version
}

pub fn get_perft() -> bool {
    CONFIG.perft
}

pub fn get_uci_commands() -> Option<Vec<String>> {
    CONFIG.uci_commands.clone()
}

pub fn get_own_book() -> bool {
    RUNTIME_CONFIG.own_book.read().unwrap().unwrap_or(CONFIG.own_book)
}

pub fn set_own_book(use_book: bool) {
    *RUNTIME_CONFIG.own_book.write().unwrap() = Some(use_book);
}

pub fn get_book_depth() -> usize {
    RUNTIME_CONFIG.book_depth.read().unwrap().unwrap_or(CONFIG.book_depth)
}

pub fn set_book_depth(max_book_depth: usize) {
    *RUNTIME_CONFIG.book_depth.write().unwrap() = Some(max_book_depth);
}

pub fn get_contempt() -> i32 {
    RUNTIME_CONFIG.contempt.read().unwrap().unwrap_or(0)
}

pub fn set_contempt(contempt: i32) {
    *RUNTIME_CONFIG.contempt.write().unwrap() = Some(contempt);
}

pub fn get_hash_size() -> usize {
    RUNTIME_CONFIG.hash_size.read().unwrap().unwrap_or(CONFIG.hash_size)
}

pub fn set_hash_size(hash_size: usize) {
    *RUNTIME_CONFIG.hash_size.write().unwrap() = Some(hash_size);
}

pub fn get_config_as_string() -> String {
    #[allow(dead_code)]
    #[derive(Debug)]
    struct DynamicConfig {
        log_file: String,
        log_level: LevelFilter,
        own_book: bool,
        book_depth: usize,
        hash_size: usize,
        contempt: i32,
    }
    let configuration = DynamicConfig {
        log_file: get_log_file(),
        log_level: get_log_level(),
        own_book: get_own_book(),
        book_depth: get_book_depth(),
        hash_size: get_hash_size(),
        contempt: get_contempt(),
    };
    format!("{configuration:?}")
}

#[derive(Parser, Debug, Clone)]
pub struct Config {
    pub log_file: String,
    pub log_level: LevelFilter,
    pub own_book: bool,
    pub book_depth: usize,
    pub hash_size: usize,
    pub version: bool,
    pub perft: bool,
    pub uci_commands: Option<Vec<String>>,
}

#[derive(Debug, Default)]
struct RuntimeConfig {
    pub own_book: RwLock<Option<bool>>,
    pub book_depth: RwLock<Option<usize>>,
    pub hash_size: RwLock<Option<usize>>,
    pub contempt: RwLock<Option<i32>>,
}

impl RuntimeConfig {
    pub fn reset(&self) {
        *self.own_book.write().unwrap() = None;
        *self.book_depth.write().unwrap() = None;
        *self.hash_size.write().unwrap() = None;
        *self.contempt.write().unwrap() = None;
    }
}

static CONFIG: Lazy<Config> = Lazy::new(load_config);
static RUNTIME_CONFIG: Lazy<RuntimeConfig> = Lazy::new(RuntimeConfig::default);
static CONFIG_OVERRIDE: Lazy<Mutex<Option<Config>>> = Lazy::new(|| Mutex::new(None));

fn load_config() -> Config {
    dotenv().ok();
    const DEFAULT_LOGFILE_PATH: &str = "./natto.log";
    const DEFAULT_LOG_LEVEL: &str = "info";
    const DEFAULT_OWN_BOOK: &str = "false";
    const DEFAULT_BOOK_DEPTH: &str = "10";
    const DEFAULT_HASH_SIZE: &str = "256";

    CONFIG_OVERRIDE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| {
            let matches = Command::new("Chess Engine")
                .version(VERSION)
                .author(AUTHORS)
                .about("A UCI chess engine")
                .help_template(
                    "{bin} {version}\n\
                    {author}\n\n\
                    {about}\n\n\
                    USAGE:\n    {usage}\n\n\
                    OPTIONS:\n{all-args}",
                )
                .disable_version_flag(true)
                .arg(Arg::new("log-file").short('f').long("log-file").action(ArgAction::Set)
                    .required(false)
                    .default_value(DEFAULT_LOGFILE_PATH)
                    .help("The full path to the log file")
                    .env("ENGINE_LOG_FILE")
                )
                .arg(Arg::new("log-level").short('l').long("log-level").action(ArgAction::Set)
                    .required(false)
                    .default_value(DEFAULT_LOG_LEVEL)
                    .value_parser(["trace", "debug", "info", "warn", "error"])
                    .help("The log level")
                    .env("ENGINE_LOG_LEVEL")
                )
                .arg(Arg::new("own-book").short('b').long("own-book").action(ArgAction::Set)
                    .required(false)
                    .default_value(DEFAULT_OWN_BOOK)
                    .value_parser(["true", "false"])
                    .action(ArgAction::Set)
                    .help("Set to true to use the opening book otherwise false")
                    .env("ENGINE_OWN_BOOK")
                )
                .arg(Arg::new("book-depth").short('d').long("book-depth").action(ArgAction::Set)
                    .required(false)
                    .default_value(DEFAULT_BOOK_DEPTH)
                    .value_parser(value_parser!(u16).range(1..))
                    .help("The maximum full move number of a position that will be considered for the opening book")
                    .env("ENGINE_BOOK_DEPTH")
                )
                .arg(Arg::new("hash-size").short('s').long("hash-size").action(ArgAction::Set)
                    .required(false)
                    .default_value(DEFAULT_HASH_SIZE)
                    .value_parser(is_power_of_two)
                    .help("the size of the hash table in megabytes - must be a power of two")
                    .env("ENGINE_HASH_SIZE")
                )
                .arg(Arg::new("version").short('v').long("version").action(ArgAction::SetTrue)
                    .required(false)
                    .default_value("false")
                    .help("Display the application version information and exit")
                )
                .arg(Arg::new("perft").short('p').long("perft").action(ArgAction::SetTrue)
                    .required(false)
                    .default_value("false")
                    .help("Run the perft (performance test)")
                )
                .arg(Arg::new("uci").short('u').long("uci").action(ArgAction::Set)
                    .required(false)
                    .num_args(1..)
                    .value_delimiter(',')
                    .help("Run the comma separated UCI protocol commands")
                )
                .group(
                    ArgGroup::new("flags")
                        .args(["perft", "uci"])
                        .required(false)
                        .multiple(false)
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
                own_book: matches.get_one::<String>("own-book").is_none_or(|v| v == DEFAULT_OWN_BOOK),
                book_depth: matches.get_one::<u16>("book-depth").copied().unwrap() as usize,
                hash_size: matches.get_one::<String>("hash-size").map(|v| v.parse::<usize>().unwrap()).unwrap(),
                version: *matches.get_one::<bool>("version").unwrap_or(&false),
                perft: *matches.get_one::<bool>("perft").unwrap_or(&false),
                uci_commands: matches.get_many::<String>("uci").map(|values| values.cloned().collect()),
            }
        })
}

pub fn reset_global_configs(config: Config) {
    CONFIG_OVERRIDE.lock().unwrap().replace(config);
    RUNTIME_CONFIG.reset();
}

fn is_power_of_two(s: &str) -> Result<String, String> {
    let size: usize = s.parse().map_err(|_| format!("`{s}` isn't a number"))?;
    if size.is_power_of_two() {
        Ok(size.to_string())
    } else {
        Err(format!("`{s}` isn't a power of two"))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use ctor::ctor;

    #[cfg(test)]
    #[ctor]
    pub fn initialize_test_config() {
        reset_global_configs(create_default_config());
    }

    fn create_default_config() -> Config {
        Config {
            log_file: "./test.log".to_string(),
            log_level: LevelFilter::Info,
            own_book: true,
            book_depth: 10,
            hash_size: 100,
            version: false,
            perft: false,
            uci_commands: None,
        }
    }

    #[test]
    fn test_get_log_file() {
        assert_eq!(get_log_file(), "./test.log");
    }

    #[test]
    fn test_get_log_level() {
        assert_eq!(get_log_level(), LevelFilter::Info);
    }

    #[test]
    fn test_get_perft() {
        assert_eq!(get_perft(), false);
    }

    #[test]
    fn test_get_uci_commands() {
        assert_eq!(get_uci_commands(), None);
    }

    #[test]
    fn test_read_write_use_book() {
        assert_eq!(get_own_book(), true);
        set_own_book(false);
        assert_eq!(get_own_book(), false);
        set_own_book(true);
        assert_eq!(get_own_book(), true);
    }

    #[test]
    fn test_read_write_max_book_depth() {
        assert_eq!(get_book_depth(), 10);
        set_book_depth(20);
        assert_eq!(get_book_depth(), 20);
    }

    #[test]
    fn test_read_write_hash_size() {
        assert_eq!(get_hash_size(), 100);
        set_hash_size(2);
        assert_eq!(get_hash_size(), 2);
        set_hash_size(100);
    }

    #[test]
    fn test_read_write_contempt() {
        assert_eq!(get_contempt(), 0);
        set_contempt(-50);
        assert_eq!(get_contempt(), -50);
        set_contempt(10);
    }
}
