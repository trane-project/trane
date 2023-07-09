@Serializable
data class Instrument (
	val name: String,
	val id: String
)

@Serializable
data class ImprovisationConfig (
	val improvisation_dependencies: List<String>? = null,
	val rhythm_only: Boolean? = null,
	val passage_directory: String,
	val file_extensions: List<String>
)

@Serializable
data class ImprovisationPreferences (
	val instruments: List<Instrument>? = null,
	val rhythm_instruments: List<Instrument>? = null
)

@Serializable
object KnowledgeBaseConfig

@Serializable
data class MusicPassage (
	val start: String,
	val end: String,
	val sub_passages: HashMap<UInt, MusicPassage>
)

@Serializable
sealed class MusicAsset {
	@Serializable
	@SerialName("SoundSlice")
	data class SoundSlice(val content: String): MusicAsset()
	@Serializable
	@SerialName("LocalFile")
	data class LocalFile(val content: String): MusicAsset()
}

@Serializable
data class MusicPieceConfig (
	val music_asset: MusicAsset,
	val passages: MusicPassage
)

/// Generated type representing the anonymous struct variant `Track` of the `TranscriptionAsset` Rust enum
@Serializable
data class TranscriptionAssetTrackInner (
	val short_id: String,
	val track_name: String,
	val artist_name: String? = null,
	val album_name: String? = null,
	val duration: String? = null,
	val external_link: String? = null
)

@Serializable
sealed class TranscriptionAsset {
	@Serializable
	@SerialName("Track")
	data class Track(val content: TranscriptionAssetTrackInner): TranscriptionAsset()
}

@Serializable
data class TranscriptionPassages (
	val asset: TranscriptionAsset,
	val intervals: HashMap<UInt, List<String>>
)

@Serializable
data class TranscriptionPreferences (
	val instruments: List<Instrument>? = null
)

@Serializable
data class TranscriptionConfig (
	val transcription_dependencies: List<String>? = null,
	val passage_directory: String? = null,
	val inlined_passages: List<TranscriptionPassages>? = null,
	val skip_advanced_lessons: Boolean? = null
)

/// Generated type representing the anonymous struct variant `BasicFilter` of the `KeyValueFilter` Rust enum
@Serializable
data class KeyValueFilterBasicFilterInner (
	val key: String,
	val value: String,
	val filter_type: FilterType
)

/// Generated type representing the anonymous struct variant `CombinedFilter` of the `KeyValueFilter` Rust enum
@Serializable
data class KeyValueFilterCombinedFilterInner (
	val op: FilterOp,
	val filters: List<KeyValueFilter>
)

@Serializable
sealed class KeyValueFilter {
	@Serializable
	@SerialName("BasicFilter")
	data class BasicFilter(val content: KeyValueFilterBasicFilterInner): KeyValueFilter()
	@Serializable
	@SerialName("CombinedFilter")
	data class CombinedFilter(val content: KeyValueFilterCombinedFilterInner): KeyValueFilter()
}

@Serializable
enum class FilterOp(val string: String) {
	@SerialName("All")
	All("All"),
	@SerialName("Any")
	Any("Any"),
}

@Serializable
data class MetadataFilter (
	val course_filter: KeyValueFilter? = null,
	val lesson_filter: KeyValueFilter? = null,
	val op: FilterOp
)

/// Generated type representing the anonymous struct variant `CourseFilter` of the `UnitFilter` Rust enum
@Serializable
data class UnitFilterCourseFilterInner (
	val course_ids: List<String>
)

/// Generated type representing the anonymous struct variant `LessonFilter` of the `UnitFilter` Rust enum
@Serializable
data class UnitFilterLessonFilterInner (
	val lesson_ids: List<String>
)

/// Generated type representing the anonymous struct variant `MetadataFilter` of the `UnitFilter` Rust enum
@Serializable
data class UnitFilterMetadataFilterInner (
	val filter: MetadataFilter
)

/// Generated type representing the anonymous struct variant `Dependents` of the `UnitFilter` Rust enum
@Serializable
data class UnitFilterDependentsInner (
	val unit_ids: List<String>
)

/// Generated type representing the anonymous struct variant `Dependencies` of the `UnitFilter` Rust enum
@Serializable
data class UnitFilterDependenciesInner (
	val unit_ids: List<String>,
	val depth: UInt
)

