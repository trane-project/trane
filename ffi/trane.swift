/*
 Generated by typeshare 1.6.0
 */

import Foundation

public struct Instrument: Codable {
	public let name: String
	public let id: String

	public init(name: String, id: String) {
		self.name = name
		self.id = id
	}
}

public struct ImprovisationConfig: Codable {
	public let improvisation_dependencies: [String]?
	public let rhythm_only: Bool?
	public let passage_directory: String
	public let file_extensions: [String]

	public init(improvisation_dependencies: [String]?, rhythm_only: Bool?, passage_directory: String, file_extensions: [String]) {
		self.improvisation_dependencies = improvisation_dependencies
		self.rhythm_only = rhythm_only
		self.passage_directory = passage_directory
		self.file_extensions = file_extensions
	}
}

public struct ImprovisationPreferences: Codable {
	public let instruments: [Instrument]?
	public let rhythm_instruments: [Instrument]?

	public init(instruments: [Instrument]?, rhythm_instruments: [Instrument]?) {
		self.instruments = instruments
		self.rhythm_instruments = rhythm_instruments
	}
}

public struct KnowledgeBaseConfig: Codable {
	public init() {}
}

public struct MusicPassage: Codable {
	public let start: String
	public let end: String
	public let sub_passages: [UInt32: MusicPassage]

	public init(start: String, end: String, sub_passages: [UInt32: MusicPassage]) {
		self.start = start
		self.end = end
		self.sub_passages = sub_passages
	}
}

public enum MusicAsset: Codable {
	case soundSlice(String)
	case localFile(String)

