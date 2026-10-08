fn main() {
    println!(
        "PAGURO chess engine C Sandro Corsini 2026 ver {}",
        env!("CARGO_PKG_VERSION")
    );
    paguro::init();
    paguro::uci::run();
}
