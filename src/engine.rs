use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::{io, thread};
use std::io::BufRead;
use std::thread::JoinHandle;
use log::{debug, error, info};
use regex::bytes::Regex;
use crate::opening_book::lichess_book::LiChessOpeningBook;
use crate::opening_book::opening_book::OpeningBook;
use crate::{fen, uci, chess_util::util, config};
use crate::config::{set_contempt, set_hash_size, set_max_book_depth, set_use_book};
use crate::search::negamax::iterative_deepening;
use crate::search::transposition_table::{TranspositionTable, TRANSPOSITION_TABLE};
use crate::game::GameStatus::{Checkmate, Stalemate};
use crate::r#move::convert_chess_move_to_raw;
use crate::uci::send_to_gui;

pub fn run(uci_commands: &Option<Vec<String>>) {
    Engine::new().run(uci_commands);
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
    None
    
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
            _ => UciCommand::None
        }
    }
}

struct Engine {
    channel: (Sender<String>, Receiver<String>),
    search_stop_flag: Arc<AtomicBool>,
    main_loop_quit_flag: Arc<AtomicBool>,
    opening_book: Option<LiChessOpeningBook>,
}

impl Engine {
    fn new() -> Self {
        Engine {
            channel: mpsc::channel(),
            search_stop_flag: Arc::new(AtomicBool::new(false)),
            main_loop_quit_flag: Arc::new(AtomicBool::new(false)),
            opening_book: Self::create_opening_book(),
        }
    }

    fn run(&self, uci_commands: &Option<Vec<String>>) {
        // Spawn input-handling thread
        let (tx, rx) = &self.channel;
        let _input_thread = self.start_input_thread(tx.clone());

        let mut search_handle: Option<JoinHandle<()>> = None;
        let mut uci_position: Option<uci::UciPosition> = None;
        info!("Chess engine started");
        self.main_loop(rx, &mut search_handle, &mut uci_position, uci_commands);
        if let Some(handle) = search_handle {
            handle.join().unwrap();
            info!("Search thread has stopped");
        } else {
            info!("Search thread is not running");
        }
    }

    fn main_loop(&self, rx: &Receiver<String>, search_handle: &mut Option<JoinHandle<()>>, uci_position: &mut Option<uci::UciPosition>, uci_commands: &Option<Vec<String>>) {
        if let Some(uci_commands) = uci_commands {
            for uci_command in uci_commands {
                println!("Running UCI command: {}", uci_command);
                self.run_uci_command(search_handle, uci_position, &uci_command, UciCommand::from_input(&uci_command));
            }
        } else {
            while !self.main_loop_quit_flag.load(Ordering::Relaxed) {
                if let Ok(input) = rx.recv() {
                    let command = UciCommand::from_input(&input);
                    self.run_uci_command(search_handle, uci_position, &input, command);
                }
            }
        }
        debug!("main loop quit flag is set");
    }

    fn run_uci_command(&self, search_handle: &mut Option<JoinHandle<()>>, uci_position: &mut Option<uci::UciPosition>, input: &String, command: UciCommand) {
        match command {
            UciCommand::Uci => Engine::uci_options(),
            UciCommand::SetOption(input) => self.uci_set_option(&input),
            UciCommand::LogConfig => println!("{}", config::get_config_as_string()),
            UciCommand::IsReady => self.uci_is_ready(),
            UciCommand::Stop => self.uci_stop(&self.search_stop_flag, search_handle),
            UciCommand::Quit => self.uci_quit(&self.search_stop_flag, &self.main_loop_quit_flag),
            UciCommand::UciNewGame => self.uci_new_game(uci_position),
            UciCommand::Position(_position_str) => self.uci_set_position(&input.to_string(), uci_position, ),
            UciCommand::None => self.uci_none(input.to_string()),
            UciCommand::Go(_go_options_string) => self.uci_go(&&self.search_stop_flag, search_handle, input.to_string(), uci_position),
        }
    }

    fn uci_set_position(&self, input: &String, uci_position: &mut Option<uci::UciPosition>) {
        let uci_pos = uci::parse_position(&input);
        if let Some(uci_pos) = uci_pos {
            *uci_position = Some(uci_pos.clone());
            info!("uci set position to [{}] with hash code [{}] from input [{}]", fen::write(&uci_pos.end_position), uci_pos.end_position.hash_code(), &input);
        } else {
            error!("failed to parse position from input [{}]", &input)
        }
    }

    fn uci_new_game(&self, uci_position: &mut Option<uci::UciPosition>) {
        info!("UCI new game command received");
        *uci_position = None;
        TRANSPOSITION_TABLE.clear();
    }

