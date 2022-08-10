//! Module defining the dependency graph and the basic operations that can be applied to it.
#[cfg(test)]
mod tests;

use std::fmt::Write;

use anyhow::{anyhow, ensure, Result};
use ustr::{Ustr, UstrMap, UstrSet};

use crate::data::UnitType;

/// Stores the dependency relationships between units. It only provides basic functions to update
/// the graph and query the outgoing or ingoing edges of a node.
pub trait UnitGraph {
    /// Adds a new course to the unit graph. This function should be called before the dependencies
    /// and lessons of this course are added to properly check that the course ID is unique.
    fn add_course(&mut self, course_id: &Ustr) -> Result<()>;

    /// Adds a new lesson to the unit graph. This lesson should be called before the dependencies
    /// and exercises of this lesson are added to properly check that the lesson ID is unique.
    fn add_lesson(&mut self, lesson_id: &Ustr, course_id: &Ustr) -> Result<()>;

    /// Adds a new exercise to the unit graph.
    fn add_exercise(&mut self, exercise_id: &Ustr, lesson_id: &Ustr) -> Result<()>;

    /// Takes a unit and its dependencies and updates the graph accordingly. Returns an error if
    /// unit_type is UnitType::Exercise as only courses and lessons are allowed to have
    /// dependencies.
    fn add_dependencies(
        &mut self,
        unit_id: &Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
    ) -> Result<()>;

    /// Returns the type of the given unit.
    fn get_unit_type(&self, unit_id: &Ustr) -> Option<UnitType>;

    /// Returns the lessons belonging to the given course.
    fn get_course_lessons(&self, course_id: &Ustr) -> Option<UstrSet>;

    /// Returns the lessons in the given course that do not depend upon any of the other lessons in
    /// the course.
    fn get_course_starting_lessons(&self, course_id: &Ustr) -> Option<UstrSet>;

    /// Updates the starting lessons for all courses.
    fn update_starting_lessons(&mut self);

    /// Returns the course to which the given lesson belongs.
    fn get_lesson_course(&self, lesson_id: &Ustr) -> Option<Ustr>;

    /// Returns the exercises belonging to the given lesson.
    fn get_lesson_exercises(&self, lesson_id: &Ustr) -> Option<UstrSet>;

    /// Returns the lesson to which the given exercise belongs.
    fn get_exercise_lesson(&self, exercise_id: &Ustr) -> Option<Ustr>;

    /// Returns the dependencies of the given unit.
    fn get_dependencies(&self, unit_id: &Ustr) -> Option<UstrSet>;

    /// Returns all the units which depend on the given unit.
    fn get_dependents(&self, unit_id: &Ustr) -> Option<UstrSet>;

    /// Returns the courses which have no dependencies, that is, the courses from which a walk of
    /// the unit graph can be safely started.
    fn get_dependency_sinks(&self) -> UstrSet;

    /// Checks that there are no cycles in the graph.
    fn check_cycles(&self) -> Result<()>;

    /// Generates a DOT graph of the dependent graph. The dependent graph is outputted instead of
    /// the dependency graph so that the output is easier to follow along.
    fn generate_dot_graph(&self) -> String;
}

/// Implements the UnitGraph trait based on two hash maps storing the dependency relationships.
#[derive(Default)]
pub(crate) struct InMemoryUnitGraph {
    /// The mapping of a unit to its type.
    type_map: UstrMap<UnitType>,

    /// The mapping of a course to its lessons.
    course_lesson_map: UstrMap<UstrSet>,

    /// The mapping of a course to its starting lessons. Starting lessons are those lessons in a
    /// course which do not depend on any other lesson in the course.
    starting_lessons_map: UstrMap<UstrSet>,

    /// The mapping of a lesson to its course.
    lesson_course_map: UstrMap<Ustr>,

    /// The mapping of a lesson to its exercises.
    lesson_exercise_map: UstrMap<UstrSet>,

    /// The mapping of an exercise to its lesson.
    exercise_lesson_map: UstrMap<Ustr>,

    /// The mapping of a unit to its dependencies.
    dependency_graph: UstrMap<UstrSet>,

    /// The mappinng of a unit to all the units which depend on it.
    reverse_graph: UstrMap<UstrSet>,

    /// The units which have no dependencies, that is, the sinks of the dependency graph.
    dependency_sinks: UstrSet,
}

