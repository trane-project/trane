// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

/**
 * The configuration to create a course that teaches literacy based on the provided material.
 * Material can be of two types.
 *
 * 1. Examples. For example, they can be words that share the same spelling and pronunciation (e.g.
 *    "cat", "bat", "hat"), sentences that share similar words, or sentences from the same book or
 *    article (for more advanced courses).
 * 2. Exceptions. For example, they can be words that share the same spelling but have different
 *    pronunciations (e.g. "cow" and "crow").
 *
 * All examples and exceptions accept markdown syntax. Examples and exceptions can be declared in
 * the configuration or in separate files in the course's directory. Files that end with the
 * extensions ".examples.md" and ".exceptions.md" will be considered as examples and exceptions,
 * respectively.
 */
export type LiteracyConfig = { 
/**
 * The dependencies on other literacy courses. Specifying these dependencies here instead of
 * the [CourseManifest] allows Trane to generate more fine-grained dependencies.
 */
literacy_dependencies: Array<string>, 
/**
 * Inlined examples to use in the course.
 */
inline_examples: Array<string>, 
/**
 * Inlined exceptions to use in the course.
 */
inline_exceptions: Array<string>, 
/**
 * Whether to generate an optional lesson that asks the student to write the material based on
 * the tutor's dictation.
 */
generate_dictation: boolean, };
