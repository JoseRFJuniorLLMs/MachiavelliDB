//! Typed Prince Query Language front end. Mutation ASTs are plans; execution
//! must pass constitutional, quorum and durable intent gates in the host.
use machiavelli_core::NodeId;
use pest::Parser;
use pest_derive::Parser;
#[derive(Parser)]
#[grammar_inline = r#"
WHITESPACE = _{ " " | "\t" | "\r" | "\n" }
query = { SOI ~ command ~ deadline? ~ EOI }
command = _{ scout | wargame | fortuna | dike | fortify | ally | betray | deceive | conquer | tribute | prince_mcts | prince }
id = @{ ASCII_HEX_DIGIT{8} ~ "-" ~ ASCII_HEX_DIGIT{4} ~ "-" ~ ASCII_HEX_DIGIT{4} ~ "-" ~ ASCII_HEX_DIGIT{4} ~ "-" ~ ASCII_HEX_DIGIT{12} }
uint = @{ ASCII_DIGIT+ }
number = @{ "-"? ~ ASCII_DIGIT+ ~ ("." ~ ASCII_DIGIT+)? ~ (("e"|"E") ~ ("+"|"-")? ~ ASCII_DIGIT+)? }
scout = { "SCOUT" ~ ("ALL"|id) }
wargame = { "WARGAME" ~ id ~ "STEPS" ~ uint ~ "DT" ~ number }
fortuna = { "FORTUNA" ~ id ~ "AMPLITUDE" ~ number }
dike = { "DIKE" ~ id ~ "ABOVE" ~ number ~ "TTL" ~ uint }
fortify = { "FORTIFY" ~ id }
ally = { "ALLY" ~ id ~ "WITH" ~ id }
betray = { "BETRAY" ~ id }
deceive = { "DECEIVE" ~ id }
conquer = { "CONQUER" ~ id }
tribute = { "TRIBUTE" ~ id ~ "AMOUNT" ~ number }
prince_mcts = { "PRINCE" ~ "MCTS" ~ id ~ "ITERATIONS" ~ uint ~ "HORIZON" ~ uint ~ "SEED" ~ uint }
prince = { "PRINCE" ~ "KUHN" ~ "ITERATIONS" ~ uint }
deadline = { "DEADLINE" ~ uint }
"#]
struct Grammar;
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Scout(Option<NodeId>),
    Wargame {
        target: NodeId,
        steps: usize,
        dt: f64,
    },
    Fortuna {
        target: NodeId,
        amplitude: f64,
    },
    Dike {
        target: NodeId,
        above: f64,
        ttl_ms: u64,
    },
    Fortify(NodeId),
    Ally {
        target: NodeId,
        other: NodeId,
    },
    Betray(NodeId),
    Deceive(NodeId),
    Conquer(NodeId),
    Tribute {
        target: NodeId,
        amount: f64,
    },
    PrinceMcts {
        target: NodeId,
        iterations: usize,
        horizon: usize,
        seed: u64,
    },
    PrinceKuhn {
        iterations: usize,
    },
}
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub command: Command,
    pub deadline_ms: u64,
}
pub fn parse(input: &str) -> Result<Query, String> {
    if input.len() > 4096 {
        return Err("query too large".into());
    }
    let root = Grammar::parse(Rule::query, input)
        .map_err(|e| e.to_string())?
        .next()
        .ok_or("empty query")?;
    let mut fields = root.into_inner();
    let command = fields.next().ok_or("missing command")?;
    let rule = command.as_rule();
    let args: Vec<&str> = command.into_inner().map(|p| p.as_str()).collect();
    let id = |i: usize| args[i].parse::<NodeId>().map_err(|e| e.to_string());
    let uint = |i: usize| args[i].parse::<u64>().map_err(|e| e.to_string());
    let number = |i: usize| -> Result<f64, String> {
        let v = args[i].parse::<f64>().map_err(|e| e.to_string())?;
        if !v.is_finite() {
            Err("nonfinite numeric argument".into())
        } else {
            Ok(v)
        }
    };
    let command = match rule {
        Rule::scout => Command::Scout(if args.is_empty() { None } else { Some(id(0)?) }),
        Rule::wargame => {
            let steps = uint(1)?;
            let dt = number(2)?;
            if steps > 1_000_000 || dt.abs() > 1.0 {
                return Err("wargame exceeds numerical/resource bounds".into());
            }
            Command::Wargame {
                target: id(0)?,
                steps: steps as usize,
                dt,
            }
        }
        Rule::fortuna => {
            let amplitude = number(1)?;
            if amplitude < 0.0 {
                return Err("negative amplitude".into());
            }
            Command::Fortuna {
                target: id(0)?,
                amplitude,
            }
        }
        Rule::dike => {
            let ttl_ms = uint(2)?;
            if ttl_ms == 0 {
                return Err("zero TTL".into());
            }
            Command::Dike {
                target: id(0)?,
                above: number(1)?,
                ttl_ms,
            }
        }
        Rule::fortify => Command::Fortify(id(0)?),
        Rule::ally => Command::Ally {
            target: id(0)?,
            other: id(1)?,
        },
        Rule::betray => Command::Betray(id(0)?),
        Rule::deceive => Command::Deceive(id(0)?),
        Rule::conquer => Command::Conquer(id(0)?),
        Rule::tribute => {
            let amount = number(1)?;
            if amount < 0.0 {
                return Err("negative tribute".into());
            }
            Command::Tribute {
                target: id(0)?,
                amount,
            }
        }
        Rule::prince_mcts => {
            let iterations = uint(1)?;
            let horizon = uint(2)?;
            if iterations == 0 || iterations > 100000 || horizon == 0 || horizon > 32 {
                return Err("MCTS requires iterations 1..100000 and horizon 1..32".into());
            }
            Command::PrinceMcts {
                target: id(0)?,
                iterations: iterations as usize,
                horizon: horizon as usize,
                seed: uint(3)?,
            }
        }
        Rule::prince => {
            let n = uint(0)?;
            if n == 0 || n > 1_000_000 {
                return Err("invalid iteration limit".into());
            }
            Command::PrinceKuhn {
                iterations: n as usize,
            }
        }
        _ => return Err("unsupported grammar production".into()),
    };
    let expected_tokens = match rule {
        Rule::scout | Rule::fortify | Rule::betray | Rule::deceive | Rule::conquer => 2,
        Rule::wargame | Rule::dike => 6,
        Rule::prince_mcts => 9,
        _ => 4,
    };
    let mut has_deadline = false;
    let mut deadline_ms = 1000;
    for field in fields {
        if field.as_rule() == Rule::deadline {
            has_deadline = true;
            deadline_ms = field
                .into_inner()
                .next()
                .ok_or("missing deadline")?
                .as_str()
                .parse::<u64>()
                .map_err(|e| e.to_string())?;
        }
    }
    if deadline_ms == 0 || deadline_ms > 10_000 {
        return Err("deadline must be 1..10000 ms".into());
    }
    if input.split_whitespace().count() != expected_tokens + if has_deadline { 2 } else { 0 } {
        return Err("PQL tokens require whitespace separation".into());
    }
    Ok(Query {
        command,
        deadline_ms,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    const ID: &str = "6a54d3ce-57db-476e-bc63-4d26baf20450";
    #[test]
    fn all_verbs_parse_into_typed_plans() {
        for command in [
            "SCOUT ALL".into(),
            format!("SCOUT {ID}"),
            format!("WARGAME {ID} STEPS 100 DT 0.01"),
            format!("FORTUNA {ID} AMPLITUDE 0.5"),
            format!("DIKE {ID} ABOVE 3 TTL 1000"),
            format!("FORTIFY {ID}"),
            format!("ALLY {ID} WITH {ID}"),
            format!("BETRAY {ID}"),
            format!("DECEIVE {ID}"),
            format!("CONQUER {ID}"),
            format!("TRIBUTE {ID} AMOUNT 2"),
            "PRINCE KUHN ITERATIONS 1000".into(),
        ] {
            assert!(parse(&command).is_ok(), "{command}: {:?}", parse(&command));
        }
        assert_eq!(parse("SCOUT ALL DEADLINE 200").unwrap().deadline_ms, 200);
    }
    #[test]
    fn parser_rejects_trailing_text_overflow_and_invalid_semantics() {
        for command in [
            "SCOUT ALL extra".into(),
            "SCOUTALL".into(),
            "SCOUT ALL DEADLINE 0".into(),
            format!("WARGAME {ID} STEPS 9999999999999999999999 DT 1"),
            format!("TRIBUTE {ID} AMOUNT -1"),
            format!("FORTUNA {ID} AMPLITUDE 1e999"),
        ] {
            assert!(parse(&command).is_err(), "{command}");
        }
    }
}
#[cfg(test)]
mod mcts_tests {
    use super::*;
    const ID: &str = "6a54d3ce-57db-476e-bc63-4d26baf20450";
    #[test]
    fn mcts_budget_and_seed_are_parsed_without_weakening_token_bounds() {
        let query = parse(&format!(
            "PRINCE MCTS {ID} ITERATIONS 2000 HORIZON 8 SEED 42 DEADLINE 1000"
        ))
        .unwrap();
        assert_eq!(
            query.command,
            Command::PrinceMcts {
                target: ID.parse().unwrap(),
                iterations: 2000,
                horizon: 8,
                seed: 42
            }
        );
        for suffix in [
            "ITERATIONS 0 HORIZON 8 SEED 0",
            "ITERATIONS 100001 HORIZON 8 SEED 0",
            "ITERATIONS 1 HORIZON 33 SEED 0",
            "ITERATIONS 1 HORIZON 0 SEED 0",
            "ITERATIONS1 HORIZON 8 SEED 0",
            "ITERATIONS 1 HORIZON 8 SEED -1",
        ] {
            assert!(parse(&format!("PRINCE MCTS {ID} {suffix}")).is_err());
        }
        assert!(
            parse(&format!(
                "PRINCE MCTS {ID} ITERATIONS 1 HORIZON 1 SEED 18446744073709551615"
            ))
            .is_ok()
        );
    }
}
