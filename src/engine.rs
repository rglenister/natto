use std::sync::{mpsc, Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::{io, thread};
use std::io::BufRead;
use std::thread::JoinHandle;
use log::{debug, error, info};
use crate::opening_book::lichess_book::LiChessOpeningBook;
use crate::opening_book::opening_book::OpeningBook;
use crate::{fen, uci, util};
use crate::config::CONFIG;
use crate::search::negamax::iterative_search;
use crate::search::transposition_table::TRANSPOSITION_TABLE;
use crate::game::GameStatus::{Checkmate, Stalemate};
use crate::r#move::convert_chess_move_to_raw;
use crate::uci::{UciGoOptions, UciPosition};

pub fn run() {
    Engine::new().run();
}

enum UciCommand {
    Uci,
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

    fn run(&self) {
        // Spawn input-handling thread
        let (tx, rx) = &self.channel;
        let _input_thread = self.start_input_thread(tx.clone());

        let mut search_handle: Option<JoinHandle<()>> = None;
        let mut uci_position: Option<UciPosition> = None;
        info!("Chess engine started");
        self.main_loop(rx, &mut search_handle, &mut uci_position);
        if let Some(handle) = search_handle {
            handle.join().unwrap();
            info!("Search thread has stopped");
        } else {
            info!("Search thread is not running");
        }
    }

    fn main_loop(&self, rx: &Receiver<String>, search_handle: &mut Option<JoinHandle<()>>, uci_position: &mut Option<UciPosition>) {
        while !self.main_loop_quit_flag.load(Ordering::Relaxed) {
            if let Ok(input) = rx.recv() {
                let command = UciCommand::from_input(&input);
                match command {
                    UciCommand::Stop => self.uci_stop(&self.search_stop_flag, search_handle),
                    UciCommand::Quit => self.uci_quit(&self.search_stop_flag, &self.main_loop_quit_flag),
                    UciCommand::Uci => self.uci_uci(),
                    UciCommand::IsReady => self.uci_is_ready(),
                    UciCommand::UciNewGame => self.uci_new_game(uci_position),
                    UciCommand::Position(_position_str) => self.uci_set_position(&input.to_string(), uci_position,),
                    UciCommand::None => self.uci_none(input.to_string()),
                    UciCommand::Go(_go_options_string) => self.uci_go(&&self.search_stop_flag, search_handle, input.to_string(), uci_position),
                }
            }
        }
        debug!("main loop quit flag is set");
    }
    
    fn uci_set_position(&self, input: &String, uci_position: &mut Option<UciPosition>) {
        let uci_pos = uci::parse_position(&input);
        if let Some(uci_pos) = uci_pos {
            *uci_position = Some(uci_pos.clone());
            info!("uci set position to [{}] with hash code [{}] from input [{}]", fen::write(&uci_pos.end_position), uci_pos.end_position.hash_code(), &input);
        } else {
            error!("failed to parse position from input [{}]", &input)
        }
    }

    fn uci_new_game(&self, uci_position: &mut Option<UciPosition>) {
        info!("UCI new game command received");
        *uci_position = None;
        TRANSPOSITION_TABLE.clear();
    }

    fn uci_go(&self, search_stop_flag: &&Arc<AtomicBool>, search_handle: &mut Option<JoinHandle<()>>, input: String, uci_position: &Option<UciPosition>) {
        self.uci_stop(search_stop_flag, search_handle);
        if let Some(uci_pos) = uci_position {
            if search_handle.is_none() {
                if !self.play_move_from_opening_book(uci_pos) {
                    let uci_go_options: UciGoOptions = uci::parse_uci_go_options(Some(input.clone()));
                    debug!("go options = {:?}", uci_go_options);

                    let search_params = uci::create_search_params(&uci_go_options, &uci_pos);
                    let repeat_position_counts = Some(util::create_repeat_position_counts(uci_pos.all_game_positions()));

                    debug!("search params = {}", search_params);
                    debug!("Starting search...");
                    search_stop_flag.store(false, Ordering::Relaxed); // Reset stop flag

                    let stop_flag = Arc::clone(&search_stop_flag);
                    let uci_pos_clone = uci_pos.clone();
                    *search_handle = Some(thread::spawn(move || {
                        let search_results = iterative_search(&uci_pos_clone.end_position, &search_params, stop_flag, repeat_position_counts);
                        let best_move = search_results.pv.first().map(|cm| convert_chess_move_to_raw(&cm.1));
                        if let Some(best_move) = best_move {
                            uci::send_to_gui(format!("bestmove {}", best_move));
                        } else {
                            match search_results.game_status {
                                Checkmate => { uci::send_to_gui("info score mate 0".to_string()); }
                                Stalemate => { uci::send_to_gui("info score 0".to_string()); }
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
        uci::send_to_gui("readyok".to_string());
    }

    fn uci_uci(&self, ) {
        uci::send_to_gui("id name natto\nid author Richard Glenister\nuciok".to_string());
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

    fn play_move_from_opening_book(&self, uci_pos: &UciPosition) -> bool {
        if let Some(opening_book) = self.opening_book.as_ref() {
            if uci_pos.end_position.full_move_number() <= CONFIG.max_book_depth {
                info!("getting opening book move for position: {}", fen::write(&uci_pos.end_position));
                let opening_move = opening_book.get_opening_move(&uci_pos.end_position);
                if let Ok(opening_move) = opening_move {
                    debug!("got move {} from opening book", opening_move);
                    uci::send_to_gui(format!("bestmove {}", opening_move));
                    return true;
                } else {
                    info!("Failed to retrieve opening book move: {}", opening_move.err().unwrap());
                }
            } else {
                info!("Not playing move from opening book because the full move number {} exceeds the maximum allowed {}", 
                    uci_pos.end_position.full_move_number(), CONFIG.max_book_depth);
            }
        }
        false
    }

    fn create_opening_book() -> Option<LiChessOpeningBook> {
        if CONFIG.use_book {
            info!("Using opening book");
            Some(LiChessOpeningBook::new())
        } else {
            info!("Not using opening book");
            None
        }
    }
}
