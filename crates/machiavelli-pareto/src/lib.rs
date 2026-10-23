//! Explicit benefit-maximizing and risk-minimizing objectives.
//! Fractional Kelly limits exposure under the binary payoff model; it does not
//! guarantee zero real-world ruin under misspecification or correlated losses.
#[derive(Clone, Debug)]
pub struct Point {
    benefits: Vec<f64>,
    risk: f64,
}
impl Point {
    pub fn new(benefits: Vec<f64>, risk: f64) -> Result<Self, &'static str> {
        if benefits.is_empty()
            || benefits.iter().any(|v| !v.is_finite())
            || !risk.is_finite()
            || risk < 0.0
        {
            return Err("invalid objectives");
        }
        Ok(Self { benefits, risk })
    }
    pub fn dominates(&self, other: &Self) -> Result<bool, &'static str> {
        if self.benefits.len() != other.benefits.len() {
            return Err("objective dimension mismatch");
        }
        Ok(self.risk <= other.risk
            && self
                .benefits
                .iter()
                .zip(&other.benefits)
                .all(|(a, b)| a >= b)
            && (self.risk < other.risk
                || self
                    .benefits
                    .iter()
                    .zip(&other.benefits)
                    .any(|(a, b)| a > b)))
    }
}
pub fn frontier(points: &[Point]) -> Result<Vec<usize>, &'static str> {
    let mut result = Vec::new();
    for (i, p) in points.iter().enumerate() {
        let mut dominated = false;
        for (j, q) in points.iter().enumerate() {
            if i != j && q.dominates(p)? {
                dominated = true;
                break;
            }
        }
        if !dominated {
            result.push(i);
        }
    }
    Ok(result)
}
pub fn kelly(probability: f64, net_odds: f64, fraction: f64) -> Result<f64, &'static str> {
    if !probability.is_finite()
        || !(0.0..=1.0).contains(&probability)
        || !net_odds.is_finite()
        || net_odds <= 0.0
        || !fraction.is_finite()
        || !(0.25..=0.5).contains(&fraction)
    {
        return Err("invalid Kelly parameters");
    }
    Ok((fraction * (probability - (1.0 - probability) / net_odds)).clamp(0.0, fraction))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pareto_keeps_tradeoffs() {
        let points = vec![
            Point::new(vec![2.0], 1.0).unwrap(),
            Point::new(vec![1.0], 2.0).unwrap(),
            Point::new(vec![3.0], 3.0).unwrap(),
        ];
        assert_eq!(frontier(&points).unwrap(), vec![0, 2]);
    }
    #[test]
    fn kelly_never_shorts_or_overallocates() {
        assert!((kelly(0.6, 1.0, 0.5).unwrap() - 0.1).abs() < 1e-14);
        assert_eq!(kelly(0.1, 1.0, 0.5).unwrap(), 0.0);
        assert_eq!(kelly(1.0, 1.0, 0.5).unwrap(), 0.5);
        assert!(kelly(f64::NAN, 1.0, 0.5).is_err());
    }
}