@Serializable
sealed class UnitFilter {
	@Serializable
	@SerialName("CourseFilter")
	data class CourseFilter(val content: UnitFilterCourseFilterInner): UnitFilter()
	@Serializable
	@SerialName("LessonFilter")
	data class LessonFilter(val content: UnitFilterLessonFilterInner): UnitFilter()
	@Serializable
	@SerialName("MetadataFilter")
	data class MetadataFilter(val content: UnitFilterMetadataFilterInner): UnitFilter()
	@Serializable
	@SerialName("ReviewListFilter")
	object ReviewListFilter: UnitFilter()
	@Serializable
	@SerialName("Dependents")
	data class Dependents(val content: UnitFilterDependentsInner): UnitFilter()
	@Serializable
	@SerialName("Dependencies")
	data class Dependencies(val content: UnitFilterDependenciesInner): UnitFilter()
}

@Serializable
data class SavedFilter (
	val id: String,
	val description: String,
	val filter: UnitFilter
)

/// Generated type representing the anonymous struct variant `UnitFilter` of the `SessionPart` Rust enum
@Serializable
data class SessionPartUnitFilterInner (
	val filter: UnitFilter,
	val duration: UInt
)

/// Generated type representing the anonymous struct variant `SavedFilter` of the `SessionPart` Rust enum
@Serializable
data class SessionPartSavedFilterInner (
	val filter_id: String,
	val duration: UInt
)

/// Generated type representing the anonymous struct variant `NoFilter` of the `SessionPart` Rust enum
@Serializable
data class SessionPartNoFilterInner (
	val duration: UInt
)

@Serializable
sealed class SessionPart {
	@Serializable
	@SerialName("UnitFilter")
	data class UnitFilter(val content: SessionPartUnitFilterInner): SessionPart()
	@Serializable
	@SerialName("SavedFilter")
	data class SavedFilter(val content: SessionPartSavedFilterInner): SessionPart()
	@Serializable
	@SerialName("NoFilter")
	data class NoFilter(val content: SessionPartNoFilterInner): SessionPart()
}

@Serializable
data class StudySession (
	val id: String,
	val description: String? = null,
	val parts: List<SessionPart>? = null
)

@Serializable
data class StudySessionData (
	val start_time: String,
	val definition: StudySession
)

@Serializable
data class ExerciseTrial (
	val score: Float,
	val timestamp: String
)

/// Generated type representing the anonymous struct variant `MarkdownAsset` of the `BasicAsset` Rust enum
@Serializable
data class BasicAssetMarkdownAssetInner (
	val path: String
)

/// Generated type representing the anonymous struct variant `InlinedAsset` of the `BasicAsset` Rust enum
@Serializable
data class BasicAssetInlinedAssetInner (
	val content: String
)

/// Generated type representing the anonymous struct variant `InlinedUniqueAsset` of the `BasicAsset` Rust enum
@Serializable
data class BasicAssetInlinedUniqueAssetInner (
	val content: String
)

@Serializable
sealed class BasicAsset {
	@Serializable
	@SerialName("MarkdownAsset")
	data class MarkdownAsset(val content: BasicAssetMarkdownAssetInner): BasicAsset()
	@Serializable
	@SerialName("InlinedAsset")
	data class InlinedAsset(val content: BasicAssetInlinedAssetInner): BasicAsset()
	@Serializable
	@SerialName("InlinedUniqueAsset")
	data class InlinedUniqueAsset(val content: BasicAssetInlinedUniqueAssetInner): BasicAsset()
}

@Serializable
sealed class CourseGenerator {
	@Serializable
	@SerialName("Improvisation")
	data class Improvisation(val content: ImprovisationConfig): CourseGenerator()
	@Serializable
	@SerialName("KnowledgeBase")
	data class KnowledgeBase(val content: KnowledgeBaseConfig): CourseGenerator()
	@Serializable
	@SerialName("MusicPiece")
	data class MusicPiece(val content: MusicPieceConfig): CourseGenerator()
	@Serializable
	@SerialName("Transcription")
	data class Transcription(val content: TranscriptionConfig): CourseGenerator()
}

