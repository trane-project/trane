// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { BasicAsset } from "./BasicAsset";

/**
 * A manifest describing the contents of a lesson.
 */
export type LessonManifest = { 
/**
 * The ID assigned to this lesson.
 *
 * For example, `music::instrument::guitar::basic_jazz_chords::major_chords`.
 */
id: string, 
/**
 * The IDs of all dependencies of this lesson.
 */
dependencies: Array<string>, 
/**
 * A map of dependency IDs to an value representing the weight of the dependency when
 * calculating rewards. The dependencies with the highest value will receive the full reward,
 * while those with a lower value will receive a proportional part of the reward. For example,
 * if two dependencies, with weights 2 and 1, are assigned a reward of 4, then the first
 * dependency will receive a reward of 4, while the second will receive a reward of 2. A value
 * of 0 will disable propagations of rewards along that edge in the unit graph. If not set,
 * every dependency will be assigned a weight of 1.
 */
dependency_weights: { [key: string]: number } | null, 
/**
 *The IDs of the courses or lessons that this lesson supersedes. If this lesson is mastered,
 * then exercises from the superseded courses or lessons will no longer be shown to the
 * student.
 */
superseded: Array<string>, 
/**
 * The ID of the course to which the lesson belongs.
 */
course_id: string, 
/**
 * The name of the lesson to be presented to the user.
 *
 * For example, "Basic Jazz Major Chords".
 */
name: string, 
/**
 * An optional description of the lesson.
 */
description: string | null, 
/**
 * be attached to a lesson named "C Major Scale". The purpose is the same as the metadata
 * stored in the course manifest but allows finer control over which lessons are selected.
 */
metadata: { [key: string]: Array<string> } | null, 
/**
 * An optional asset, which presents the material covered in the lesson.
 */
lesson_material: BasicAsset | null, 
/**
 * An optional asset, which presents instructions common to all exercises in the lesson.
 */
lesson_instructions: BasicAsset | null, };
