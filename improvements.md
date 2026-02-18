- Recency-weighted difficulty (small): update difficulty estimation to weight recent failures more
  and treat 1 as worse than 2, instead of flat failure-rate counting (src/exercise_scorer.rs:190).
- Confidence damping for sparse history (small): scale final score by trial-count confidence so 1–2
  good attempts do not look over-mastered (src/exercise_scorer.rs:487).
- Massed-practice discount (small): reduce stability growth when reviews happen too close together
  (same session retries should not boost long-term stability much) (src/exercise_scorer.rs:317).
- Adaptive old-good floor (small): keep the floor idea, but make it depend on exercise type and
  number of trials rather than fixed constants (src/exercise_scorer.rs:424).
- Numerical guardrails (tiny): clamp extreme elapsed-day values and non-finite intermediates around
  retrievability projection for safer edge behavior (src/exercise_scorer.rs:411,
  src/exercise_scorer.rs:465).