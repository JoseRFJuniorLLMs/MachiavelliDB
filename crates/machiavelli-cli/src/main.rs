use machiavelli_core::{ContinentiaNode, NodeId};
use machiavelli_hamiltonian::{AdversarialPotential, StrategicHamiltonian, StrategicMass};
use machiavelli_storage::{Mutation, Store};
use machiavelli_symplectic::{Integrator, Scheme, systems::Harmonic};
use std::error::Error;

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: machiavelli DATA_DIR init|stats|list|records|plan ID ITERATIONS HORIZON SEED DEADLINE_MS|train-kuhn ITERATIONS DEADLINE_MS|get ID|put JSON_FILE|delete ID|commit BATCH_JSON [EXPECTED_SEQUENCE]|backup DEST|restore SOURCE|simulate ID STEPS DT|solve GAME_JSON ITERATIONS DEADLINE_MS".into());
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
        "train-deep" if args.len() == 6 => {
            use machiavelli_cfr::extensive::{Game, Tree, deep};
            use machiavelli_storage::{Family, RecordKey, StrategicRecord};
            let tree: Tree = serde_json::from_reader(std::fs::File::open(&args[2])?)?;
            let game = Game::new(tree.clone())?;
            let id = machiavelli_core::InfosetHash::of(&serde_json::to_vec(&tree)?);
            let snapshot = store.snapshot();
            let checkpoint = match snapshot.record(&RecordKey::new(Family::Models, id.as_bytes())) {
                Some(StrategicRecord::Neural { checkpoint, .. }) => checkpoint.as_ref().clone(),
                _ => deep::initial(
                    &game,
                    deep::Config {
                        seed: args[4].parse()?,
                        ..Default::default()
                    },
                )?,
            };
            if checkpoint.config.seed != args[4].parse::<u64>()? {
                return Err("resume requires the original seed".into());
            }
            let ms: u64 = args[5].parse()?;
            if ms == 0 || ms > 60000 {
                return Err("deadline must be 1..60000ms".into());
            }
            let report = deep::resume(
                &game,
                &checkpoint,
                args[3].parse()?,
                &machiavelli_core::Deadline::from_millis(ms),
            )?;
            let sequence = store.commit_at(
                snapshot.sequence(),
                &[Mutation::PutStrategic(StrategicRecord::Neural {
                    id,
                    game: Box::new(tree),
                    checkpoint: Box::new(report.checkpoint.clone()),
                })],
            )?;
            println!(
                "{}",
                serde_json::json!({"sequence":sequence,"model_id":id,"training":report})
            );
        }
        "init" | "stats" if args.len() == 2 => println!(
            "{}",
            serde_json::json!({
                "nodes":store.snapshot().len(), "strategic_records":store.snapshot().strategic_len(), "sequence":store.snapshot().sequence(),
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
        "plan" if args.len() == 7 => {
            let ms: u64 = args[6].parse()?;
            if ms == 0 || ms > 10000 {
                return Err("deadline must be 1..10000 ms".into());
            }
            let budget = machiavelli_centaur::planning::Budget {
                iterations: args[3].parse()?,
                horizon: args[4].parse()?,
                seed: args[5].parse()?,
            };
            let result = machiavelli_centaur::planning::plan(
                &store.snapshot(),
                args[2].parse()?,
                budget,
                &machiavelli_core::Deadline::from_millis(ms),
            )?;
            println!("{}", serde_json::to_string(&result)?);
        }
        "train-kuhn" if args.len() == 4 => {
            use machiavelli_storage::{Family, RecordKey, StrategicRecord};
            let iterations = args[2].parse()?;
            let ms: u64 = args[3].parse()?;
            if ms == 0 || ms > 10000 {
                return Err("deadline must be 1..10000 ms".into());
            }
            let game = machiavelli_cfr::extensive::kuhn();
            let mut checkpoint = game.initial_checkpoint();
            let snapshot = store.snapshot();
            let keys = checkpoint
                .infos
                .keys()
                .map(|name| (name.clone(), checkpoint.infoset_id(name)))
                .collect::<Vec<_>>();
            let mut restored = 0;
            let mut saved_iterations = None;
            for (name, id) in &keys {
                let definition = snapshot.record(&RecordKey::new(Family::Infosets, id.as_bytes()));
                let table = snapshot.record(&RecordKey::new(Family::Regrets, id.as_bytes()));
                match (definition, table) {
                    (None, None) => {}
                    (
                        Some(StrategicRecord::Infoset {
                            player, actions, ..
                        }),
                        Some(StrategicRecord::Regrets {
                            regrets,
                            strategy_sum,
                            iterations,
                            ..
                        }),
                    ) => {
                        let state = checkpoint.infos.get_mut(name).unwrap();
                        if state.player != *player
                            || state.actions != *actions
                            || saved_iterations.is_some_and(|n| n != *iterations)
                        {
                            return Err("inconsistent solver checkpoint".into());
                        }
                        state.regrets = regrets.clone();
                        state.strategy_sum = strategy_sum.clone();
                        saved_iterations = Some(*iterations);
                        restored += 1;
                    }
                    _ => return Err("incomplete solver checkpoint".into()),
                }
            }
            if restored != 0 && restored != keys.len() {
                return Err("incomplete solver checkpoint".into());
            }
            checkpoint.iterations = saved_iterations.unwrap_or(0);
            let result = game.resume(
                &checkpoint,
                iterations,
                &machiavelli_core::Deadline::from_millis(ms),
            )?;
            let mut operations = Vec::new();
            for (name, id) in keys {
                let info = &result.checkpoint.infos[&name];
                operations.push(Mutation::PutStrategic(StrategicRecord::Infoset {
                    id,
                    player: info.player,
                    actions: info.actions.clone(),
                }));
                operations.push(Mutation::PutStrategic(StrategicRecord::Regrets {
                    id,
                    regrets: info.regrets.clone(),
                    strategy_sum: info.strategy_sum.clone(),
                    iterations: result.checkpoint.iterations,
                }));
            }
            let sequence = store.commit_at(snapshot.sequence(), &operations)?;
            println!(
                "{}",
                serde_json::json!({"game":"kuhn","persisted":true,"sequence":sequence,"iterations":result.iterations,"cumulative_iterations":result.checkpoint.iterations,"value":result.value,"exploitability":result.exploitability,"strategies":result.strategies})
            );
        }
        "records" if args.len() == 2 => {
            println!(
                "{}",
                serde_json::to_string(&store.snapshot().records().collect::<Vec<_>>())?
            );
        }
        "commit" if matches!(args.len(), 3 | 4) => {
            let operations: Vec<Mutation> =
                serde_json::from_reader(std::fs::File::open(&args[2])?)?;
            if operations.len() > 4096 {
                return Err("transaction exceeds 4096 operations".into());
            }
            let sequence = if args.len() == 4 {
                store.commit_at(args[3].parse()?, &operations)?
            } else {
                store.commit(&operations)?
            };
            println!(
                "{}",
                serde_json::json!({"sequence":sequence,"operations":operations.len()})
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
