import Std
import Std.Tactic

/-! Exact integer reference lemmas. These do not verify floating-point kernels,
general VCG allocation, or convergence rates of extensive-form CFR+. -/
namespace Machiavelli

def clippedRegret (old delta : Int) : Int := max 0 (old + delta)
theorem clipped_regret_nonnegative (old delta : Int) :
    0 <= clippedRegret old delta := by
  simp only [clippedRegret]
  omega

def regretSequence (deltas : List Int) : Int :=
  deltas.foldl clippedRegret 0

theorem regret_fold_nonnegative (deltas : List Int) (initial : Int)
    (h : 0 <= initial) : 0 <= deltas.foldl clippedRegret initial := by
  induction deltas generalizing initial with
  | nil => exact h
  | cons delta rest ih =>
      exact ih (clippedRegret initial delta) (clipped_regret_nonnegative initial delta)

theorem cfr_plus_regrets_stay_nonnegative (deltas : List Int) :
    0 <= regretSequence deltas := by
  exact regret_fold_nonnegative deltas 0 (by omega)

/-- Single-item second-price utility; ties win at the threshold. -/
def auctionUtility (value bid threshold : Int) : Int :=
  if threshold <= bid then value - threshold else 0

theorem second_price_truthful (value bid threshold : Int) :
    auctionUtility value bid threshold <= auctionUtility value value threshold := by
  unfold auctionUtility
  split <;> split <;> omega

#print axioms clipped_regret_nonnegative
#print axioms cfr_plus_regrets_stay_nonnegative
#print axioms second_price_truthful

end Machiavelli
