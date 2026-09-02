use anyhow::Result;
use rust_embed::RustEmbed;
use trane::{
    TRANE_CONFIG_DIR_PATH, Trane,
    course_library::{CourseLibrary, LocalCourseLibrary},
    data::{BasicAsset, ExerciseAsset, UserPreferences},
};
use ustr::Ustr;
use vfs::{EmbeddedFS, VfsPath};

#[derive(Debug, RustEmbed)]
#[folder = "tests/embedded_test_library"]
struct EmbeddedCourses;

// Verifies the given markdown asset.
fn assert_markdown_asset(
    root: &VfsPath,
    asset: &BasicAsset,
    expected_path: &str,
    expected_content: &str,
) -> Result<()> {
    assert_eq!(
        asset,
        &BasicAsset::MarkdownAsset {
            path: expected_path.into(),
        }
    );
    if let BasicAsset::MarkdownAsset { path } = asset {
        assert_eq!(root.join(path)?.read_to_string()?, expected_content);
    }
    Ok(())
}

/// Verifies loading a raw course and its path-backed assets from an embedded filesystem.
#[test]
fn loads_embedded_course_library() -> Result<()> {
    let root = VfsPath::new(EmbeddedFS::<EmbeddedCourses>::new());
    let library = LocalCourseLibrary::new(&root, UserPreferences::default())?;

    assert_eq!(
        library.get_course_ids(),
        vec![Ustr::from("embedded::raw_course")]
    );
    assert_eq!(
        library
            .get_lesson_ids("embedded::raw_course".into())
            .unwrap(),
        vec![Ustr::from("embedded::raw_course::lesson")]
    );
    assert_eq!(
        library
            .get_exercise_ids("embedded::raw_course::lesson".into())
            .unwrap(),
        vec![Ustr::from("embedded::raw_course::lesson::exercise")]
    );

    let course = library
        .get_course_manifest("embedded::raw_course".into())
        .unwrap();
    assert_markdown_asset(
        &root,
        course.course_instructions.as_ref().unwrap(),
        "raw_course/course.instructions.md",
        "Course instructions\n",
    )?;
    assert_markdown_asset(
        &root,
        course.course_material.as_ref().unwrap(),
        "raw_course/course.material.md",
        "Course material\n",
    )?;

    let lesson = library
        .get_lesson_manifest("embedded::raw_course::lesson".into())
        .unwrap();
    assert_markdown_asset(
        &root,
        lesson.lesson_instructions.as_ref().unwrap(),
        "raw_course/lesson/lesson.instructions.md",
        "Lesson instructions\n",
    )?;
    assert_markdown_asset(
        &root,
        lesson.lesson_material.as_ref().unwrap(),
        "raw_course/lesson/lesson.material.md",
        "Lesson material\n",
    )?;

    let exercise = library
        .get_exercise_manifest("embedded::raw_course::lesson::exercise".into())
        .unwrap();
    assert_eq!(
        exercise.exercise_asset,
        ExerciseAsset::FlashcardAsset {
            front_path: "raw_course/lesson/exercise/front.md".into(),
            back_path: Some("raw_course/lesson/exercise/back.md".into()),
        }
    );
    if let ExerciseAsset::FlashcardAsset {
        front_path,
        back_path: Some(back_path),
    } = &exercise.exercise_asset
    {
        assert_eq!(root.join(front_path)?.read_to_string()?, "Exercise front\n");
        assert_eq!(root.join(back_path)?.read_to_string()?, "Exercise back\n");
    }
    Ok(())
}

/// Verifies opening a full Trane instance with embedded courses and physical user data.
#[test]
fn opens_trane_with_embedded_course_library() -> Result<()> {
    let data_root = tempfile::tempdir()?;
    let course_library_root = VfsPath::new(EmbeddedFS::<EmbeddedCourses>::new());
    let trane = Trane::new_local_with_vfs(data_root.path(), &course_library_root)?;

    assert!(data_root.path().join(TRANE_CONFIG_DIR_PATH).is_dir());
    assert_eq!(
        trane.get_all_exercise_ids(None),
        vec![Ustr::from("embedded::raw_course::lesson::exercise")]
    );
    Ok(())
}