    fn uci_go(&self, search_stop_flag: &&Arc<AtomicBool>, search_handle: &mut Option<JoinHandle<()>>, input: String, uci_position: &Option<uci::UciPosition>) {
        self.uci_stop(search_stop_flag, search_handle);
        if let Some(uci_pos) = uci_position {
            if search_handle.is_none() {
                if !self.play_move_from_opening_book(uci_pos) {
                    let uci_go_options: uci::UciGoOptions = uci::parse_uci_go_options(Some(input.clone()));
                    debug!("go options = {:?}", uci_go_options);

                    let search_params = uci::create_search_params(&uci_go_options, &uci_pos);
                    let repeat_position_counts = Some(util::create_repeat_position_counts(uci_pos.all_game_positions()));

                    debug!("search params = {}", search_params);
                    debug!("Starting search...");
                    search_stop_flag.store(false, Ordering::Relaxed); // Reset stop flag

                    let stop_flag = Arc::clone(&search_stop_flag);
                    let uci_pos_clone = uci_pos.clone();
                    *search_handle = Some(thread::spawn(move || {
                        let search_results = iterative_deepening(&uci_pos_clone.end_position, &search_params, stop_flag, repeat_position_counts);
                        debug!("score: {} depth {}", search_results.score, search_results.depth);
                        let best_move = search_results.pv.first().map(|cm| convert_chess_move_to_raw(&cm.1));
                        if let Some(best_move) = best_move {
                            uci::send_to_gui(format!("bestmove {}", best_move).as_str());
                        } else {
                            match search_results.game_status {
                                Checkmate => { uci::send_to_gui("info score mate 0"); }
                                Stalemate => { uci::send_to_gui("info score 0"); }
                                _ => ()
                            }
                        }
                    }));
                }
            } else {
                error!("Cannot initiate search because the position is already being searched");
            }
        } else {
            error!("Cannot initiate search because the position has not been set");
        }
    }
    
    fn uci_none(&self, input: String) {
        error!("invalid UCI command: {:?}", input);
    }
    fn uci_is_ready(&self, ) {
        uci::send_to_gui("readyok");
    }

    fn uci_options() {
        uci::send_to_gui("id name natto");
        uci::send_to_gui("id author Richard Glenister");
        uci::send_to_gui("option name UseBook type check default true");
        uci::send_to_gui("option name MaxBookDepth type spin default 10 min 1 max 50");
        uci::send_to_gui("option name Hash type spin default 128 min 1 max 2048");
        uci::send_to_gui("option name Contempt type spin default 0 min -500 max 500");
        uci::send_to_gui("uciok");
    }

    fn parse_uci_option(input: &str) -> Option<(String, String)> {
        let re = regex::Regex::new(r"setoption name ([^\s]+) value ([^\s]+)").unwrap();
        if let Some(captures) = re.captures(input) {
            let name = captures.get(1).unwrap().as_str();
            let value = captures.get(2).unwrap().as_str();
            Some((name.to_string(), value.to_string()))
        } else {
            error!("Failed to parse UCI option: {}", input);
            None
        }
    }
    fn uci_set_option(&self, input: &str) {
        if let Some((name, value)) = Self::parse_uci_option(input) {
            match name.to_lowercase().as_str() {
                "hash" => {
                    if let Ok(v) = value.parse::<usize>() {
                        set_hash_size(v);
                        send_to_gui(&format!("info string Hash set to {}", v));
                    }
                }
                "contempt" => {
                    if let Ok(v) = value.parse::<isize>() {
                        set_contempt(v);
                        send_to_gui(&format!("info string Contempt set to {}", v));
                    }
                }
                "usebook" => {
                    if let Ok(v) = value.parse::<bool>() {
                        set_use_book(v);
                        send_to_gui(&format!("info string UseBook set to {}", v));
                    }
                }
                "maxbookdepth" => {
                    if let Ok(v) = value.parse::<usize>() {
                        set_max_book_depth(v);
                        send_to_gui(&format!("info string MaxBookDepth set to {}", v));
                    }
                }
                _ => {
                    uci::send_to_gui(&format!("info string Unknown option: {}", name));
                }
            }
            
        }
    }

    fn uci_quit(&self, search_stop_flag: &Arc<AtomicBool>, main_loop_quit_flag: &Arc<AtomicBool>) {
        info!("UCI Quit command received. Shutting down...");
        search_stop_flag.store(true, Ordering::Relaxed);
        main_loop_quit_flag.store(true, Ordering::Relaxed);
    }

    fn uci_stop(&self, search_stop_flag: &Arc<AtomicBool>, search_handle: &mut Option<JoinHandle<()>>) {
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
                info!("Received from stdin: {}", line);
                if tx.send(line).is_err() {
                    break; // Stop if main thread is gone
                }
            }
        })
    }

    fn play_move_from_opening_book(&self, uci_pos: &uci::UciPosition) -> bool {
        if let Some(opening_book) = self.opening_book.as_ref() {
            if uci_pos.end_position.full_move_number() <= config::get_max_book_depth() {
                info!("getting opening book move for position: {}", fen::write(&uci_pos.end_position));
                let opening_move = opening_book.get_opening_move(&uci_pos.end_position);
                if let Ok(opening_move) = opening_move {
                    debug!("got move {} from opening book", opening_move);
                    uci::send_to_gui(format!("bestmove {}", opening_move).as_str());
                    return true;
                } else {
                    info!("Failed to retrieve opening book move: {}", opening_move.err().unwrap());
                }
            } else {
                info!("Not playing move from opening book because the full move number {} exceeds the maximum allowed {}", 
                    uci_pos.end_position.full_move_number(), config::get_max_book_depth());
            }
        }
        false
    }

    fn create_opening_book() -> Option<LiChessOpeningBook> {
        if config::get_use_book() {
            info!("Using opening book");
            Some(LiChessOpeningBook::new())
        } else {
            info!("Not using opening book");
            None
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uci_option() {
        assert_eq!(Engine::parse_uci_option("setoption name contempt value -50"), Some(("contempt".to_string(), "-50".to_string())));
    }
}