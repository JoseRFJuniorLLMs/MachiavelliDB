//! Clarke-pivot VCG over an explicit finite set of feasible resource allocations.
//! Reports are valuations per bidder/outcome, not independent per-resource bids.
//! Strategy-proofness assumes quasi-linear utility and truthful feasible outcomes.
#[derive(Debug)]
pub struct Allocation {
    pub outcome: usize,
    pub payments: Vec<f64>,
    pub welfare: f64,
}
pub fn allocate(valuations: &[Vec<f64>]) -> Result<Allocation, &'static str> {
    let outcomes = valuations.first().ok_or("no bidders")?.len();
    if outcomes == 0
        || valuations
            .iter()
            .any(|v| v.len() != outcomes || v.iter().any(|x| !x.is_finite() || *x < 0.0))
    {
        return Err("invalid valuations");
    }
    let welfare: Vec<f64> = (0..outcomes)
        .map(|a| valuations.iter().map(|v| v[a]).sum())
        .collect();
    if welfare.iter().any(|v| !v.is_finite()) {
        return Err("welfare overflow");
    }
    let outcome = (0..outcomes)
        .max_by(|&a, &b| welfare[a].total_cmp(&welfare[b]).then_with(|| b.cmp(&a)))
        .unwrap();
    let mut payments = Vec::new();
    for i in 0..valuations.len() {
        // Sum others directly, avoiding cancellation of a large winning bid.
        let others: Vec<f64> = (0..outcomes)
            .map(|a| {
                valuations
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, v)| v[a])
                    .sum()
            })
            .collect();
        let externality = others.iter().copied().fold(0.0_f64, f64::max) - others[outcome];
        payments.push(externality.max(0.0));
    }
    Ok(Allocation {
        outcome,
        payments,
        welfare: welfare[outcome],
    })
}
pub fn brier(probability: f64, outcome: bool) -> Result<f64, &'static str> {
    if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
        return Err("invalid probability");
    }
    let error = probability - f64::from(outcome);
    Ok(1.0 - error * error)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_item_is_second_price() {
        let result = allocate(&[vec![0.0, 10.0, 0.0], vec![0.0, 0.0, 7.0]]).unwrap();
        assert_eq!(result.outcome, 1);
        assert_eq!(result.payments, vec![7.0, 0.0]);
        for lie in 0..20 {
            let alternative = allocate(&[vec![0.0, lie as f64, 0.0], vec![0.0, 0.0, 7.0]]).unwrap();
            let utility =
                if alternative.outcome == 1 { 10.0 } else { 0.0 } - alternative.payments[0];
            assert!(utility <= 3.0);
        }
    }
    #[test]
    fn brier_expected_score_is_maximized_by_truth() {
        let p = 0.7;
        let expected = |q| p * brier(q, true).unwrap() + (1.0 - p) * brier(q, false).unwrap();
        for i in 0..=100 {
            assert!(expected(i as f64 / 100.0) <= expected(p) + 1e-14);
        }
        assert!(allocate(&[vec![f64::NAN]]).is_err());
    }
}
