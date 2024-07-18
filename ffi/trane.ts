/*
 Generated by typeshare 1.7.0
*/

export interface Instrument {
	name: string;
	id: string;
}

export interface KnowledgeBaseConfig {
}

export interface MusicPassage {
	start: string;
	end: string;
	sub_passages: Record<number, MusicPassage>;
}

export type MusicAsset = 
	| { type: "SoundSlice", content: string }
	| { type: "LocalFile", content: string };

export interface MusicPieceConfig {
	music_asset: MusicAsset;
	passages: MusicPassage;
}

export type TranscriptionAsset = 
	| { type: "Track", content: {
	short_id: string;
	track_name: string;
	artist_name?: string;
	album_name?: string;
	duration?: string;
	external_link?: TranscriptionLink;
}};

export interface TranscriptionPassages {
	asset: TranscriptionAsset;
	intervals?: Record<number, string[]>;
}

export interface TranscriptionPreferences {
	instruments?: Instrument[];
}

export interface TranscriptionConfig {
	transcription_dependencies?: string[];
	passage_directory?: string;
	inlined_passages?: TranscriptionPassages[];
	skip_singing_lessons?: boolean;
	skip_advanced_lessons?: boolean;
}

export type UnitFilter = 
	| { type: "CourseFilter", content: {
	course_ids: string[];
}}
	| { type: "LessonFilter", content: {
	lesson_ids: string[];
}}
	| { type: "MetadataFilter", content: {
	filter: KeyValueFilter;
}}
	| { type: "ReviewListFilter", content?: undefined }
	| { type: "Dependents", content: {
	unit_ids: string[];
}}
	| { type: "Dependencies", content: {
	unit_ids: string[];
	depth: number;
}};

export interface SavedFilter {
	id: string;
	description: string;
	filter: UnitFilter;
}

export type SessionPart = 
	| { type: "UnitFilter", content: {
	filter: UnitFilter;
	duration: number;
}}
	| { type: "SavedFilter", content: {
	filter_id: string;
	duration: number;
}}
	| { type: "NoFilter", content: {
	duration: number;
}};

export interface StudySession {
	id: string;
	description?: string;
	parts?: SessionPart[];
}

export interface StudySessionData {
	start_time: string;
	definition: StudySession;
}

export interface ExerciseTrial {
	score: number;
	timestamp: string;
}

export type BasicAsset = 
	| { type: "MarkdownAsset", content: {
	path: string;
}}
	| { type: "InlinedAsset", content: {
	content: string;
}}
	| { type: "InlinedUniqueAsset", content: {
	content: string;
}};

export type CourseGenerator = 
	| { type: "KnowledgeBase", content: KnowledgeBaseConfig }
	| { type: "MusicPiece", content: MusicPieceConfig }
	| { type: "Transcription", content: TranscriptionConfig };

export interface CourseManifest {
	id: string;
	name?: string;
	dependencies?: string[];
	superseded?: string[];
	description?: string;
	authors?: string[];
	metadata?: Record<string, string[]>;
	course_material?: BasicAsset;
	course_instructions?: BasicAsset;
	generator_config?: CourseGenerator;
}

export interface LessonManifest {
	id: string;
	dependencies?: string[];
	superseded?: string[];
	course_id: string;
	name?: string;
	description?: string;
	metadata?: Record<string, string[]>;
	lesson_material?: BasicAsset;
	lesson_instructions?: BasicAsset;
}

export enum ExerciseType {
	Declarative = "Declarative",
	Procedural = "Procedural",
}

export type ExerciseAsset = 
	| { type: "BasicAsset", content: BasicAsset }
	| { type: "FlashcardAsset", content: {
	front_path: string;
	back_path?: string;
}}
	| { type: "SoundSliceAsset", content: {
	link: string;
	description?: string;
	backup?: string;
}}
	| { type: "TranscriptionAsset", content: {
	content?: string;
	external_link?: TranscriptionLink;
}};

export interface ExerciseManifest {
	id: string;
	lesson_id: string;
	course_id: string;
	name?: string;
	description?: string;
	exercise_type?: ExerciseType;
	exercise_asset: ExerciseAsset;
}

export interface MasteryWindow {
	percentage: number;
	range: number[];
}

export type PassingScoreOptions = 
	| { type: "ConstantScore", content: number }
	| { type: "IncreasingScore", content: {
	starting_score: number;
	step_size: number;
	max_steps: number;
}};

export interface SchedulerOptions {
	batch_size: number;
	new_window_opts: MasteryWindow;
	target_window_opts: MasteryWindow;
	current_window_opts: MasteryWindow;
	easy_window_opts: MasteryWindow;
	mastered_window_opts: MasteryWindow;
	passing_score: PassingScoreOptions;
	superseding_score: number;
	num_trials: number;
}

export interface SchedulerPreferences {
	batch_size?: number;
}

export interface RepositoryMetadata {
	id: string;
	url: string;
}

export interface UserPreferences {
	transcription?: TranscriptionPreferences;
	scheduler?: SchedulerPreferences;
	ignored_paths?: string[];
}

export type TranscriptionLink = 
	| { type: "YouTube", content: string };

export enum FilterOp {
	All = "All",
	Any = "Any",
}

export enum FilterType {
	Include = "Include",
	Exclude = "Exclude",
}

export type KeyValueFilter = 
	| { type: "CourseFilter", content: {
	key: string;
	value: string;
	filter_type: FilterType;
}}
	| { type: "LessonFilter", content: {
	key: string;
	value: string;
	filter_type: FilterType;
}}
	| { type: "CombinedFilter", content: {
	op: FilterOp;
	filters: KeyValueFilter[];
}};

export type ExerciseFilter = 
	| { type: "UnitFilter", content: UnitFilter }
	| { type: "StudySession", content: StudySessionData };

export enum MasteryScore {
	One = "One",
	Two = "Two",
	Three = "Three",
	Four = "Four",
	Five = "Five",
}

export enum UnitType {
	Exercise = "Exercise",
	Lesson = "Lesson",
	Course = "Course",
}

