use std::collections::HashSet;

use anyhow::Result;

use crate::data::UnitType;

use super::{InMemoryUnitGraph, UnitGraph};

#[test]
fn get_uid_and_id_and_type() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let id = "id1".to_string();
    graph.add_dependencies(&id, UnitType::Course, &vec![])?;
    assert_eq!(graph.get_uid(&id), Some(1));
    assert_eq!(graph.get_id(1), Some(id));
    assert_eq!(graph.get_unit_type(1), Some(UnitType::Course));
    Ok(())
}

#[test]
fn get_course_lessons_and_exercises() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let course_id = "course1".to_string();
    let lesson1_id = "course1::lesson1".to_string();
    let lesson2_id = "course1::lesson2".to_string();
    let lesson1_exercise1_id = "course1::lesson1::exercise1".to_string();
    let lesson1_exercise2_id = "course1::lesson1::exercise2".to_string();
    let lesson2_exercise1_id = "course1::lesson2::exercise1".to_string();
    let lesson2_exercise2_id = "course1::lesson2::exercise2".to_string();
    graph.add_dependencies(&course_id, UnitType::Course, &vec![])?;

    graph.add_lesson(&lesson1_id, &course_id)?;
    graph.add_exercise(&lesson1_exercise1_id, &lesson1_id)?;
    graph.add_exercise(&lesson1_exercise2_id, &lesson1_id)?;
    graph.add_lesson(&lesson2_id, &course_id)?;
    graph.add_exercise(&lesson2_exercise1_id, &lesson2_id)?;
    graph.add_exercise(&lesson2_exercise2_id, &lesson2_id)?;

    assert_eq!(
        graph
            .get_course_lessons(graph.get_uid(&course_id).unwrap())
            .unwrap(),
        HashSet::from([
            graph.get_uid(&lesson1_id).unwrap(),
            graph.get_uid(&lesson2_id).unwrap(),
        ])
    );
    assert_eq!(
        graph
            .get_lesson_exercises(graph.get_uid(&lesson1_id).unwrap())
            .unwrap(),
        HashSet::from([
            graph.get_uid(&lesson1_exercise1_id).unwrap(),
            graph.get_uid(&lesson1_exercise2_id).unwrap(),
        ])
    );
    assert_eq!(
        graph
            .get_lesson_exercises(graph.get_uid(&lesson2_id).unwrap())
            .unwrap(),
        HashSet::from([
            graph.get_uid(&lesson2_exercise1_id).unwrap(),
            graph.get_uid(&lesson2_exercise2_id).unwrap(),
        ])
    );

    Ok(())
}

#[test]
fn dependencies() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let course1_id = "course1".to_string();
    let course2_id = "course2".to_string();
    let course3_id = "course3".to_string();
    let course4_id = "course4".to_string();
    let course5_id = "course5".to_string();
    graph.add_dependencies(&course1_id, UnitType::Course, &vec![])?;
    graph.add_dependencies(&course2_id, UnitType::Course, &vec![course1_id.clone()])?;
    graph.add_dependencies(&course3_id, UnitType::Course, &vec![course1_id.clone()])?;
    graph.add_dependencies(&course4_id, UnitType::Course, &vec![course2_id.clone()])?;
    graph.add_dependencies(&course5_id, UnitType::Course, &vec![course3_id.clone()])?;

    assert_eq!(
        graph
            .get_dependents(graph.get_uid(&course1_id).unwrap())
            .unwrap(),
        HashSet::from([
            graph.get_uid(&course2_id).unwrap(),
            graph.get_uid(&course3_id).unwrap(),
        ])
    );
    assert_eq!(
        graph
            .get_dependencies(graph.get_uid(&course2_id).unwrap())
            .unwrap(),
        HashSet::from([graph.get_uid(&course1_id).unwrap()])
    );
    assert_eq!(
        graph
            .get_dependents(graph.get_uid(&course2_id).unwrap())
            .unwrap(),
        HashSet::from([graph.get_uid(&course4_id).unwrap()])
    );
    assert_eq!(
        graph
            .get_dependencies(graph.get_uid(&course4_id).unwrap())
            .unwrap(),
        HashSet::from([graph.get_uid(&course2_id).unwrap()])
    );
    assert_eq!(
        graph
            .get_dependents(graph.get_uid(&course3_id).unwrap())
            .unwrap(),
        HashSet::from([graph.get_uid(&course5_id).unwrap()])
    );
    assert_eq!(
        graph
            .get_dependencies(graph.get_uid(&course5_id).unwrap())
            .unwrap(),
        HashSet::from([graph.get_uid(&course3_id).unwrap()])
    );
    assert_eq!(
        graph.get_dependency_sinks(),
        HashSet::from([graph.get_uid(&course1_id).unwrap(),])
    );
    graph.check_cycles()?;

    Ok(())
}

#[test]
fn dependencies_cycle() -> Result<()> {
    let mut graph = InMemoryUnitGraph::default();
    let course1_id = "course1".to_string();
    let course2_id = "course2".to_string();
    let course3_id = "course3".to_string();
    let course4_id = "course4".to_string();
    let course5_id = "course5".to_string();
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
