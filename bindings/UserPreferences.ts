// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { SchedulerPreferences } from "./SchedulerPreferences";
import type { TranscriptionPreferences } from "./TranscriptionPreferences";

/**
 * The user-specific configuration
 */
export type UserPreferences = { 
/**
 * The preferences for generating transcription courses.
 */
transcription: TranscriptionPreferences | null, 
/**
 * The preferences for customizing the behavior of the scheduler.
 */
scheduler: SchedulerPreferences | null, 
/**
 * The paths to ignore when opening the course library. The paths are relative to the
 * repository root. All child paths are also ignored. For example, adding the directory
 * "foo/bar" will ignore any courses in "foo/bar" or any of its subdirectories.
 */
ignored_paths: Array<string>, };
