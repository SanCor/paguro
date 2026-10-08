use paguro::board::Position;

fn perft_fen(fen: &str, depth: u32) -> u64 {
    paguro::init();
    let mut pos = Position::from_fen(fen).unwrap();
    pos.perft(depth)
}

#[test]
fn perft_startpos_d1() {
    assert_eq!(
        perft_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            1
        ),
        20
    );
}

#[test]
fn perft_startpos_d2() {
    assert_eq!(
        perft_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            2
        ),
        400
    );
}

#[test]
fn perft_startpos_d3() {
    assert_eq!(
        perft_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            3
        ),
        8902
    );
}

#[test]
fn perft_startpos_d4() {
    assert_eq!(
        perft_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            4
        ),
        197_281
    );
}

#[test]
fn perft_startpos_d5() {
    assert_eq!(
        perft_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
            5
        ),
        4_865_609
    );
}

#[test]
fn perft_kiwipete_d3() {
    assert_eq!(
        perft_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -",
            3
        ),
        97_862
    );
}

#[test]
fn perft_kiwipete_d4() {
    assert_eq!(
        perft_fen(
            "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq -",
            4
        ),
        4_085_603
    );
}

#[test]
fn perft_pos3_d5() {
    assert_eq!(
        perft_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - -", 5),
        674_624
    );
}

#[test]
fn perft_pos4_d4() {
    assert_eq!(
        perft_fen(
            "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
            4
        ),
        422_333
    );
}

#[test]
fn perft_pos5_d3() {
    assert_eq!(
        perft_fen(
            "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
            3
        ),
        62_379
    );
}

#[test]
fn nnue_loads_and_eval_startpos() {
    paguro::init();
    let path = paguro::uci::find_eval_file(paguro::uci::DEFAULT_EVAL);
    let Some(path) = path else {
        panic!("NNUE file nn-3475407dc199.nnue not found");
    };
    let net = paguro::nnue::network::Network::load(path).expect("load nnue");
    let mut pos = Position::new();
    let v = paguro::nnue::evaluate(&mut pos, &net);
    // White's opening edge is a small plus; reject mate-scale garbage.
    assert!(v.abs() < 500, "startpos nnue eval {v}");
}
