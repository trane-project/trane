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

## 2. Max Lessons In Progress (Cognitive Load Management)

### Motivation

Cognitive load theory (Sweller 1988, 2011) and curriculum sequencing research show
that splitting attention across too many new topics simultaneously degrades
learning. Working memory can hold ~4 novel chunks at a time. Each new lesson
introduces its own concepts and patterns that occupy working memory during
practice. Consolidated material (already-learned lessons) does not have this cost.

Currently, trane's DFS traversal can spread candidates across an unbounded number
of low-mastery lessons. If the dependency graph has many lessons with satisfied
prerequisites, the batch may contain new exercises from a large number of lessons
simultaneously, each getting only a few exercises. The existing course frequency
weight in `filter.rs` penalizes overrepresentation of a single course but does not
limit how many distinct lessons contribute to the batch — it actually encourages
even spreading, which is the opposite of concentration.

### Design

Add a `max_lessons_in_progress` parameter to `SchedulerOptions` that caps how many
distinct lessons with scores below `passing_score` can contribute candidates to a
single batch. Once the cap is reached, exercises from additional low-score lessons
are skipped, but the DFS continues traversing to find exercises from
already-in-progress lessons and review material.

#### Algorithm

During candidate generation in the DFS (`get_candidates_from_lesson_helper`):

```
// Maintained across the entire DFS traversal.
let mut in_progress_lessons: UstrSet = UstrSet::default();

// For each lesson encountered during traversal:
fn should_add_candidates(lesson_id: Ustr, lesson_score: f32, options: &SchedulerOptions) -> bool {
    if lesson_score >= options.passing_score.min_score {
        // Already passed — this is review material, always include.
        return true;
    }
    if in_progress_lessons.contains(&lesson_id) {
        // Already tracked as in-progress, continue contributing.
        return true;
    }
    if in_progress_lessons.len() >= options.max_lessons_in_progress {
        // Limit reached — skip this lesson's exercises.
        return false;
    }
    // New in-progress lesson within the limit.
    in_progress_lessons.insert(lesson_id);
    true
}
```

Key behaviors:

- Lessons already above `passing_score` are unaffected — they are review material
  and do not count toward the limit.
- Lessons already being tracked as in-progress continue to contribute exercises.
- New lessons that would exceed the limit are skipped, but the DFS continues to
  other branches to find review material and already-in-progress lessons.
- The DFS already does not traverse past lessons below `passing_score` to their
  dependents, so skipping a lesson's exercises is equivalent to stopping that
  branch.

#### Where to change

- `data.rs`: Add `max_lessons_in_progress: usize` to `SchedulerOptions` with a
  default of 5. Add validation in `SchedulerOptions::verify`.

- `scheduler.rs` (DFS traversal, `get_candidates_from_lesson_helper` and callers):
  Thread an `in_progress_lessons: &mut UstrSet` through the traversal. Before
  adding candidates from a lesson, check `should_add_candidates`. If it returns
  false, skip candidate generation for that lesson but continue the DFS.

#### Testing

- Unit test: with `max_lessons_in_progress = 3` and 10 low-score lessons
  available, verify that candidates come from at most 3 distinct lessons.
- Unit test: lessons above `passing_score` are always included regardless of the
  limit.
- Unit test: once a lesson is counted as in-progress, it continues contributing
  in subsequent batches (the set is per-traversal, not persistent).
- Unit test: with `max_lessons_in_progress` set higher than the number of
  available lessons, all lessons contribute (no change from current behavior).
- Integration test: run a full scheduling pass with a wide graph and verify the
  batch respects the lesson count limit.
