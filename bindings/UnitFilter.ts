// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { KeyValueFilter } from "./KeyValueFilter";

/**
 * A filter on a course or lesson manifest.
 */
export type UnitFilter = { "CourseFilter": { 
/**
 * The IDs of the courses to filter.
 */
course_ids: Array<string>, } } | { "LessonFilter": { 
/**
 * The IDs of the lessons to filter.
 */
lesson_ids: Array<string>, } } | { "MetadataFilter": { 
/**
 * The filter to apply to the course or lesson metadata.
 */
filter: KeyValueFilter, } } | "ReviewListFilter" | { "Dependents": { 
/**
 * The IDs of the units from which to start the search.
 */
unit_ids: Array<string>, } } | { "Dependencies": { 
/**
 * The IDs from which to look up the dependencies.
 */
unit_ids: Array<string>, 
/**
 * The depth of the dependency tree to search.
 */
depth: number, } };
