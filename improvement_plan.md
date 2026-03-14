# Improvement Plan

## 1. Adaptive Interleaving (Blocked-to-Interleaved Transition)

### Motivation

Meta-analysis (Brunmair & Richter 2019) shows blocking is superior for initial
rule-based learning (d = 0.76 advantage), while interleaving is superior for
discrimination between learned skills (d = 0.65 advantage). The recommendation is
a blocked-to-interleaved progression as proficiency grows.

Currently, trane shuffles the final exercise batch uniformly (`scheduler.rs:1019`).
There is no mechanism to present exercises from the same lesson in contiguous
blocks when the student is still learning that lesson.

### Design

Replace the final shuffle in `candidates_to_exercises()` with a block-aware
ordering function. The key idea: group exercises by lesson, chunk each lesson's
exercises into blocks whose size is inversely proportional to the lesson score,
shuffle the blocks, then flatten.

#### Block size calculation

For each lesson represented in the batch, compute a block size:

```
block_size = max(1, ceil(num_exercises_from_lesson * (1.0 - lesson_score / interleave_threshold)))
```

Where `interleave_threshold` is the lesson score at which exercises become fully
interleaved (block size = 1). A natural choice is `passing_score` (default 3.0),
since that's the existing concept of "has basic competence."

- Lesson score 0.0 -> block_size = num_exercises (fully blocked)
- Lesson score 1.5 -> block_size = ceil(num_exercises * 0.5) (half-sized blocks)
- Lesson score >= 3.0 -> block_size = 1 (fully interleaved, same as current behavior)

#### Algorithm

```
fn block_interleave(candidates: Vec<Candidate>, interleave_threshold: f32) -> Vec<Candidate> {
    // 1. Group candidates by lesson_id.
    let groups: HashMap<Ustr, Vec<Candidate>> = group_by_lesson(candidates);

    // 2. For each lesson, shuffle exercises within the lesson, then chunk into
    //    blocks based on lesson score.
    let mut all_blocks: Vec<Vec<Candidate>> = Vec::new();
    for (lesson_id, mut exercises) in groups {
        exercises.shuffle(&mut rng());
        let lesson_score = exercises[0].lesson_score;
        let ratio = (1.0 - lesson_score / interleave_threshold).max(0.0);
        let block_size = (exercises.len() as f32 * ratio).ceil().max(1.0) as usize;
        for chunk in exercises.chunks(block_size) {
            all_blocks.push(chunk.to_vec());
        }
    }

    // 3. Shuffle the blocks.
    all_blocks.shuffle(&mut rng());

    // 4. Flatten into final list.
    all_blocks.into_iter().flatten().collect()
}
```

#### Where to change

- `scheduler.rs:1008-1021` (`candidates_to_exercises`): Replace the shuffle with
  the block-interleave function. This requires passing `Candidate` (which already
  has `lesson_id` and `lesson_score`) instead of converting to `ExerciseManifest`
  before ordering. Reorder the function to: block-interleave candidates first, then
  convert to manifests.

- `data.rs`: Add `interleave_threshold: f32` to `SchedulerOptions` with a default
  equal to `passing_score_v2.min_score`. Add validation in `SchedulerOptions::verify`.

#### Testing

- Unit test: given a batch with exercises from lessons at varying scores, verify
  that low-score lessons produce contiguous blocks while high-score lessons produce
  individual items.
- Unit test: at lesson_score = 0, all exercises from that lesson are in one block.
- Unit test: at lesson_score >= threshold, block_size = 1 for all lessons (same as
  a shuffle — the order is random but no grouping).
- Integration test: run a full scheduling pass and verify the output ordering
  respects block structure.
