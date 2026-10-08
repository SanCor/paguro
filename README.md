# Paguro Chess Engine

<img src="icons/Paguro_icon807x768.png" alt="Paguro logo" width="180">

Paguro is a free and open-source UCI chess engine written in Rust by
Sandro Corsini.

Current version: **0.5.0**

Paguro is an engine only: to play games, use it with a UCI-compatible chess
GUI such as Arena, BanksiaGUI, Cute Chess, or another UCI frontend.

## Features

- Iterative-deepening alpha-beta/PVS search
- Quiescence search and clustered transposition table
- Null-move pruning, futility pruning, LMR, LMP, and history heuristics
- Incremental HalfKAv2 NNUE evaluation compatible with Stockfish 14 networks
- AVX2 NNUE kernels with a scalar fallback
- Polyglot opening-book support
- UCI time controls, fixed depth, and fixed movetime
- Portable and CPU-native release builds

Paguro currently searches on one thread. It does not currently support
Syzygy tablebases.

## NNUE network

Paguro uses the evaluation network:

`nn-3475407dc199.nnue`

This was the default network used by Stockfish 14. The network is a
third-party work from the official Stockfish network collection; it is not
authored by Sandro Corsini or by the Paguro project.

The official Stockfish networks repository states that its networks were
uploaded by their authors under the **CC0 1.0 Universal** public-domain
dedication:

- Network repository: <https://github.com/official-stockfish/networks>
- Stockfish commit selecting this network:
  <https://github.com/official-stockfish/Stockfish/commit/49283d3a6676e114b531d6a8b9e5f69000655912>
- Official download:
  <https://tests.stockfishchess.org/api/nn/nn-3475407dc199.nnue>
- SHA-256:
  `3475407dc19973ea44467678634cce023d620e419770c111cc8937fe6689ec87`

See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for attribution and
licensing details.

The source repository does not require the 47 MB network to be committed.
Download it separately and place it:

1. beside `paguro.exe`;
2. in the current working directory; or
3. in a `nets` subdirectory.

Without the network, Paguro starts with its simpler handcrafted fallback
evaluation and reports the fallback on the console.

## Building

Install a current stable Rust toolchain, then run:

```text
cargo build --release
```

The portable executable is created at:

```text
target/release/paguro.exe
```

On Windows, a build optimized for the current CPU can be produced with:

```text
powershell -ExecutionPolicy Bypass -File tools/build_native.ps1
```

The native executable is created at:

```text
target/native/release/paguro.exe
```

A native executable may not run on older or different processors. Use the
portable build for general distribution.

## Running

From a terminal:

```text
paguro.exe
```

Basic UCI handshake:

```text
uci
isready
position startpos
go movetime 5000
quit
```

At startup Paguro prints its name, copyright notice, and version, followed by
diagnostic messages about preferences, NNUE, and the opening book.

## UCI options

- `Hash`: transposition-table size in MB, from 1 to 4096
- `Threads`: currently fixed at 1
- `EvalFile`: NNUE filename
- `OwnBook`: enable or disable the Polyglot opening book
- `BookFile`: Polyglot `.bin` file

## Preferences and opening books

Copy `paguro.ini` beside the executable or keep it in the working directory.
Example:

```ini
OwnBook = true
BookFile = book.bin
```

`BookFile` can point to another Polyglot book, including one created by the
user. UCI options override the file preferences for the current session.
No third-party opening book is distributed by this source repository.

## Tests

Run all unit, NNUE, and perft tests with:

```text
cargo test --release
```

The fixed-depth performance benchmark is available on Windows:

```text
powershell -ExecutionPolicy Bypass -File tools/bench_knps.ps1 `
  -Engine target/release/paguro.exe
```

## Release package

A binary release should contain:

- the portable Paguro executable;
- `nn-3475407dc199.nnue`;
- `paguro.ini`;
- `README.md`;
- `LICENSE`;
- `THIRD_PARTY_NOTICES.md`;
- optionally, a Polyglot opening book whose redistribution terms are known.

When distributing a GPL-covered executable, make the corresponding Paguro
source code available as required by the GPL.

## Development transparency

Portions of this project were developed with the assistance of AI-based
programming tools. All code was reviewed, tested, and is maintained by the
project author.

## License

Paguro is copyright © 2026 Sandro Corsini and is licensed under the
GNU General Public License, version 3 or (at your option) any later version.

The NNUE network is separate third-party material distributed under CC0 1.0
Universal. Its inclusion does not change Paguro's GPL license.
