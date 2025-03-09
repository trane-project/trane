// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { BasicAsset } from "./BasicAsset";
import type { CourseGenerator } from "./CourseGenerator";

/**
 * A manifest describing the contents of a course.
 */
export type CourseManifest = { 
/**
 * The ID assigned to this course.
 *
 * For example, `music::instrument::guitar::basic_jazz_chords`.
 */
id: string, 
/**
 * The name of the course to be presented to the user.
 *
 * For example, "Basic Jazz Chords on Guitar".
 */
name: string, 
/**
 * The IDs of all dependencies of this course.
 */
dependencies: Array<string>, 
/**
 * The IDs of the courses or lessons that this course supersedes. If this course is mastered,
 * then exercises from the superseded courses or lessons will no longer be shown to the
 * student.
 */
superseded: Array<string>, 
/**
 * An optional description of the course.
 */
description: string | null, 
/**
 * An optional list of the course's authors.
 */
authors: Array<string> | null, 
/**
 * be attached to a course named "Basic Jazz Chords on Guitar".
 *
 * The purpose of this metadata is to allow students to focus on more specific material during
 * a study session which does not belong to a single lesson or course. For example, a student
 * might want to only focus on guitar scales or ear training.
 */
metadata: { [key: string]: Array<string> } | null, 
/**
 * An optional asset, which presents the material covered in the course.
 */
course_material: BasicAsset | null, 
/**
 * An optional asset, which presents instructions common to all exercises in the course.
 */
course_instructions: BasicAsset | null, 
/**
 * An optional configuration to generate material for this course. Generated courses allow
 * easier creation of courses for specific purposes without requiring the manual creation of
 * all the files a normal course would need.
 */
generator_config: CourseGenerator | null, };
