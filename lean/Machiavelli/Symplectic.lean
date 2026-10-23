import Std
import Std.Tactic

/-!
# Exact integer model of canonical symplectic identities

The coordinates below are mathematical integers of arbitrary size. These
proofs concern this reference model, not real-valued trajectories, IEEE-754
rounding, the Rust implementation, or the complete numerical integrators.
A kick or drift preserves the canonical form when its supplied integer
operator satisfies the explicitly stated self-adjointness hypothesis.
-/

namespace Machiavelli

abbrev Vec (n : Nat) := Fin n → Int

structure Phase (n : Nat) where
  q : Vec n
  p : Vec n

def negPhase {n : Nat} (z : Phase n) : Phase n :=
  ⟨fun i => -z.q i, fun i => -z.p i⟩

def J {n : Nat} (z : Phase n) : Phase n :=
  ⟨z.p, fun i => -z.q i⟩

theorem j_squared {n : Nat} (z : Phase n) : J (J z) = negPhase z := by
  cases z
  simp [J, negPhase]

/-- Finite exact dot product, defined without an external algebra library. -/
def dot : {n : Nat} → Vec n → Vec n → Int
  | 0, _, _ => 0
  | n + 1, x, y =>
      x 0 * y 0 + dot (fun i : Fin n => x i.succ) (fun i : Fin n => y i.succ)

theorem dot_comm {n : Nat} (x y : Vec n) : dot x y = dot y x := by
  induction n with
  | zero => rfl
  | succ n ih =>
      simp only [dot]
      rw [Int.mul_comm (x 0) (y 0), ih]

theorem dot_add_left {n : Nat} (x y z : Vec n) :
    dot (fun i => x i + y i) z = dot x z + dot y z := by
  induction n with
  | zero => rfl
  | succ n ih =>
      simp only [dot, Int.add_mul]
      rw [ih]
      omega

theorem dot_add_right {n : Nat} (x y z : Vec n) :
    dot x (fun i => y i + z i) = dot x y + dot x z := by
  rw [dot_comm, dot_add_left, dot_comm y x, dot_comm z x]

/-- omega(u,v) = q_u dot p_v - p_u dot q_v. -/
def omega {n : Nat} (u v : Phase n) : Int :=
  dot u.q v.p - dot u.p v.q

theorem omega_alternating {n : Nat} (z : Phase n) : omega z z = 0 := by
  unfold omega
  rw [dot_comm z.q z.p]
  omega

theorem omega_antisymmetric {n : Nat} (u v : Phase n) :
    omega u v = -omega v u := by
  have hqp := dot_comm u.q v.p
  have hpq := dot_comm u.p v.q
  unfold omega
  omega

/-- Self-adjointness is an explicit hypothesis on the exact integer operator. -/
def SelfAdjoint {n : Nat} (operator : Vec n → Vec n) : Prop :=
  ∀ x y, dot x (operator y) = dot (operator x) y

def kick {n : Nat} (operator : Vec n → Vec n) (z : Phase n) : Phase n :=
  ⟨z.q, fun i => z.p i + operator z.q i⟩

def drift {n : Nat} (operator : Vec n → Vec n) (z : Phase n) : Phase n :=
  ⟨fun i => z.q i + operator z.p i, z.p⟩

def PreservesOmega {n : Nat} (transform : Phase n → Phase n) : Prop :=
  ∀ u v, omega (transform u) (transform v) = omega u v

theorem kick_preserves_omega {n : Nat} (operator : Vec n → Vec n)
    (selfAdjoint : SelfAdjoint operator) : PreservesOmega (kick operator) := by
  intro u v
  change dot u.q (fun i => v.p i + operator v.q i) -
    dot (fun i => u.p i + operator u.q i) v.q = omega u v
  rw [dot_add_right, dot_add_left]
  have h := selfAdjoint u.q v.q
  unfold omega
  omega

theorem drift_preserves_omega {n : Nat} (operator : Vec n → Vec n)
    (selfAdjoint : SelfAdjoint operator) : PreservesOmega (drift operator) := by
  intro u v
  change dot (fun i => u.q i + operator u.p i) v.p -
    dot u.p (fun i => v.q i + operator v.p i) = omega u v
  rw [dot_add_left, dot_add_right]
  have h := selfAdjoint u.p v.p
  unfold omega
  omega

theorem composition_preserves_omega {n : Nat} (first second : Phase n → Phase n)
    (firstPreserves : PreservesOmega first)
    (secondPreserves : PreservesOmega second) :
    PreservesOmega (fun z => first (second z)) := by
  intro u v
  rw [firstPreserves, secondPreserves]

#print axioms j_squared
#print axioms omega_alternating
#print axioms omega_antisymmetric
#print axioms kick_preserves_omega
#print axioms drift_preserves_omega
#print axioms composition_preserves_omega

end Machiavelli
