use machiavelli_core::{ContinentiaNode, NodeId};
use machiavelli_hamiltonian::{AdversarialPotential, StrategicHamiltonian, StrategicMass};
use machiavelli_storage::{Mutation, Store};
use machiavelli_symplectic::{Integrator, Scheme, systems::Harmonic};
use std::error::Error;

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: machiavelli DATA_DIR init|stats|list|get ID|put JSON_FILE|delete ID|simulate ID STEPS DT".into());
    }
    if args[1] == "solve" {
        if args.len() != 5 {
            return Err(
                "usage: machiavelli DATA_DIR solve GAME_JSON ITERATIONS DEADLINE_MS".into(),
            );
        }
        let matrix: machiavelli_core::PayoffMatrix =
            serde_json::from_reader(std::fs::File::open(&args[2])?)?;
        let game = machiavelli_nash::ZeroSumGame::new(&matrix)?;
        let ms: u64 = args[4].parse()?;
        if ms == 0 || ms > 10000 {
            return Err("deadline must be 1..10000 ms".into());
        }
        let solution = machiavelli_cfr::solve(
            &game,
            args[3].parse()?,
            &machiavelli_core::Deadline::from_millis(ms),
        )?;
        println!(
            "{}",
            serde_json::json!({
                "model":"normal_form_zero_sum","row_strategy":solution.row_strategy,
                "col_strategy":solution.col_strategy,"exploitability":solution.certificate.exploitability,
                "value":solution.certificate.value,"lower_bound":solution.certificate.lower_bound,
                "upper_bound":solution.certificate.upper_bound,"iterations":solution.iterations,
                "deadline_reached":solution.deadline_reached
            })
        );
        return Ok(());
    }
    if args[1] == "restore" && args.len() == 3 {
        let restored = machiavelli_storage::backup::restore(&args[2], &args[0])?;
        println!(
            "{}",
            serde_json::json!({"restored_nodes":restored.snapshot().len()})
        );
        return Ok(());
    }
    let mut store = Store::open(&args[0])?;
    match args[1].as_str() {
        "init" | "stats" if args.len() == 2 => println!(
            "{}",
            serde_json::json!({
                "nodes":store.snapshot().len(), "sequence":store.snapshot().sequence(),
                "recovered_bytes":store.recovered_bytes()
            })
        ),
        "backup" if args.len() == 3 => {
            machiavelli_storage::backup::backup(&store.snapshot(), &args[2])?;
            println!(
                "{}",
                serde_json::json!({"backed_up_nodes":store.snapshot().len()})
            );
        }
        "list" if args.len() == 2 => {
            println!(
                "{}",
                serde_json::to_string(&store.snapshot().nodes().collect::<Vec<_>>())?
            );
        }
        "get" if args.len() == 3 => {
            let snapshot = store.snapshot();
            let node = snapshot
                .get(args[2].parse::<NodeId>()?)
                .ok_or("node not found")?;
            println!("{}", serde_json::to_string(node)?);
        }
        "put" if args.len() == 3 => {
            let file = std::fs::File::open(&args[2])?;
            let node: ContinentiaNode = serde_json::from_reader(file)?;
            let sequence = store.commit(&[Mutation::Put(node)])?;
            println!("{}", serde_json::json!({"sequence":sequence}));
        }
        "delete" if args.len() == 3 => {
            let sequence = store.commit(&[Mutation::Delete(args[2].parse()?)])?;
            println!("{}", serde_json::json!({"sequence":sequence}));
        }
        "simulate" if args.len() == 5 => {
            let steps: usize = args[3].parse()?;
            let dt: f64 = args[4].parse()?;
            if steps > 1_000_000 || !dt.is_finite() || dt.abs() > 1.0 {
                return Err("simulation requires steps <= 1000000 and finite |dt| <= 1".into());
            }
            let snapshot = store.snapshot();
            let node = snapshot.get(args[2].parse()?).ok_or("node not found")?;
            let n = node.phase.dim();
            let mass = StrategicMass::from_node(&node.meta, n, 0.0, Default::default())?;
            let system = StrategicHamiltonian::new(
                mass,
                Harmonic::isotropic(n, 1.0),
                AdversarialPotential::new(vec![0.0; n], 0.0, 1.0)?,
            )?;
            let mut phase = node.phase.clone();
            let initial = system.evaluate(&phase)?;
            let (q, p) = phase.qp_mut();
            Integrator::new(system.separable(), Scheme::Yoshida4)
                .compensated()
                .run(q, p, dt, steps)?;
            let final_energy = system.evaluate(&phase)?;
            println!(
                "{}",
                serde_json::json!({"model":"reference_harmonic",
                "persisted":false,"source_sequence":snapshot.sequence(),
                "initial_energy":initial,"final_energy":final_energy,"phase":phase})
            );
        }
        _ => return Err("invalid command or argument count".into()),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
