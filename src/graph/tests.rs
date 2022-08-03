use anyhow::Result;
use indoc::indoc;
use ustr::Ustr;

use crate::data::UnitType;

use super::{InMemoryUnitGraph, UnitGraph};

#[test]
fn get_unit_type() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let id = Ustr::from("id1");
    graph.add_dependencies(&id, UnitType::Course, &vec![])?;
    assert_eq!(graph.get_unit_type(&id), Some(UnitType::Course));
    Ok(())
}

#[test]
fn get_course_lessons_and_exercises() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let course_id = Ustr::from("course1");
    let lesson1_id = Ustr::from("course1::lesson1");
    let lesson2_id = Ustr::from("course1::lesson2");
    let lesson1_exercise1_id = Ustr::from("course1::lesson1::exercise1");
    let lesson1_exercise2_id = Ustr::from("course1::lesson1::exercise2");
    let lesson2_exercise1_id = Ustr::from("course1::lesson2::exercise1");
    let lesson2_exercise2_id = Ustr::from("course1::lesson2::exercise2");
    graph.add_dependencies(&course_id, UnitType::Course, &vec![])?;

    graph.add_lesson(&lesson1_id, &course_id)?;
    graph.add_exercise(&lesson1_exercise1_id, &lesson1_id)?;
    graph.add_exercise(&lesson1_exercise2_id, &lesson1_id)?;
    graph.add_lesson(&lesson2_id, &course_id)?;
    graph.add_exercise(&lesson2_exercise1_id, &lesson2_id)?;
    graph.add_exercise(&lesson2_exercise2_id, &lesson2_id)?;

    let course_lessons = graph.get_course_lessons(&course_id).unwrap();
    assert_eq!(course_lessons.len(), 2);
    assert!(course_lessons.contains(&lesson1_id));
    assert!(course_lessons.contains(&lesson2_id));

    let lesson1_exercises = graph.get_lesson_exercises(&lesson1_id).unwrap();
    assert_eq!(lesson1_exercises.len(), 2);
    assert!(lesson1_exercises.contains(&lesson1_exercise1_id));
    assert!(lesson1_exercises.contains(&lesson1_exercise2_id));

    let lesson2_exercises = graph.get_lesson_exercises(&lesson2_id).unwrap();
    assert_eq!(lesson2_exercises.len(), 2);
    assert!(lesson2_exercises.contains(&lesson2_exercise1_id));
    assert!(lesson2_exercises.contains(&lesson2_exercise2_id));

    Ok(())
}

#[test]
fn dependencies() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let course1_id = Ustr::from("course1");
    let course2_id = Ustr::from("course2");
    let course3_id = Ustr::from("course3");
    let course4_id = Ustr::from("course4");
    let course5_id = Ustr::from("course5");
    graph.add_dependencies(&course1_id, UnitType::Course, &vec![])?;
    graph.add_dependencies(&course2_id, UnitType::Course, &vec![course1_id.clone()])?;
    graph.add_dependencies(&course3_id, UnitType::Course, &vec![course1_id.clone()])?;
    graph.add_dependencies(&course4_id, UnitType::Course, &vec![course2_id.clone()])?;
    graph.add_dependencies(&course5_id, UnitType::Course, &vec![course3_id.clone()])?;

    {
        let dependents = graph.get_dependents(&course1_id).unwrap();
        assert_eq!(dependents.len(), 2);
        assert!(dependents.contains(&course2_id));
        assert!(dependents.contains(&course3_id));

        assert!(graph.get_dependencies(&course1_id).unwrap().is_empty());
    }

    {
        let dependents = graph.get_dependents(&course2_id).unwrap();
        assert_eq!(dependents.len(), 1);
        assert!(dependents.contains(&course4_id));

        let dependencies = graph.get_dependencies(&course2_id).unwrap();
        assert_eq!(dependencies.len(), 1);
        assert!(dependencies.contains(&course1_id));
    }

    {
        let dependents = graph.get_dependents(&course2_id).unwrap();
        assert_eq!(dependents.len(), 1);
        assert!(dependents.contains(&course4_id));

        let dependencies = graph.get_dependencies(&course2_id).unwrap();
        assert_eq!(dependencies.len(), 1);
        assert!(dependencies.contains(&course1_id));
    }

    {
        let dependents = graph.get_dependents(&course3_id).unwrap();
        assert_eq!(dependents.len(), 1);
        assert!(dependents.contains(&course5_id));

        let dependencies = graph.get_dependencies(&course3_id).unwrap();
        assert_eq!(dependencies.len(), 1);
        assert!(dependencies.contains(&course1_id));
    }

    {
        assert!(graph.get_dependents(&course4_id).is_none());

        let dependencies = graph.get_dependencies(&course4_id).unwrap();
        assert_eq!(dependencies.len(), 1);
        assert!(dependencies.contains(&course2_id));
    }

    {
        assert!(graph.get_dependents(&course5_id).is_none());

        let dependencies = graph.get_dependencies(&course5_id).unwrap();
        assert_eq!(dependencies.len(), 1);
        assert!(dependencies.contains(&course3_id));
    }

    let sinks = graph.get_dependency_sinks();
    assert_eq!(sinks.len(), 1);
    assert!(sinks.contains(&course1_id));

    graph.check_cycles()?;
    Ok(())
}

