// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { ExerciseAsset } from "./ExerciseAsset";
import type { ExerciseType } from "./ExerciseType";

/**
 * Manifest describing a single exercise.
 */
export type ExerciseManifest = { 
/**
 * The ID assigned to this exercise.
 *
 * For example, `music::instrument::guitar::basic_jazz_chords::major_chords::exercise_1`.
 */
id: string, 
/**
 * The ID of the lesson to which this exercise belongs.
 */
lesson_id: string, 
/**
 * The ID of the course to which this exercise belongs.
 */
course_id: string, 
/**
 * The name of the exercise to be presented to the user.
 *
 * For example, "Exercise 1".
 */
name: string, 
/**
 * An optional description of the exercise.
 */
description: string | null, 
/**
 * The type of knowledge the exercise tests.
 */
exercise_type: ExerciseType, 
/**
 * The asset containing the exercise itself.
 */
exercise_asset: ExerciseAsset, };