@Serializable
data class CourseManifest (
	val id: String,
	val name: String? = null,
	val dependencies: List<String>? = null,
	val description: String? = null,
	val authors: List<String>? = null,
	val metadata: HashMap<String, List<String>>? = null,
	val course_material: BasicAsset? = null,
	val course_instructions: BasicAsset? = null,
	val generator_config: CourseGenerator? = null
)

@Serializable
data class LessonManifest (
	val id: String,
	val dependencies: List<String>? = null,
	val course_id: String,
	val name: String? = null,
	val description: String? = null,
	val metadata: HashMap<String, List<String>>? = null,
	val lesson_material: BasicAsset? = null,
	val lesson_instructions: BasicAsset? = null
)

@Serializable
enum class ExerciseType(val string: String) {
	@SerialName("Declarative")
	Declarative("Declarative"),
	@SerialName("Procedural")
	Procedural("Procedural"),
}

/// Generated type representing the anonymous struct variant `SoundSliceAsset` of the `ExerciseAsset` Rust enum
@Serializable
data class ExerciseAssetSoundSliceAssetInner (
	val link: String,
	val description: String? = null,
	val backup: String? = null
)

/// Generated type representing the anonymous struct variant `FlashcardAsset` of the `ExerciseAsset` Rust enum
@Serializable
data class ExerciseAssetFlashcardAssetInner (
	val front_path: String,
	val back_path: String? = null
)

@Serializable
sealed class ExerciseAsset {
	@Serializable
	@SerialName("SoundSliceAsset")
	data class SoundSliceAsset(val content: ExerciseAssetSoundSliceAssetInner): ExerciseAsset()
	@Serializable
	@SerialName("FlashcardAsset")
	data class FlashcardAsset(val content: ExerciseAssetFlashcardAssetInner): ExerciseAsset()
	@Serializable
	@SerialName("BasicAsset")
	data class BasicAsset(val content: BasicAsset): ExerciseAsset()
}

@Serializable
data class ExerciseManifest (
	val id: String,
	val lesson_id: String,
	val course_id: String,
	val name: String? = null,
	val description: String? = null,
	val exercise_type: ExerciseType? = null,
	val exercise_asset: ExerciseAsset
)

@Serializable
data class MasteryWindow (
	val percentage: Float,
	val range: List<Float>
)

/// Generated type representing the anonymous struct variant `IncreasingScore` of the `PassingScoreOptions` Rust enum
@Serializable
data class PassingScoreOptionsIncreasingScoreInner (
	val starting_score: Float,
	val step_size: Float,
	val max_steps: UInt
)

@Serializable
sealed class PassingScoreOptions {
	@Serializable
	@SerialName("ConstantScore")
	data class ConstantScore(val content: Float): PassingScoreOptions()
	@Serializable
	@SerialName("IncreasingScore")
	data class IncreasingScore(val content: PassingScoreOptionsIncreasingScoreInner): PassingScoreOptions()
}

@Serializable
data class SchedulerOptions (
	val batch_size: UInt,
	val new_window_opts: MasteryWindow,
	val target_window_opts: MasteryWindow,
	val current_window_opts: MasteryWindow,
	val easy_window_opts: MasteryWindow,
	val mastered_window_opts: MasteryWindow,
	val passing_score: PassingScoreOptions,
	val num_trials: UInt
)

@Serializable
data class SchedulerPreferences (
	val batch_size: UInt? = null
)

@Serializable
data class RepositoryMetadata (
	val id: String,
	val url: String
)

@Serializable
data class UserPreferences (
	val improvisation: ImprovisationPreferences? = null,
	val transcription: TranscriptionPreferences? = null,
	val scheduler: SchedulerPreferences? = null,
	val ignored_paths: List<String>? = null
)

@Serializable
enum class FilterType(val string: String) {
	@SerialName("Include")
	Include("Include"),
	@SerialName("Exclude")
	Exclude("Exclude"),
}

@Serializable
sealed class ExerciseFilter {
	@Serializable
	@SerialName("UnitFilter")
	data class UnitFilter(val content: UnitFilter): ExerciseFilter()
	@Serializable
	@SerialName("StudySession")
	data class StudySession(val content: StudySessionData): ExerciseFilter()
}

@Serializable
enum class UnitType(val string: String) {
	@SerialName("Exercise")
	Exercise("Exercise"),
	@SerialName("Lesson")
	Lesson("Lesson"),
	@SerialName("Course")
	Course("Course"),
}