impl InMemoryUnitGraph {
    /// Updates the set of units with no dependencies.
    fn update_dependency_sinks(&mut self, unit_id: &Ustr, dependencies: &[Ustr]) {
        let empty = UstrSet::default();
        let current_dependencies = self.dependency_graph.get(unit_id).unwrap_or(&empty);
        if current_dependencies.is_empty() && dependencies.is_empty() {
            self.dependency_sinks.insert(*unit_id);
        } else {
            self.dependency_sinks.remove(unit_id);
        }
    }

    /// Updates the type of the given unit. Returns an error if the unit already had a type and it's
    /// different than the type provided to this function.
    fn update_unit_type(&mut self, unit_id: &Ustr, unit_type: UnitType) -> Result<()> {
        match self.type_map.get(unit_id) {
            None => {
                self.type_map.insert(*unit_id, unit_type);
                Ok(())
            }
            Some(existing_type) => {
                if unit_type == *existing_type {
                    Ok(())
                } else {
                    Err(anyhow!("cannot update unit type to a different value"))
                }
            }
        }
    }
}

impl UnitGraph for InMemoryUnitGraph {
    fn add_course(&mut self, course_id: &Ustr) -> Result<()> {
        if self.type_map.contains_key(course_id) {
            return Err(anyhow!("course with ID {} already exists", course_id));
        }

        self.update_unit_type(course_id, UnitType::Course)?;
        Ok(())
    }

    fn add_lesson(&mut self, lesson_id: &Ustr, course_id: &Ustr) -> Result<()> {
        if self.type_map.contains_key(lesson_id) {
            return Err(anyhow!("lesson with ID {} already exists", lesson_id));
        }
        self.update_unit_type(lesson_id, UnitType::Lesson)?;
        self.update_unit_type(course_id, UnitType::Course)?;

        self.lesson_course_map.insert(*lesson_id, *course_id);
        self.course_lesson_map
            .entry(*course_id)
            .or_insert_with(UstrSet::default)
            .insert(*lesson_id);
        Ok(())
    }

    fn add_exercise(&mut self, exercise_id: &Ustr, lesson_id: &Ustr) -> Result<()> {
        if self.type_map.contains_key(exercise_id) {
            return Err(anyhow!("exercise with ID {} already exists", exercise_id));
        }
        self.update_unit_type(exercise_id, UnitType::Exercise)?;
        self.update_unit_type(lesson_id, UnitType::Lesson)?;

        self.lesson_exercise_map
            .entry(*lesson_id)
            .or_insert_with(UstrSet::default)
            .insert(*exercise_id);
        self.exercise_lesson_map.insert(*exercise_id, *lesson_id);
        Ok(())
    }

    fn add_dependencies(
        &mut self,
        unit_id: &Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
    ) -> Result<()> {
        ensure!(
            unit_type != UnitType::Exercise,
            "exercise {} cannot have dependencies",
            unit_id,
        );
        ensure!(
            dependencies.iter().all(|dep| dep != unit_id),
            "unit {} cannot depend on itself",
            unit_id,
        );
        ensure!(
            self.type_map.contains_key(unit_id),
            "unit {} of type {:?} must be explicitly added before adding dependencies",
            unit_id,
            unit_type,
        );

        self.update_dependency_sinks(unit_id, dependencies);
        for dep_id in dependencies {
            // Update the dependency sinks for all dependencies so that the scheduler works even in
            // the case some dependencies are missing.
            self.update_dependency_sinks(dep_id, &[]);
        }

        self.dependency_graph
            .entry(*unit_id)
            .or_insert_with(UstrSet::default)
            .extend(dependencies);
        for dependency_id in dependencies {
            self.reverse_graph
                .entry(*dependency_id)
                .or_insert_with(UstrSet::default)
                .insert(*unit_id);
        }
        Ok(())
    }

    fn get_unit_type(&self, unit_id: &Ustr) -> Option<UnitType> {
        self.type_map.get(unit_id).cloned()
    }

    fn get_course_lessons(&self, course_id: &Ustr) -> Option<UstrSet> {
        self.course_lesson_map.get(course_id).cloned()
    }

    fn get_course_starting_lessons(&self, course_id: &Ustr) -> Option<UstrSet> {
        self.starting_lessons_map.get(course_id).cloned()
    }