	enum CodingKeys: String, CodingKey, Codable {
		case soundSlice = "SoundSlice",
			localFile = "LocalFile"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .soundSlice:
				if let content = try? container.decode(String.self, forKey: .content) {
					self = .soundSlice(content)
					return
				}
			case .localFile:
				if let content = try? container.decode(String.self, forKey: .content) {
					self = .localFile(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(MusicAsset.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for MusicAsset"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .soundSlice(let content):
			try container.encode(CodingKeys.soundSlice, forKey: .type)
			try container.encode(content, forKey: .content)
		case .localFile(let content):
			try container.encode(CodingKeys.localFile, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public struct MusicPieceConfig: Codable {
	public let music_asset: MusicAsset
	public let passages: MusicPassage

	public init(music_asset: MusicAsset, passages: MusicPassage) {
		self.music_asset = music_asset
		self.passages = passages
	}
}


/// Generated type representing the anonymous struct variant `Track` of the `TranscriptionAsset` Rust enum
public struct TranscriptionAssetTrackInner: Codable {
	public let short_id: String
	public let track_name: String
	public let artist_name: String?
	public let album_name: String?
	public let duration: String?
	public let external_link: TranscriptionLink?

	public init(short_id: String, track_name: String, artist_name: String?, album_name: String?, duration: String?, external_link: TranscriptionLink?) {
		self.short_id = short_id
		self.track_name = track_name
		self.artist_name = artist_name
		self.album_name = album_name
		self.duration = duration
		self.external_link = external_link
	}
}
public enum TranscriptionAsset: Codable {
	case track(TranscriptionAssetTrackInner)

	enum CodingKeys: String, CodingKey, Codable {
		case track = "Track"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .track:
				if let content = try? container.decode(TranscriptionAssetTrackInner.self, forKey: .content) {
					self = .track(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(TranscriptionAsset.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for TranscriptionAsset"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .track(let content):
			try container.encode(CodingKeys.track, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public struct TranscriptionPassages: Codable {
	public let asset: TranscriptionAsset
	public let intervals: [UInt32: [String]]

	public init(asset: TranscriptionAsset, intervals: [UInt32: [String]]) {
		self.asset = asset
		self.intervals = intervals
	}
}

public struct TranscriptionPreferences: Codable {
	public let instruments: [Instrument]?

	public init(instruments: [Instrument]?) {
		self.instruments = instruments
	}
}

public struct TranscriptionConfig: Codable {
	public let transcription_dependencies: [String]?
	public let passage_directory: String?
	public let inlined_passages: [TranscriptionPassages]?
	public let skip_advanced_lessons: Bool?

	public init(transcription_dependencies: [String]?, passage_directory: String?, inlined_passages: [TranscriptionPassages]?, skip_advanced_lessons: Bool?) {
		self.transcription_dependencies = transcription_dependencies
		self.passage_directory = passage_directory
		self.inlined_passages = inlined_passages
		self.skip_advanced_lessons = skip_advanced_lessons
	}
}


/// Generated type representing the anonymous struct variant `BasicFilter` of the `KeyValueFilter` Rust enum
public struct KeyValueFilterBasicFilterInner: Codable {
	public let key: String
	public let value: String
	public let filter_type: FilterType

	public init(key: String, value: String, filter_type: FilterType) {
		self.key = key
		self.value = value
		self.filter_type = filter_type
	}
}

/// Generated type representing the anonymous struct variant `CombinedFilter` of the `KeyValueFilter` Rust enum
public struct KeyValueFilterCombinedFilterInner: Codable {
	public let op: FilterOp
	public let filters: [KeyValueFilter]

	public init(op: FilterOp, filters: [KeyValueFilter]) {
		self.op = op
		self.filters = filters
	}
}
public indirect enum KeyValueFilter: Codable {
	case basicFilter(KeyValueFilterBasicFilterInner)
	case combinedFilter(KeyValueFilterCombinedFilterInner)

	enum CodingKeys: String, CodingKey, Codable {
		case basicFilter = "BasicFilter",
			combinedFilter = "CombinedFilter"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .basicFilter:
				if let content = try? container.decode(KeyValueFilterBasicFilterInner.self, forKey: .content) {
					self = .basicFilter(content)
					return
				}
			case .combinedFilter:
				if let content = try? container.decode(KeyValueFilterCombinedFilterInner.self, forKey: .content) {
					self = .combinedFilter(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(KeyValueFilter.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for KeyValueFilter"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .basicFilter(let content):
			try container.encode(CodingKeys.basicFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		case .combinedFilter(let content):
			try container.encode(CodingKeys.combinedFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public enum FilterOp: String, Codable {
	case all = "All"
	case any = "Any"
}

public struct MetadataFilter: Codable {
	public let course_filter: KeyValueFilter?
	public let lesson_filter: KeyValueFilter?
	public let op: FilterOp

	public init(course_filter: KeyValueFilter?, lesson_filter: KeyValueFilter?, op: FilterOp) {
		self.course_filter = course_filter
		self.lesson_filter = lesson_filter
		self.op = op
	}
}


/// Generated type representing the anonymous struct variant `CourseFilter` of the `UnitFilter` Rust enum
public struct UnitFilterCourseFilterInner: Codable {
	public let course_ids: [String]

	public init(course_ids: [String]) {
		self.course_ids = course_ids
	}
}

/// Generated type representing the anonymous struct variant `LessonFilter` of the `UnitFilter` Rust enum
public struct UnitFilterLessonFilterInner: Codable {
	public let lesson_ids: [String]

	public init(lesson_ids: [String]) {
		self.lesson_ids = lesson_ids
	}
}

/// Generated type representing the anonymous struct variant `MetadataFilter` of the `UnitFilter` Rust enum
public struct UnitFilterMetadataFilterInner: Codable {
	public let filter: MetadataFilter

	public init(filter: MetadataFilter) {
		self.filter = filter
	}
}

/// Generated type representing the anonymous struct variant `Dependents` of the `UnitFilter` Rust enum
public struct UnitFilterDependentsInner: Codable {
	public let unit_ids: [String]

	public init(unit_ids: [String]) {
		self.unit_ids = unit_ids
	}
}

/// Generated type representing the anonymous struct variant `Dependencies` of the `UnitFilter` Rust enum
public struct UnitFilterDependenciesInner: Codable {
	public let unit_ids: [String]
	public let depth: UInt32

	public init(unit_ids: [String], depth: UInt32) {
		self.unit_ids = unit_ids
		self.depth = depth
	}
}
public enum UnitFilter: Codable {
	case courseFilter(UnitFilterCourseFilterInner)
	case lessonFilter(UnitFilterLessonFilterInner)
	case metadataFilter(UnitFilterMetadataFilterInner)
	case reviewListFilter
	case dependents(UnitFilterDependentsInner)
	case dependencies(UnitFilterDependenciesInner)

	enum CodingKeys: String, CodingKey, Codable {
		case courseFilter = "CourseFilter",
			lessonFilter = "LessonFilter",
			metadataFilter = "MetadataFilter",
			reviewListFilter = "ReviewListFilter",
			dependents = "Dependents",
			dependencies = "Dependencies"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .courseFilter:
				if let content = try? container.decode(UnitFilterCourseFilterInner.self, forKey: .content) {
					self = .courseFilter(content)
					return
				}
			case .lessonFilter:
				if let content = try? container.decode(UnitFilterLessonFilterInner.self, forKey: .content) {
					self = .lessonFilter(content)
					return
				}
			case .metadataFilter:
				if let content = try? container.decode(UnitFilterMetadataFilterInner.self, forKey: .content) {
					self = .metadataFilter(content)
					return
				}
			case .reviewListFilter:
				self = .reviewListFilter
				return
			case .dependents:
				if let content = try? container.decode(UnitFilterDependentsInner.self, forKey: .content) {
					self = .dependents(content)
					return
				}
			case .dependencies:
				if let content = try? container.decode(UnitFilterDependenciesInner.self, forKey: .content) {
					self = .dependencies(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(UnitFilter.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for UnitFilter"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .courseFilter(let content):
			try container.encode(CodingKeys.courseFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		case .lessonFilter(let content):
			try container.encode(CodingKeys.lessonFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		case .metadataFilter(let content):
			try container.encode(CodingKeys.metadataFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		case .reviewListFilter:
			try container.encode(CodingKeys.reviewListFilter, forKey: .type)
		case .dependents(let content):
			try container.encode(CodingKeys.dependents, forKey: .type)
			try container.encode(content, forKey: .content)
		case .dependencies(let content):
			try container.encode(CodingKeys.dependencies, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public struct SavedFilter: Codable {
	public let id: String
	public let description: String
	public let filter: UnitFilter

	public init(id: String, description: String, filter: UnitFilter) {
		self.id = id
		self.description = description
		self.filter = filter
	}
}


/// Generated type representing the anonymous struct variant `UnitFilter` of the `SessionPart` Rust enum
public struct SessionPartUnitFilterInner: Codable {
	public let filter: UnitFilter
	public let duration: UInt32

	public init(filter: UnitFilter, duration: UInt32) {
		self.filter = filter
		self.duration = duration
	}
}

/// Generated type representing the anonymous struct variant `SavedFilter` of the `SessionPart` Rust enum
public struct SessionPartSavedFilterInner: Codable {
	public let filter_id: String
	public let duration: UInt32

	public init(filter_id: String, duration: UInt32) {
		self.filter_id = filter_id
		self.duration = duration
	}
}

/// Generated type representing the anonymous struct variant `NoFilter` of the `SessionPart` Rust enum
public struct SessionPartNoFilterInner: Codable {
	public let duration: UInt32

	public init(duration: UInt32) {
		self.duration = duration
	}
}
public enum SessionPart: Codable {
	case unitFilter(SessionPartUnitFilterInner)
	case savedFilter(SessionPartSavedFilterInner)
	case noFilter(SessionPartNoFilterInner)

	enum CodingKeys: String, CodingKey, Codable {
		case unitFilter = "UnitFilter",
			savedFilter = "SavedFilter",
			noFilter = "NoFilter"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .unitFilter:
				if let content = try? container.decode(SessionPartUnitFilterInner.self, forKey: .content) {
					self = .unitFilter(content)
					return
				}
			case .savedFilter:
				if let content = try? container.decode(SessionPartSavedFilterInner.self, forKey: .content) {
					self = .savedFilter(content)
					return
				}
			case .noFilter:
				if let content = try? container.decode(SessionPartNoFilterInner.self, forKey: .content) {
					self = .noFilter(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(SessionPart.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for SessionPart"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .unitFilter(let content):
			try container.encode(CodingKeys.unitFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		case .savedFilter(let content):
			try container.encode(CodingKeys.savedFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		case .noFilter(let content):
			try container.encode(CodingKeys.noFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public struct StudySession: Codable {
	public let id: String
	public let description: String?
	public let parts: [SessionPart]?

	public init(id: String, description: String?, parts: [SessionPart]?) {
		self.id = id
		self.description = description
		self.parts = parts
	}
}

public struct StudySessionData: Codable {
	public let start_time: String
	public let definition: StudySession

	public init(start_time: String, definition: StudySession) {
		self.start_time = start_time
		self.definition = definition
	}
}

public struct ExerciseTrial: Codable {
	public let score: Float
	public let timestamp: String

	public init(score: Float, timestamp: String) {
		self.score = score
		self.timestamp = timestamp
	}
}


/// Generated type representing the anonymous struct variant `MarkdownAsset` of the `BasicAsset` Rust enum
public struct BasicAssetMarkdownAssetInner: Codable {
	public let path: String

	public init(path: String) {
		self.path = path
	}
}

/// Generated type representing the anonymous struct variant `InlinedAsset` of the `BasicAsset` Rust enum
public struct BasicAssetInlinedAssetInner: Codable {
	public let content: String

	public init(content: String) {
		self.content = content
	}
}

/// Generated type representing the anonymous struct variant `InlinedUniqueAsset` of the `BasicAsset` Rust enum
public struct BasicAssetInlinedUniqueAssetInner: Codable {
	public let content: String

	public init(content: String) {
		self.content = content
	}
}
public enum BasicAsset: Codable {
	case markdownAsset(BasicAssetMarkdownAssetInner)
	case inlinedAsset(BasicAssetInlinedAssetInner)
	case inlinedUniqueAsset(BasicAssetInlinedUniqueAssetInner)

	enum CodingKeys: String, CodingKey, Codable {
		case markdownAsset = "MarkdownAsset",
			inlinedAsset = "InlinedAsset",
			inlinedUniqueAsset = "InlinedUniqueAsset"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .markdownAsset:
				if let content = try? container.decode(BasicAssetMarkdownAssetInner.self, forKey: .content) {
					self = .markdownAsset(content)
					return
				}
			case .inlinedAsset:
				if let content = try? container.decode(BasicAssetInlinedAssetInner.self, forKey: .content) {
					self = .inlinedAsset(content)
					return
				}
			case .inlinedUniqueAsset:
				if let content = try? container.decode(BasicAssetInlinedUniqueAssetInner.self, forKey: .content) {
					self = .inlinedUniqueAsset(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(BasicAsset.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for BasicAsset"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .markdownAsset(let content):
			try container.encode(CodingKeys.markdownAsset, forKey: .type)
			try container.encode(content, forKey: .content)
		case .inlinedAsset(let content):
			try container.encode(CodingKeys.inlinedAsset, forKey: .type)
			try container.encode(content, forKey: .content)
		case .inlinedUniqueAsset(let content):
			try container.encode(CodingKeys.inlinedUniqueAsset, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public enum CourseGenerator: Codable {
	case improvisation(ImprovisationConfig)
	case knowledgeBase(KnowledgeBaseConfig)
	case musicPiece(MusicPieceConfig)
	case transcription(TranscriptionConfig)

	enum CodingKeys: String, CodingKey, Codable {
		case improvisation = "Improvisation",
			knowledgeBase = "KnowledgeBase",
			musicPiece = "MusicPiece",
			transcription = "Transcription"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .improvisation:
				if let content = try? container.decode(ImprovisationConfig.self, forKey: .content) {
					self = .improvisation(content)
					return
				}
			case .knowledgeBase:
				if let content = try? container.decode(KnowledgeBaseConfig.self, forKey: .content) {
					self = .knowledgeBase(content)
					return
				}
			case .musicPiece:
				if let content = try? container.decode(MusicPieceConfig.self, forKey: .content) {
					self = .musicPiece(content)
					return
				}
			case .transcription:
				if let content = try? container.decode(TranscriptionConfig.self, forKey: .content) {
					self = .transcription(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(CourseGenerator.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for CourseGenerator"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .improvisation(let content):
			try container.encode(CodingKeys.improvisation, forKey: .type)
			try container.encode(content, forKey: .content)
		case .knowledgeBase(let content):
			try container.encode(CodingKeys.knowledgeBase, forKey: .type)
			try container.encode(content, forKey: .content)
		case .musicPiece(let content):
			try container.encode(CodingKeys.musicPiece, forKey: .type)
			try container.encode(content, forKey: .content)
		case .transcription(let content):
			try container.encode(CodingKeys.transcription, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public struct CourseManifest: Codable {
	public let id: String
	public let name: String?
	public let dependencies: [String]?
	public let description: String?
	public let authors: [String]?
	public let metadata: [String: [String]]?
	public let course_material: BasicAsset?
	public let course_instructions: BasicAsset?
	public let generator_config: CourseGenerator?

	public init(id: String, name: String?, dependencies: [String]?, description: String?, authors: [String]?, metadata: [String: [String]]?, course_material: BasicAsset?, course_instructions: BasicAsset?, generator_config: CourseGenerator?) {
		self.id = id
		self.name = name
		self.dependencies = dependencies
		self.description = description
		self.authors = authors
		self.metadata = metadata
		self.course_material = course_material
		self.course_instructions = course_instructions
		self.generator_config = generator_config
	}
}

public struct LessonManifest: Codable {
	public let id: String
	public let dependencies: [String]?
	public let course_id: String
	public let name: String?
	public let description: String?
	public let metadata: [String: [String]]?
	public let lesson_material: BasicAsset?
	public let lesson_instructions: BasicAsset?

	public init(id: String, dependencies: [String]?, course_id: String, name: String?, description: String?, metadata: [String: [String]]?, lesson_material: BasicAsset?, lesson_instructions: BasicAsset?) {
		self.id = id
		self.dependencies = dependencies
		self.course_id = course_id
		self.name = name
		self.description = description
		self.metadata = metadata
		self.lesson_material = lesson_material
		self.lesson_instructions = lesson_instructions
	}
}

public enum ExerciseType: String, Codable {
	case declarative = "Declarative"
	case procedural = "Procedural"
}


/// Generated type representing the anonymous struct variant `SoundSliceAsset` of the `ExerciseAsset` Rust enum
public struct ExerciseAssetSoundSliceAssetInner: Codable {
	public let link: String
	public let description: String?
	public let backup: String?

	public init(link: String, description: String?, backup: String?) {
		self.link = link
		self.description = description
		self.backup = backup
	}
}

/// Generated type representing the anonymous struct variant `FlashcardAsset` of the `ExerciseAsset` Rust enum
public struct ExerciseAssetFlashcardAssetInner: Codable {
	public let front_path: String
	public let back_path: String?

	public init(front_path: String, back_path: String?) {
		self.front_path = front_path
		self.back_path = back_path
	}
}
public enum ExerciseAsset: Codable {
	case soundSliceAsset(ExerciseAssetSoundSliceAssetInner)
	case flashcardAsset(ExerciseAssetFlashcardAssetInner)
	case basicAsset(BasicAsset)

	enum CodingKeys: String, CodingKey, Codable {
		case soundSliceAsset = "SoundSliceAsset",
			flashcardAsset = "FlashcardAsset",
			basicAsset = "BasicAsset"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .soundSliceAsset:
				if let content = try? container.decode(ExerciseAssetSoundSliceAssetInner.self, forKey: .content) {
					self = .soundSliceAsset(content)
					return
				}
			case .flashcardAsset:
				if let content = try? container.decode(ExerciseAssetFlashcardAssetInner.self, forKey: .content) {
					self = .flashcardAsset(content)
					return
				}
			case .basicAsset:
				if let content = try? container.decode(BasicAsset.self, forKey: .content) {
					self = .basicAsset(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(ExerciseAsset.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for ExerciseAsset"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .soundSliceAsset(let content):
			try container.encode(CodingKeys.soundSliceAsset, forKey: .type)
			try container.encode(content, forKey: .content)
		case .flashcardAsset(let content):
			try container.encode(CodingKeys.flashcardAsset, forKey: .type)
			try container.encode(content, forKey: .content)
		case .basicAsset(let content):
			try container.encode(CodingKeys.basicAsset, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public struct ExerciseManifest: Codable {
	public let id: String
	public let lesson_id: String
	public let course_id: String
	public let name: String?
	public let description: String?
	public let exercise_type: ExerciseType?
	public let exercise_asset: ExerciseAsset

	public init(id: String, lesson_id: String, course_id: String, name: String?, description: String?, exercise_type: ExerciseType?, exercise_asset: ExerciseAsset) {
		self.id = id
		self.lesson_id = lesson_id
		self.course_id = course_id
		self.name = name
		self.description = description
		self.exercise_type = exercise_type
		self.exercise_asset = exercise_asset
	}
}

public struct MasteryWindow: Codable {
	public let percentage: Float
	public let range: [Float]

	public init(percentage: Float, range: [Float]) {
		self.percentage = percentage
		self.range = range
	}
}


/// Generated type representing the anonymous struct variant `IncreasingScore` of the `PassingScoreOptions` Rust enum
public struct PassingScoreOptionsIncreasingScoreInner: Codable {
	public let starting_score: Float
	public let step_size: Float
	public let max_steps: UInt32

	public init(starting_score: Float, step_size: Float, max_steps: UInt32) {
		self.starting_score = starting_score
		self.step_size = step_size
		self.max_steps = max_steps
	}
}
public enum PassingScoreOptions: Codable {
	case constantScore(Float)
	case increasingScore(PassingScoreOptionsIncreasingScoreInner)

	enum CodingKeys: String, CodingKey, Codable {
		case constantScore = "ConstantScore",
			increasingScore = "IncreasingScore"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .constantScore:
				if let content = try? container.decode(Float.self, forKey: .content) {
					self = .constantScore(content)
					return
				}
			case .increasingScore:
				if let content = try? container.decode(PassingScoreOptionsIncreasingScoreInner.self, forKey: .content) {
					self = .increasingScore(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(PassingScoreOptions.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for PassingScoreOptions"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .constantScore(let content):
			try container.encode(CodingKeys.constantScore, forKey: .type)
			try container.encode(content, forKey: .content)
		case .increasingScore(let content):
			try container.encode(CodingKeys.increasingScore, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public struct SchedulerOptions: Codable {
	public let batch_size: UInt32
	public let new_window_opts: MasteryWindow
	public let target_window_opts: MasteryWindow
	public let current_window_opts: MasteryWindow
	public let easy_window_opts: MasteryWindow
	public let mastered_window_opts: MasteryWindow
	public let passing_score: PassingScoreOptions
	public let num_trials: UInt32

	public init(batch_size: UInt32, new_window_opts: MasteryWindow, target_window_opts: MasteryWindow, current_window_opts: MasteryWindow, easy_window_opts: MasteryWindow, mastered_window_opts: MasteryWindow, passing_score: PassingScoreOptions, num_trials: UInt32) {
		self.batch_size = batch_size
		self.new_window_opts = new_window_opts
		self.target_window_opts = target_window_opts
		self.current_window_opts = current_window_opts
		self.easy_window_opts = easy_window_opts
		self.mastered_window_opts = mastered_window_opts
		self.passing_score = passing_score
		self.num_trials = num_trials
	}
}

public struct SchedulerPreferences: Codable {
	public let batch_size: UInt32?

	public init(batch_size: UInt32?) {
		self.batch_size = batch_size
	}
}

public struct RepositoryMetadata: Codable {
	public let id: String
	public let url: String

	public init(id: String, url: String) {
		self.id = id
		self.url = url
	}
}

public struct UserPreferences: Codable {
	public let improvisation: ImprovisationPreferences?
	public let transcription: TranscriptionPreferences?
	public let scheduler: SchedulerPreferences?
	public let ignored_paths: [String]?

	public init(improvisation: ImprovisationPreferences?, transcription: TranscriptionPreferences?, scheduler: SchedulerPreferences?, ignored_paths: [String]?) {
		self.improvisation = improvisation
		self.transcription = transcription
		self.scheduler = scheduler
		self.ignored_paths = ignored_paths
	}
}

public enum TranscriptionLink: Codable {
	case youTube(String)

	enum CodingKeys: String, CodingKey, Codable {
		case youTube = "YouTube"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .youTube:
				if let content = try? container.decode(String.self, forKey: .content) {
					self = .youTube(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(TranscriptionLink.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for TranscriptionLink"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .youTube(let content):
			try container.encode(CodingKeys.youTube, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public enum FilterType: String, Codable {
	case include = "Include"
	case exclude = "Exclude"
}

public enum ExerciseFilter: Codable {
	case unitFilter(UnitFilter)
	case studySession(StudySessionData)

	enum CodingKeys: String, CodingKey, Codable {
		case unitFilter = "UnitFilter",
			studySession = "StudySession"
	}

	private enum ContainerCodingKeys: String, CodingKey {
		case type, content
	}

	public init(from decoder: Decoder) throws {
		let container = try decoder.container(keyedBy: ContainerCodingKeys.self)
		if let type = try? container.decode(CodingKeys.self, forKey: .type) {
			switch type {
			case .unitFilter:
				if let content = try? container.decode(UnitFilter.self, forKey: .content) {
					self = .unitFilter(content)
					return
				}
			case .studySession:
				if let content = try? container.decode(StudySessionData.self, forKey: .content) {
					self = .studySession(content)
					return
				}
			}
		}
		throw DecodingError.typeMismatch(ExerciseFilter.self, DecodingError.Context(codingPath: decoder.codingPath, debugDescription: "Wrong type for ExerciseFilter"))
	}

	public func encode(to encoder: Encoder) throws {
		var container = encoder.container(keyedBy: ContainerCodingKeys.self)
		switch self {
		case .unitFilter(let content):
			try container.encode(CodingKeys.unitFilter, forKey: .type)
			try container.encode(content, forKey: .content)
		case .studySession(let content):
			try container.encode(CodingKeys.studySession, forKey: .type)
			try container.encode(content, forKey: .content)
		}
	}
}

public enum MasteryScore: String, Codable {
	case one = "One"
	case two = "Two"
	case three = "Three"
	case four = "Four"
	case five = "Five"
}

public enum UnitType: String, Codable {
	case exercise = "Exercise"
	case lesson = "Lesson"
	case course = "Course"
}
