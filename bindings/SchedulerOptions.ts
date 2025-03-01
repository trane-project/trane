// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { MasteryWindow } from "./MasteryWindow";
import type { PassingScoreOptions } from "./PassingScoreOptions";

/**
 * Options to control how the scheduler selects exercises.
 */
export type SchedulerOptions = { 
/**
 * The maximum number of candidates to return each time the scheduler is called.
 */
batch_size: number, 
/**
 * The options of the new mastery window. That is, the window of exercises that have not
 * received a score so far.
 */
new_window_opts: MasteryWindow, 
/**
 * The options of the target mastery window. That is, the window of exercises that lie outside
 * the user's current abilities.
 */
target_window_opts: MasteryWindow, 
/**
 * The options of the current mastery window. That is, the window of exercises that lie
 * slightly outside the user's current abilities.
 */
current_window_opts: MasteryWindow, 
/**
 * The options of the easy mastery window. That is, the window of exercises that lie well
 * within the user's current abilities.
 */
easy_window_opts: MasteryWindow, 
/**
 * The options for the mastered mastery window. That is, the window of exercises that the user
 * has properly mastered.
 */
mastered_window_opts: MasteryWindow, 
/**
 * The minimum average score of a unit required to move on to its dependents.
 */
passing_score: PassingScoreOptions, 
/**
 * The minimum score required to supersede a unit. If unit A is superseded by B, then the
 * exercises from unit A will not be shown once the score of unit B is greater than or equal to
 * this value.
 */
superseding_score: number, 
/**
 * The number of trials to retrieve from the practice stats to compute an exercise's score.
 */
num_trials: number, 
/**
 * The number of rewards to retrieve from the practice rewards to compute a unit's reward.
 */
num_rewards: number, };
