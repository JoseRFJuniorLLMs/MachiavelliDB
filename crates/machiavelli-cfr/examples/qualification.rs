use machiavelli_cfr::extensive::{kuhn, leduc};
use machiavelli_core::Deadline;
fn main() {
    let kuhn = kuhn()
        .solve(20_000, &Deadline::from_millis(30_000))
        .unwrap();
    println!(
        "M2 Kuhn: iterations={}, value={:.12}, exploitability={:.9}",
        kuhn.iterations, kuhn.value, kuhn.exploitability
    );
    let leduc = leduc()
        .solve(200_000, &Deadline::from_millis(30_000))
        .unwrap();
    println!(
        "M2 Leduc: iterations={}, value={:.12}, exploitability={:.9}",
        leduc.iterations, leduc.value, leduc.exploitability
    );
    if kuhn.exploitability >= 1e-3 || leduc.exploitability >= 1e-3 {
        eprintln!("M2 not qualified: at least one measured gap exceeds 1e-3");
        std::process::exit(1);
    }
}
