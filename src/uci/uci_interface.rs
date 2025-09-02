use crate::book::lichess_book::LiChessOpeningBook;
use crate::book::opening_book::OpeningBook;
use crate::core::move_gen;
use crate::core::r#move;
use crate::search::negamax::Search;
use crate::search::transposition_table::TranspositionTable;
use crate::search::{move_ordering, negamax};
use crate::uci::logger_controller::LoggerController;
use crate::uci::{config, uci_util};
use crate::utils;
use crate::utils::fen;
use chrono::Local;
use dotenv::dotenv;
use log::{debug, error, info};
use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;
use std::{io, thread};

pub fn run() {
    dotenv().ok();
    let logger_controller = configure_logging();
    info!("Debug assertions are {}", if cfg!(debug_assertions) { "enabled" } else { "disabled" });
    info!("{}", config::get_config_as_string());

    if config::get_perft() {
        println!("Running perft test");
        utils::perf_t::perf_t();
    } else {
        info!("Starting uci");
        Engine::new(logger_controller.ok()).run();
        info!("Engine exited cleanly");
    }
}

enum UciCommand {
    Uci,
    SetOption(String),
    LogConfig,
    IsReady,
    UciNewGame,
    Position(String),
    Go(Option<String>),
    Stop,
    Quit,
    None,
}
impl UciCommand {
    fn from_input(input: &str) -> Self {
        let mut parts = input.split_whitespace();
        match parts.next() {
            Some("uci") => UciCommand::Uci,
            Some("setoption") => UciCommand::SetOption(input.to_string()),
            Some("logconfig") => UciCommand::LogConfig,
            Some("isready") => UciCommand::IsReady,
            Some("ucinewgame") => UciCommand::UciNewGame,
            Some("position") => UciCommand::Position(parts.next().unwrap().to_string()),
            Some("go") => UciCommand::Go(parts.next().map(|s| s.to_string())),
            Some("stop") => UciCommand::Stop,
            Some("quit") => UciCommand::Quit,
            _ => UciCommand::None,
        }
    }
}

struct Engine {
    channel: (Sender<String>, Receiver<String>),
    search_stop_flag: Arc<AtomicBool>,
    main_loop_quit_flag: Arc<AtomicBool>,
    opening_book: LiChessOpeningBook,
    transposition_table: Arc<TranspositionTable>,
    logger_controller: Option<LoggerController>,
}

impl Engine {
    fn new(logger_controller: Option<LoggerController>) -> Self {
        Engine {
            channel: mpsc::channel(),
            search_stop_flag: Arc::new(AtomicBool::new(false)),
            main_loop_quit_flag: Arc::new(AtomicBool::new(false)),
            opening_book: LiChessOpeningBook::new(),
            transposition_table: Arc::new(TranspositionTable::new_using_config()),
            logger_controller,
        }
    }

    fn run(&self) {
        // Spawn input-handling thread
        let (tx, rx) = &self.channel;
        let _input_thread = self.start_input_thread(tx.clone());

        let mut search_handle: Option<JoinHandle<()>> = None;
        let mut uci_position: Option<uci_util::UciPosition> = None;
        let uci_commands = config::get_uci_commands();
        info!("UCI started - awaiting commands");
        self.main_loop(rx, &mut search_handle, &mut uci_position, &uci_commands);
        if let Some(handle) = search_handle {
            handle.join().unwrap();
            info!("Search thread has stopped");
        } else {
            info!("Search thread is not running");
        }
    }

    fn main_loop(
        &self,
        rx: &Receiver<String>,
        search_handle: &mut Option<JoinHandle<()>>,
        uci_position: &mut Option<uci_util::UciPosition>,
        uci_commands: &Option<Vec<String>>,
    ) {
        if let Some(uci_commands) = uci_commands {
            for uci_command in uci_commands {
                println!("Running UCI command: {uci_command}");
                self.run_uci_command(
                    search_handle,
                    uci_position,
                    uci_command,
                    UciCommand::from_input(uci_command),
                );
            }
        } else {
            while !self.main_loop_quit_flag.load(Ordering::Relaxed) {
                if let Ok(input) = rx.recv() {
                    let command = UciCommand::from_input(&input);
                    self.run_uci_command(search_handle, uci_position, &input, command);
                }
            }
        }
        debug!("the main loop quit flag is set");
    }

