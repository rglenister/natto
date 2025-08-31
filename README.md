# **Natto Chess Engine**
**Natto** is a chess engine designed for strong positional and tactical play, supporting advanced features like transposition tables, iterative deepening, and move ordering. It is designed to be compatible with UCI (Universal Chess Interface) and can be used with popular chess GUIs like [Arena](http://playwitharena.de/), [ChessBase](https://en.chessbase.com/), or [CuteChess](https://github.com/cutechess/cutechess).
## **Features**
- Advanced search algorithms (Negamax, Alpha-Beta pruning, Quiescence search)
- Transposition table with customizable memory size
- Supports threefold repetition, 50-move rule, and stalemate detection
- Import/export of FEN formats
- Configurable thread usage for parallelism of the perft performance test
- Supports the Lichess opening database

## **Requirements**
Ensure you have the following installed:
1. **Rust Toolchain**: [Install Rust](https://www.rust-lang.org/tools/install)
``` bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```
1. **Chess GUI**: Any UCI-compatible interface (e.g., Arena, ChessBase, CuteChess).
2. **Environment Configuration**: file or command-line arguments. `.env`

## **Setup**
Clone the repository and install dependencies:
``` bash
git clone https://github.com/your-username/natto.git
cd natto
cargo build --release
```
Please note that the --release flag is required for optimal performance.
## **Configuration**
Natto can be configured using a file or via command-line arguments. Here’s a list of key configuration options: `.env`
### **1. Environment Variables**
Create a file in the project root to customize settings: `.env`
``` env
# Logging Configuration
ENGINE_LOG_FILE=/absolute/path/to/natto.log # Path to the log file
ENGINE_LOG_LEVEL=debug                      # Log levels: trace, debug, info, warn, error

# Search Configuration
ENGINE_HASH_SIZE=512                        # Transposition table size (must be power-of-two, in MB)
RAYON_NUM_THREADS=10                        # Number of threads to use for parallelism in the perft performance test 

# Opening Book
ENGINE_OWN_BOOK=true                        # Enable or disable internal opening book
ENGINE_BOOK_DEPTH=10                        # Depth of moves to consider from the opening book
```
### **2. Command-Line Arguments**
You can override settings using the following command-line flags: `.env`

| Flag          | Description                            | Default Value |
|---------------|----------------------------------------| --- |
| `--hash-size` | Size of the transposition table        | `256` |
| `--log-level` | Log verbosity                          | `info` |
| `--log-file`  | Path to your log file                  | `natto.log` |
| `--own-book`  | Use the engine's internal opening book | `true` |

To run the performance test, use the `--perft` flag.

Example:
``` bash
RUST_LOG=debug cargo run --release -- --hash-size 512 --log-level error
```
To run the performance test:
``` bash
cargo run --release -- --perft
```
## **Running the Engine**
### **1. Directly from Command Line**
To start the chess engine directly:
``` bash
cargo run --release
```
This will initialize the engine in UCI mode, which can be used by any UCI-compatible GUI.
### **2. Integrate with a Chess GUI**
1. **Arena GUI**:
    - Open Arena, navigate to `Engines > Install New Engine`.
    - Select the compiled binary (`target/release/natto`).
    - Configure custom arguments and options in Arena's settings.

2. **CuteChess**:
    - Add Natto as a new UCI engine in the GUI configuration.
    - Ensure it points to the executable (`target/release/natto`).

3. **ChessBase**:
    - Add Natto as a UCI engine in `Engine > Create UCI Engine`.

## **Logging**
Logs provide helpful details about engine behavior, moves searched, and debugging information. By default, logs are stored in the processes current working directory. You can customize the path and verbosity using:
``` env
ENGINE_LOG_FILE=/path/to/natto.log
ENGINE_LOG_LEVEL=debug
```
Or override at runtime:
``` bash
RUST_LOG=trace cargo run --release
```
## **Tests**
The project includes a series of tests to verify engine correctness:
1. Run all tests:
``` bash
   cargo test --release -- --test-threads=1
```
1. Run specific tests:
``` bash
   cargo test test_mate_in_three
```

## **License**
This project is licensed under the **MIT License**. See the [LICENSE](LICENSE) file for details.
By following the instructions above, you should have Natto up and running with a chess GUI of your choice. If you encounter any problems, please submit an issue on GitHub.

