// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { SessionPart } from "./SessionPart";

/**
 * A study session is a list of parts, each of which define the exercises to study and for how
 * long. For example, a student learning to play piano and guitar could define a session that
 * spends 30 minutes on exercises for piano, and 30 minutes on exercises for guitar.
 */
export type StudySession = { 
/**
 * A unique identifier for the study session.
 */
id: string, 
/**
 * A human-readable description for the study session.
 */
description: string, 
/**
 * The parts of the study session.
 */
parts: Array<SessionPart>, };