    fn run_uci_command(
        &self,
        search_handle: &mut Option<JoinHandle<()>>,
        uci_position: &mut Option<uci_util::UciPosition>,
        input: &String,
        command: UciCommand,
    ) {
        match command {
            UciCommand::Uci => Engine::uci_options(),
            UciCommand::SetOption(input) => self.uci_set_option(&input),
            UciCommand::LogConfig => println!("{}", config::get_config_as_string()),
            UciCommand::IsReady => self.uci_is_ready(),
            UciCommand::Stop => self.uci_stop(&self.search_stop_flag, search_handle),
            UciCommand::Quit => self.uci_quit(&self.search_stop_flag, &self.main_loop_quit_flag),
            UciCommand::UciNewGame => self.uci_new_game(uci_position),
            UciCommand::Position(_position_str) => {
                self.uci_set_position(&input.to_string(), uci_position)
            }
            UciCommand::None => self.uci_none(input.to_string()),
            UciCommand::Go(_go_options_string) => {
                self.uci_go(&&self.search_stop_flag, search_handle, input.to_string(), uci_position)
            }
        }
    }

    fn uci_set_position(&self, input: &String, uci_position: &mut Option<uci_util::UciPosition>) {
        let uci_pos = uci_util::parse_position(input);
        if let Some(uci_pos) = uci_pos {
            *uci_position = Some(uci_pos.clone());
            info!(
                "uci set position to [{}] with hash code [{}] from input [{}]",
                fen::write(&uci_pos.end_position),
                uci_pos.end_position.hash_code(),
                &input
            );
        } else {
            error!("failed to parse position from input [{}]", &input)
        }
    }

    fn uci_new_game(&self, uci_position: &mut Option<uci_util::UciPosition>) {
        info!("UCI new game command received");
        *uci_position = None;
        self.transposition_table.clear();
    }