    fn update_starting_lessons(&mut self) {
        let empty = UstrSet::default();
        for course_id in self.course_lesson_map.keys() {
            let lessons = self.course_lesson_map.get(course_id).unwrap_or(&empty);

            let starting_lessons = lessons
                .iter()
                .copied()
                .filter(|lesson_id| {
                    let dependencies = self.get_dependencies(lesson_id);
                    match dependencies {
                        None => true,
                        Some(dependencies) => lessons.is_disjoint(&dependencies),
                    }
                })
                .collect();
            self.starting_lessons_map
                .insert(*course_id, starting_lessons);
        }
    }

    fn get_lesson_course(&self, lesson_id: &Ustr) -> Option<Ustr> {
        self.lesson_course_map.get(lesson_id).cloned()
    }

    fn get_lesson_exercises(&self, lesson_id: &Ustr) -> Option<UstrSet> {
        self.lesson_exercise_map.get(lesson_id).cloned()
    }

    fn get_exercise_lesson(&self, exercise_id: &Ustr) -> Option<Ustr> {
        self.exercise_lesson_map.get(exercise_id).cloned()
    }

    fn get_dependencies(&self, unit_id: &Ustr) -> Option<UstrSet> {
        self.dependency_graph.get(unit_id).cloned()
    }

    fn get_dependents(&self, unit_id: &Ustr) -> Option<UstrSet> {
        self.reverse_graph.get(unit_id).cloned()
    }

    fn get_dependency_sinks(&self) -> UstrSet {
        self.dependency_sinks.clone()
    }

    fn check_cycles(&self) -> Result<()> {
        let mut visited = UstrSet::default();
        for unit_id in self.dependency_graph.keys() {
            if visited.contains(unit_id) {
                continue;
            }

            // Start a depth-first search from the given unit and return an error if the same unit
            // is found more than once in any path.
            let mut stack: Vec<Vec<Ustr>> = Vec::new();
            stack.push(vec![*unit_id]);
            while let Some(path) = stack.pop() {
                let current_id = *path.last().unwrap_or(&Ustr::default());
                if visited.contains(&current_id) {
                    continue;
                } else {
                    visited.insert(current_id);
                }

                let dependencies = self.get_dependencies(&current_id);
                if let Some(dependencies) = dependencies {
                    for dependency_id in dependencies {
                        let dependents = self.get_dependents(&dependency_id);
                        if let Some(dependents) = dependents {
                            // Verify the integrity of the graph by checking that the dependencies
                            // of the current unit list it as a dependent.
                            if !dependents.contains(&current_id) {
                                return Err(anyhow!(
                                    "unit {} lists unit {} as a dependency but the reverse \
                                    relationship does not exist",
                                    current_id,
                                    dependency_id
                                ));
                            }
                        }

                        // Return with an error if there's a cycle in the path and continue the
                        // search otherwise.
                        if path.contains(&dependency_id) {
                            return Err(anyhow!("cycle in dependency graph detected"));
                        }
                        let mut new_path = path.clone();
                        new_path.push(dependency_id);
                        stack.push(new_path);
                    }
                }
            }
        }
        Ok(())
    }

    fn generate_dot_graph(&self) -> String {
        let mut output = String::from("digraph dependent_graph {\n");
        let mut courses = self.course_lesson_map.keys().cloned().collect::<Vec<_>>();
        courses.sort();

        for course_id in courses {
            // Add all the dependents of the course to the graph.
            let mut dependents = self
                .get_dependents(&course_id)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();

            // A course's lessons are attached to the graph by making the starting lessons a
            // dependent of the course. This is not exactly accurate, but properly adding them to
            // the graph would require each course to have two nodes, one inboud, connected to the
            // starting lessons, and one outbound, connected to the ending lessons and to the
            // dependents of the course.
            dependents.extend(
                self.get_course_starting_lessons(&course_id)
                    .unwrap_or_default()
                    .iter(),
            );
            dependents.sort();

            for dependent in dependents {
                let _ = writeln!(output, "    \"{}\" -> \"{}\"", course_id, dependent);
            }

            // Add the dependents of each lesson in the course to the graph.
            let mut lessons = self
                .get_course_lessons(&course_id)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            lessons.sort();
            for lesson_id in lessons {
                let mut dependents = self
                    .get_dependents(&lesson_id)
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<Vec<_>>();
                dependents.sort();

                for dependent in dependents {
                    let _ = writeln!(output, "    \"{}\" -> \"{}\"", lesson_id, dependent);
                }
            }
        }
        output.push_str("}\n");
        output
    }
}