#[test]
fn dependencies_cycle() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let course1_id = Ustr::from("course1");
    let course2_id = Ustr::from("course2");
    let course3_id = Ustr::from("course3");
    let course4_id = Ustr::from("course4");
    let course5_id = Ustr::from("course5");
    graph.add_dependencies(&course1_id, UnitType::Course, &vec![])?;
    graph.add_dependencies(&course2_id, UnitType::Course, &vec![course1_id.clone()])?;
    graph.add_dependencies(&course3_id, UnitType::Course, &vec![course1_id.clone()])?;
    graph.add_dependencies(&course4_id, UnitType::Course, &vec![course2_id.clone()])?;
    graph.add_dependencies(&course5_id, UnitType::Course, &vec![course3_id.clone()])?;

    // Add a cycle, which should be detected when calling check_cycles().
    graph.add_dependencies(&course1_id, UnitType::Course, &vec![course5_id.clone()])?;
    assert!(graph.check_cycles().is_err());

    Ok(())
}

#[test]
fn generate_dot_graph() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let course1_id = Ustr::from("1");
    let course1_lesson1_id = Ustr::from("1::1");
    let course1_lesson2_id = Ustr::from("1::2");
    let course2_id = Ustr::from("2");
    let course2_lesson1_id = Ustr::from("2::1");
    let course3_id = Ustr::from("3");
    let course3_lesson1_id = Ustr::from("3::1");
    let course3_lesson2_id = Ustr::from("3::2");

    graph.add_lesson(&course1_lesson1_id, &course1_id)?;
    graph.add_lesson(&course1_lesson2_id, &course1_id)?;
    graph.add_lesson(&course2_lesson1_id, &course2_id)?;
    graph.add_lesson(&course3_lesson1_id, &course3_id)?;
    graph.add_lesson(&course3_lesson2_id, &course3_id)?;

    graph.add_dependencies(&course1_id, UnitType::Course, &vec![])?;
    graph.add_dependencies(
        &course1_lesson2_id,
        UnitType::Lesson,
        &vec![course1_lesson1_id.clone()],
    )?;
    graph.add_dependencies(&course2_id, UnitType::Course, &vec![course1_id.clone()])?;
    graph.add_dependencies(&course3_id, UnitType::Course, &vec![course2_id.clone()])?;
    graph.add_dependencies(
        &course3_lesson2_id,
        UnitType::Lesson,
        &vec![course3_lesson1_id.clone()],
    )?;
    graph.update_starting_lessons();

    let dot = graph.generate_dot_graph();
    let expected = indoc! {r#"
    digraph dependent_graph {
        "1" -> "1::1"
        "1" -> "2"
        "1::1" -> "1::2"
        "2" -> "2::1"
        "2" -> "3"
        "3" -> "3::1"
        "3::1" -> "3::2"
    }
    "#};
    assert_eq!(dot, expected);
    Ok(())
}