    fn uci_go(
        &self,
        search_stop_flag: &&Arc<AtomicBool>,
        search_handle: &mut Option<JoinHandle<()>>,
        input: String,
        uci_position: &Option<uci_util::UciPosition>,
    ) {
        self.uci_stop(search_stop_flag, search_handle);
        if let Some(uci_pos) = uci_position {
            if search_handle.is_none() {
                if !self.play_move_from_opening_book(uci_pos) {
                    let uci_go_options: uci_util::UciGoOptions =
                        uci_util::parse_uci_go_options(Some(input.clone()));
                    debug!("go options = {uci_go_options:?}");

                    let search_params = uci_util::create_search_params(&uci_go_options, uci_pos);

                    debug!("search params = {search_params:?}");
                    debug!("Starting search...");
                    search_stop_flag.store(false, Ordering::Relaxed); // Reset stop flag

                    let stop_flag = Arc::clone(search_stop_flag);
                    let uci_pos_clone = uci_pos.clone();
                    let mut position = uci_pos_clone.end_position;
                    let transposition_table = Arc::clone(&self.transposition_table);
                    *search_handle = Some(thread::spawn(move || {
                        let mut search = Search::new(
                            &mut position,
                            &transposition_table,
                            search_params,
                            stop_flag,
                            uci_pos_clone.repetition_keys.clone(),
                            move_ordering::MoveOrderer::new(),
                            0,
                        );
                        let search_results = search.go();
                        debug!("score: {} depth {}", search_results.score, search_results.depth);

                        let best_move = search_results
                            .pv
                            .first()
                            .copied()
                            .or(uci_pos_clone.previous_move_from_position())
                            .or(move_gen::get_first_legal_move(&position));

                        if search_results.score == negamax::MAXIMUM_SCORE.abs() {
                            uci_util::send_to_gui("info score mate 0");
                        } else if search_results.score == negamax::DRAW_SCORE {
                            uci_util::send_to_gui("info score cp 0");
                        };

                        let best_move_str = best_move
                            .map(r#move::convert_move_to_raw)
                            .map(|rm| rm.to_string())
                            .unwrap_or_else(|| "none".to_string());
                        uci_util::send_to_gui(format!("bestmove {best_move_str}").as_str());
                    }))
                }
            } else {
                error!("Cannot initiate search because the position is already being searched");
            }
        } else {
            error!("Cannot initiate search because the position has not been set");
        }
    }

    fn uci_none(&self, input: String) {
        error!("invalid UCI command: {input:?}");
    }
    fn uci_is_ready(&self) {
        uci_util::send_to_gui("readyok");
    }

    fn uci_options() {
        uci_util::send_to_gui(format!("id name {}", config::FULL_VERSION.as_str()).as_str());
        uci_util::send_to_gui(format!("id author {}", config::AUTHORS).as_str());
        uci_util::send_to_gui("option name Debug Log File type string default");
        uci_util::send_to_gui("option name ownbook type check default true");
        uci_util::send_to_gui("option name bookdepth type spin default 10 min 1 max 50");
        uci_util::send_to_gui("option name hash type combo default 256 var 1 var 2 var 4 var 8 var 16 var 32 var 64 var 128 var 256 var 512 var 1024 var 2048");
        uci_util::send_to_gui("uciok");
    }

    fn parse_uci_option(input: &str) -> Option<(String, String)> {
        let re = regex::Regex::new(r"^setoption name (.+?) value (.+)$").unwrap();
        if let Some(captures) = re.captures(input) {
            let name = captures.get(1).unwrap().as_str();
            let value = captures.get(2).unwrap().as_str();
            Some((name.trim().to_string(), value.trim().to_string()))
        } else {
            error!("Failed to parse UCI option: {input}");
            None
        }
    }
    fn uci_set_option(&self, input: &str) {
        if let Some((name, value)) = Self::parse_uci_option(input) {
            match name.to_lowercase().as_str() {
                "debug log file" => {
                    if let Some(logger_controller) = &self.logger_controller {
                        logger_controller.set_debug_file(value.as_str());
                    }
                }
                "hash" => {
                    if let Ok(v) = value.parse::<usize>() {
                        config::set_hash_size(v);
                    }
                }
                "contempt" => {
                    if let Ok(v) = value.parse::<i32>() {
                        config::set_contempt(v);
                    }
                }
                "ownbook" => {
                    if let Ok(v) = value.parse::<bool>() {
                        config::set_own_book(v);
                    }
                }
                "bookdepth" => {
                    if let Ok(v) = value.parse::<usize>() {
                        config::set_book_depth(v);
                    }
                }
                _ => {
                    uci_util::send_to_gui(&format!("info string Unknown option: {name}"));
                }
            }
        }
        info!("{}", config::get_config_as_string());
    }

    fn uci_quit(&self, search_stop_flag: &Arc<AtomicBool>, main_loop_quit_flag: &Arc<AtomicBool>) {
        info!("UCI Quit command received. Shutting down...");
        search_stop_flag.store(true, Ordering::Relaxed);
        main_loop_quit_flag.store(true, Ordering::Relaxed);
    }

    fn uci_stop(
        &self,
        search_stop_flag: &Arc<AtomicBool>,
        search_handle: &mut Option<JoinHandle<()>>,
    ) {
        if let Some(handle) = search_handle.take() {
            info!("Stopping search...");
            search_stop_flag.store(true, Ordering::Relaxed);
            handle.join().unwrap(); // Ensure search thread stops
            info!("Search stopped");
        } else {
            info!("Not currently searching");
        }
    }

    fn start_input_thread(&self, tx: Sender<String>) -> JoinHandle<()> {
        let tx = tx.clone();
        thread::spawn(move || {
            let stdin = io::stdin();
            for line in stdin.lock().lines() {
                let line = line.unwrap();
                let line = line.trim();
                info!("Received from stdin: {line}");
                if tx.send(line.to_string()).is_err() {
                    break; // Stop if main thread is gone
                }
            }
        })
    }

    fn play_move_from_opening_book(&self, uci_pos: &uci_util::UciPosition) -> bool {
        if config::get_own_book() {
            if uci_pos.end_position.full_move_number() <= config::get_book_depth() {
                info!(
                    "getting opening book move for position: {}",
                    fen::write(&uci_pos.end_position)
                );
                let opening_move = self.opening_book.get_opening_move(&uci_pos.end_position);
                if let Ok(opening_move) = opening_move {
                    debug!("got move {opening_move} from opening book");
                    uci_util::send_to_gui(format!("bestmove {opening_move}").as_str());
                    return true;
                } else {
                    info!("Failed to retrieve opening book move: {}", opening_move.err().unwrap());
                }
            } else {
                info!("Not playing move from opening book because the full move number {} exceeds the maximum allowed {}",
                    uci_pos.end_position.full_move_number(), config::get_book_depth());
            }
        }
        false
    }
}

fn configure_logging() -> Result<LoggerController, fern::InitError> {
    let logger_controller = LoggerController::new();
    setup_logging(&logger_controller)
        .map_err(|err| {
            eprintln!("Failed to initialize logging: {err:?}");
            err
        })
        .ok();
    Ok(logger_controller)
}

fn setup_logging(logger_controller: &LoggerController) -> Result<(), fern::InitError> {
    let base = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(config::get_log_level())
        .chain(io::stderr())
        .chain(fern::log_file(config::get_log_file().clone())?);

    logger_controller.chain_debug_file(base).apply()?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uci_option() {
        assert_eq!(
            Engine::parse_uci_option("setoption name contempt value -50"),
            Some(("contempt".to_string(), "-50".to_string()))
        );
    }
}
