//! Defines the dependency graph of units of knowledge, their dependency relationships, and basic
//! read and write operations.
//!
//! The dependency graph is perhaps the most important part of the design of Trane so its nature and
//! purpose should be well documented. At its core, the goal of Trane is to guide students through
//! the graph of units of knowledge composed of exercises, by having each successive unit teach a
//! skill that can be acquired once the previous units are sufficiently mastered. This process of
//! repetition of mastered exercises and introduction of new ones should lead to the complete
//! mastery of complex meta-skills such as jazz improvisation, chess, piano, etc. that are in fact
//! the mastered integration of many smaller and interlinked skills.
//!
//! This graph is implemented by simulating a directed acyclic graph (DAG) of units and
//! dependency/dependents relationships among them. A unit can be of three types:
//! 1. An exercise, which represents a single task testing a skill which the student is required to
//!    assess when practiced.
//! 2. A lesson, which represents a collection of exercises which test the same skill and can be
//!    practiced in any order.
//! 3. A course, a collection of lessons which are related. It mostly exists to help organize the
//!    material in larger entities which share some context.
//!
//! The relationships between the units is one of the following:
//! 1. A course or lesson A is a dependency of course or lesson B if A needs to be sufficiently
//!    mastered before B can be practiced.
//! 2. The reverse relationship. Thus, we say that B is a dependent of A.
//! 3. A course or lesson A is superseded by another course or lesson B if sufficient mastery of B
//!    makes showing exercises from A redundant.
//! 4. The reverse relationship. Thus, we say that B supersedes A.
//!
//! The graph also provides a number of operations to manipulate the graph, which are only used when
//! reading the Trane library (see [`course_library`](crate::course_library)), and another few to
//! derive information from the graph ("which are the lessons in a course?" for example). The graph
//! is not in any way responsible for how the exercises are scheduled (see
//! [`scheduler`](crate::scheduler)) nor it stores any information about a student's practice (see
//! [`practice_stats`](crate::practice_stats)) or preferences (see [`blacklist`](crate::blacklist),
//! [`filter_manager`](crate::filter_manager) and [`review_list`](crate::review_list)).

use anyhow::{anyhow, bail, ensure, Result};
use std::fmt::Write;
use ustr::{Ustr, UstrMap, UstrSet};

use crate::{data::UnitType, error::UnitGraphError};

/// Stores the units and their dependency relationships (for lessons and courses only, since
/// exercises do not define any dependencies). It provides basic functions to update the graph and
/// retrieve information about it for use during scheduling and student's requests.
///
/// The operations that update the graph are only used when reading the Trane library during
/// startup. A user that copies new courses to an existing and currently opened library will need to
/// restart Trane to see the changes take effect.
pub trait UnitGraph {
    /// Adds a new course to the unit graph.
    fn add_course(&mut self, course_id: Ustr) -> Result<(), UnitGraphError>;

    /// Adds a new lesson to the unit graph. It also takes the ID of the course to which this lesson
    /// belongs.
    fn add_lesson(&mut self, lesson_id: Ustr, course_id: Ustr) -> Result<(), UnitGraphError>;

    /// Adds a new exercise to the unit graph. It also takes the ID of the lesson to which this
    /// exercise belongs.
    fn add_exercise(&mut self, exercise_id: Ustr, lesson_id: Ustr) -> Result<(), UnitGraphError>;

    /// Takes a unit and its dependencies and updates the graph accordingly. Returns an error if
    /// `unit_type` is `UnitType::Exercise` as only courses and lessons are allowed to have
    /// dependencies. An error is also returned if the unit was not previously added by calling one
    /// of `add_course` or `add_lesson`.
    fn add_dependencies(
        &mut self,
        unit_id: Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
        weights: &[f32],
    ) -> Result<(), UnitGraphError>;

    /// Adds the list of superseded units for the given unit to the graph.
    fn add_superseded(&mut self, unit_id: Ustr, superseded: &[Ustr]);

    /// Returns the type of the given unit.
    fn get_unit_type(&self, unit_id: Ustr) -> Option<UnitType>;

    /// Returns the lessons belonging to the given course.
    fn get_course_lessons(&self, course_id: Ustr) -> Option<UstrSet>;

    /// Updates the starting lessons for all courses. The starting lessons of the course are those
    /// of its lessons that should be practiced first when the course is introduced to the student.
    /// The scheduler uses them to traverse through the other lessons in the course in the correct
    /// order. This function should be called once after all the courses and lessons have been added
    /// to the graph.
    fn update_starting_lessons(&mut self);

    /// Returns the starting lessons for the given course.
    fn get_starting_lessons(&self, course_id: Ustr) -> Option<UstrSet>;

    /// Returns the course to which the given lesson belongs.
    fn get_lesson_course(&self, lesson_id: Ustr) -> Option<Ustr>;

    /// Returns the exercises belonging to the given lesson.
    fn get_lesson_exercises(&self, lesson_id: Ustr) -> Option<UstrSet>;

    /// Returns the lesson to which the given exercise belongs.
    fn get_exercise_lesson(&self, exercise_id: Ustr) -> Option<Ustr>;

    /// Returns the weights of the dependencies of the given unit.
    fn get_dependencies(&self, unit_id: Ustr) -> Option<UstrSet>;

    /// Returns the dependency weights of the given unit.
    fn get_dependency_weights(&self, unit_id: Ustr) -> Option<UstrMap<f32>>;

    /// Returns all the units which depend on the given unit.
    fn get_dependents(&self, unit_id: Ustr) -> Option<UstrSet>;

    /// Returns the weights of the dependents of the given unit.
    fn get_dependent_weights(&self, unit_id: Ustr) -> Option<UstrMap<f32>>;

    /// Returns the dependency sinks of the graph. A dependency sink is a unit with no dependencies
    /// from which a walk of the entire unit graph needs to start. Because the lessons in a course
    /// implicitly depend on their course, properly initialized lessons do not belong to this set.
    ///
    /// This set also includes the units that are mentioned as dependencies of other units but are
    /// never added to the graph because they are missing from the course library. Those units are
    /// added as dependency sinks so that the scheduler can reach their dependents, which are part
    /// of the library.
    fn get_dependency_sinks(&self) -> UstrSet;

    /// Returns the lessons and courses that are superseded by the given lesson or course.
    fn get_superseded(&self, unit_id: Ustr) -> Option<UstrSet>;

    /// Returns the lessons and courses that supersede the given lesson or course.
    fn get_superseding(&self, unit_id: Ustr) -> Option<UstrSet>;

    /// Performs a cycle check on the graph, done currently when opening the Trane library to
    /// prevent any infinite traversal of the graph and immediately inform the user of the issue.
    fn check_cycles(&self) -> Result<(), UnitGraphError>;

    /// Generates a DOT graph of the dependent graph. DOT files are used by Graphviz to visualize a
    /// graph, in this case the dependent graph. This operation was suggested in issue
    /// [#13](https://github.com/trane-project/trane-cli/issues/13) in the
    /// [trane-cli](https://github.com/trane-project/trane-cli) repo.
    ///
    /// This allows users to have some way to visualize the graph without having to implement such a
    /// feature and depend on Graphviz instead.
    ///
    /// The dependent graph is outputted instead of the dependency graph so that the output is
    /// easier to read. If you follow the arrows, then you are traversing the path that students
    /// must take to master a skill.
    fn generate_dot_graph(&self) -> String;
}

/// An implementation of [`UnitGraph`] describing the units and relationships as an adjacency list
/// stored in hash maps. All of it is stored in memory, as the memory benchmarks show that less than
/// 20 MB of memory are used even when opening a large Trane library.
#[derive(Default)]
pub struct InMemoryUnitGraph {
    /// The mapping of a unit to its type.
    type_map: UstrMap<UnitType>,

    /// The mapping of a course to its lessons.
    course_lesson_map: UstrMap<UstrSet>,

    /// The mapping of a course to its starting lessons.
    starting_lessons_map: UstrMap<UstrSet>,

    /// The mapping of a lesson to its course.
    lesson_course_map: UstrMap<Ustr>,

    /// The mapping of a lesson to its exercises.
    lesson_exercise_map: UstrMap<UstrSet>,

    /// The mapping of an exercise to its lesson.
    exercise_lesson_map: UstrMap<Ustr>,

    /// The mapping of a unit to its dependencies.
    dependency_graph: UstrMap<UstrSet>,

    /// The mapping of a unit to the weights of its dependencies.
    dependency_weights: UstrMap<UstrMap<f32>>,

    /// The mapping of a unit to all its dependents.
    dependent_graph: UstrMap<UstrSet>,

    /// The mapping of a unit to the weights of its dependents.
    dependent_weights: UstrMap<UstrMap<f32>>,

    /// The set of all dependency sinks in the graph.
    dependency_sinks: UstrSet,

    /// The mapping of a unit to the units it supersedes.
    superseded_graph: UstrMap<UstrSet>,

    /// The mapping of a unit to the units that supersede it.
    superseding_graph: UstrMap<UstrSet>,
}

impl InMemoryUnitGraph {
    /// Updates the dependency sinks of the given unit when the given unit and dependencies are
    /// added to the graph.
    fn update_dependency_sinks(&mut self, unit_id: Ustr, dependencies: &[Ustr]) {
        // If the current dependencies and the new dependencies are both empty, keep the unit in the
        // set of dependency sinks. Otherwise, remove it.
        let empty = UstrSet::default();
        let current_dependencies = self.dependency_graph.get(&unit_id).unwrap_or(&empty);
        if current_dependencies.is_empty() && dependencies.is_empty() {
            self.dependency_sinks.insert(unit_id);
        } else {
            self.dependency_sinks.remove(&unit_id);
        }

        // Remove the unit from the dependency sinks if it's a lesson and its course exists. If the
        // course is a dependency sink, the lesson is redundant. If the course is not a dependency
        // sink, the lesson is not a dependency sink either.
        if self.get_lesson_course(unit_id).is_some() {
            self.dependency_sinks.remove(&unit_id);
        }

        // If a course is mentioned as a dependency, but it's missing, it should be a dependency
        // sink. To ensure this requirement, the function is called recursively on all the
        // dependents with an empty dependency set. It's safe to do this because a call to this
        // function for a course with an empty dependency list followed by another with a non-empty
        // list has the same result as only executing the second call, but makes sure that any
        // missing courses are added to the dependency sinks.
        for dependency_id in dependencies {
            self.update_dependency_sinks(*dependency_id, &[]);
        }
    }

    /// Updates the type of the given unit. Returns an error if the unit already had a type, and
    /// it's different from the type provided in the function call.
    fn update_unit_type(&mut self, unit_id: Ustr, unit_type: UnitType) -> Result<()> {
        match self.type_map.get(&unit_id) {
            None => {
                self.type_map.insert(unit_id, unit_type);
                Ok(())
            }
            Some(existing_type) => {
                if unit_type == *existing_type {
                    Ok(())
                } else {
                    Err(anyhow!(
                        "cannot update unit type of unit {} from type {:#?}) to {:#?}.",
                        unit_id,
                        existing_type,
                        unit_type
                    ))
                }
            }
        }
    }

    /// Helper function to add a course to the graph.
    fn add_course_helper(&mut self, course_id: Ustr) -> Result<()> {
        // Verify the course doesn't already exist.
        ensure!(
            !self.type_map.contains_key(&course_id),
            "course with ID {} already exists",
            course_id
        );

        // Add the course to the type map to mark it as existing.
        self.update_unit_type(course_id, UnitType::Course)?;
        Ok(())
    }

    /// Helper function to add a lesson to the graph.
    fn add_lesson_helper(&mut self, lesson_id: Ustr, course_id: Ustr) -> Result<()> {
        // Verify the lesson doesn't already exist.
        ensure!(
            !self.type_map.contains_key(&lesson_id),
            "lesson with ID {} already exists",
            lesson_id
        );

        // Add the course and lessons to the type map.
        self.update_unit_type(lesson_id, UnitType::Lesson)?;
        self.update_unit_type(course_id, UnitType::Course)?;

        // Update the map of lesson to course and course to lessons.
        self.lesson_course_map.insert(lesson_id, course_id);
        self.course_lesson_map
            .entry(course_id)
            .or_default()
            .insert(lesson_id);
        Ok(())
    }

    /// Helper function to add an exercise to the graph.
    fn add_exercise_helper(&mut self, exercise_id: Ustr, lesson_id: Ustr) -> Result<()> {
        // Verify the exercise doesn't already exist.
        ensure!(
            !self.type_map.contains_key(&exercise_id),
            "exercise with ID {} already exists",
            exercise_id
        );

        // Add the exercise and lesson to the type map.
        self.update_unit_type(exercise_id, UnitType::Exercise)?;
        self.update_unit_type(lesson_id, UnitType::Lesson)?;

        // Update the map of exercise to lesson and lesson to exercises.
        self.lesson_exercise_map
            .entry(lesson_id)
            .or_default()
            .insert(exercise_id);
        self.exercise_lesson_map.insert(exercise_id, lesson_id);
        Ok(())
    }

    // Performs some sanity checks before adding a dependency.
    #[cfg_attr(coverage, coverage(off))]
    fn verify_dependencies(
        &self,
        unit_id: Ustr,
        unit_type: &UnitType,
        dependencies: &[Ustr],
    ) -> Result<()> {
        ensure!(
            *unit_type != UnitType::Exercise,
            "exercise {} cannot have dependencies",
            unit_id,
        );
        ensure!(
            dependencies.iter().all(|dep| *dep != unit_id),
            "unit {} cannot depend on itself",
            unit_id,
        );
        ensure!(
            self.type_map.contains_key(&unit_id),
            "unit {} of type {:?} must be explicitly added before adding dependencies",
            unit_id,
            unit_type,
        );
        Ok(())
    }

    /// Helper function to add dependencies to a unit.
    fn add_dependencies_helper(
        &mut self,
        unit_id: Ustr,
        unit_type: &UnitType,
        dependencies: &[Ustr],
        weights: &[f32],
    ) -> Result<()> {
        // Sanity checks.
        self.verify_dependencies(unit_id, unit_type, dependencies)?;
        ensure!(
            dependencies.len() == weights.len(),
            "dependencies and weights must have the same length"
        );

        // Update the dependency sinks and dependency map.
        self.update_dependency_sinks(unit_id, dependencies);
        self.dependency_graph
            .entry(unit_id)
            .or_default()
            .extend(dependencies);

        // For each dependency, insert the equivalent dependent relationship.
        for dependency_id in dependencies {
            self.dependent_graph
                .entry(*dependency_id)
                .or_default()
                .insert(unit_id);
        }

        // Update the dependency and dependent weights.
        let dependency_and_weights: Vec<(Ustr, f32)> = dependencies
            .iter()
            .zip(weights.iter())
            .map(|(dep, weight)| (*dep, *weight))
            .collect();
        if !dependency_and_weights.is_empty() {
            self.dependency_weights
                .entry(unit_id)
                .or_default()
                .extend(dependency_and_weights.clone());
            for (id, weight) in dependency_and_weights {
                self.dependent_weights
                    .entry(id)
                    .or_default()
                    .insert(unit_id, weight);
            }
        }
        Ok(())
    }

    /// Helper function to check for cycles in the dependency graph.
    fn check_cycles_helper(&self) -> Result<()> {
        // Perform a depth-first search of the dependency graph from each unit. Return an error if
        // the same unit is encountered twice during the search.
        let mut visited = UstrSet::default();
        for unit_id in self.dependency_graph.keys() {
            // The node has been visited, so it can be skipped.
            if visited.contains(unit_id) {
                continue;
            }

            // The stacks store a path of traversed units and is initialized with the current unit.
            let mut stack: Vec<Vec<Ustr>> = Vec::new();
            stack.push(vec![*unit_id]);

            // Run a depth-first search and stop if a cycle is found or the graph is exhausted.
            while let Some(path) = stack.pop() {
                // Update the set of visited nodes.
                let current_id = *path.last().unwrap_or(&Ustr::default());
                if visited.contains(&current_id) {
                    continue;
                }
                visited.insert(current_id);

                // Get the dependencies of the current node, check that the dependency and dependent
                // graph agree with each other, and generate new paths to add to the stack.
                if let Some(dependencies) = self.get_dependencies(current_id) {
                    for dependency_id in dependencies {
                        // Verify that the dependency and dependent graphs agree with each other by
                        // checking that this dependency lists the current unit as a dependent.
                        let dependents = self.get_dependents(dependency_id).unwrap_or_default();
                        if !dependents.contains(&current_id) {
                            bail!(
                                "unit {} lists unit {} as a dependency but the dependent \
                                relationship does not exist",
                                current_id,
                                dependency_id
                            );
                        }

                        // Check for repeated nodes in the path.
                        if path.contains(&dependency_id) {
                            bail!("cycle in dependency graph detected");
                        }

                        // Add a new path to the stack.
                        let mut new_path = path.clone();
                        new_path.push(dependency_id);
                        stack.push(new_path);
                    }
                }
            }
        }

        // Do the same with the graph of superseded units.
        let mut visited = UstrSet::default();
        for unit_id in self.superseded_graph.keys() {
            // The node has been visited, so it can be skipped.
            if visited.contains(unit_id) {
                continue;
            }

            // The stacks store a path of traversed units and is initialized with the current unit.
            let mut stack: Vec<Vec<Ustr>> = Vec::new();
            stack.push(vec![*unit_id]);

            // Run a depth-first search and stop if a cycle is found or the graph is exhausted.
            while let Some(path) = stack.pop() {
                // Update the set of visited nodes.
                let current_id = *path.last().unwrap_or(&Ustr::default());
                if visited.contains(&current_id) {
                    continue;
                }
                visited.insert(current_id);

                // Get the  of the current node, check that the superseded and superseding graphs
                // agree with each other, and generate new paths to add to the stack.
                if let Some(superseded) = self.get_superseded(current_id) {
                    for superseded_id in superseded {
                        let superseding = self.get_superseding(superseded_id).unwrap_or_default();
                        if !superseding.contains(&current_id) {
                            bail!(
                                "unit {} lists unit {} as a superseded unit but the superseding \
                                relationship does not exist",
                                current_id,
                                superseded_id
                            );
                        }

                        // Check for repeated nodes in the path.
                        if path.contains(&superseded_id) {
                            bail!("cycle in superseded graph detected");
                        }

                        // Add a new path to the stack.
                        let mut new_path = path.clone();
                        new_path.push(superseded_id);
                        stack.push(new_path);
                    }
                }
            }
        }
        Ok(())
    }
}

impl UnitGraph for InMemoryUnitGraph {
    fn add_course(&mut self, course_id: Ustr) -> Result<(), UnitGraphError> {
        self.add_course_helper(course_id)
            .map_err(|e| UnitGraphError::AddUnit(course_id, UnitType::Course, e))
    }

    fn add_lesson(&mut self, lesson_id: Ustr, course_id: Ustr) -> Result<(), UnitGraphError> {
        self.add_lesson_helper(lesson_id, course_id)
            .map_err(|e| UnitGraphError::AddUnit(lesson_id, UnitType::Lesson, e))
    }

    fn add_exercise(&mut self, exercise_id: Ustr, lesson_id: Ustr) -> Result<(), UnitGraphError> {
        self.add_exercise_helper(exercise_id, lesson_id)
            .map_err(|e| UnitGraphError::AddUnit(exercise_id, UnitType::Exercise, e))
    }

    fn add_dependencies(
        &mut self,
        unit_id: Ustr,
        unit_type: UnitType,
        dependencies: &[Ustr],
        weights: &[f32],
    ) -> Result<(), UnitGraphError> {
        self.add_dependencies_helper(unit_id, &unit_type, dependencies, weights)
            .map_err(|e| UnitGraphError::AddDependencies(unit_id, unit_type, e))
    }

    fn add_superseded(&mut self, unit_id: Ustr, superseded: &[Ustr]) {
        // Update the superseded map.
        if superseded.is_empty() {
            return;
        }
        self.superseded_graph
            .entry(unit_id)
            .or_default()
            .extend(superseded);

        // For each superseded ID, insert the equivalent superseding relationship.
        for superseded_id in superseded {
            self.superseding_graph
                .entry(*superseded_id)
                .or_default()
                .insert(unit_id);
        }
    }

    fn get_unit_type(&self, unit_id: Ustr) -> Option<UnitType> {
        self.type_map.get(&unit_id).cloned()
    }

    fn get_course_lessons(&self, course_id: Ustr) -> Option<UstrSet> {
        self.course_lesson_map.get(&course_id).cloned()
    }

    fn get_starting_lessons(&self, course_id: Ustr) -> Option<UstrSet> {
        self.starting_lessons_map.get(&course_id).cloned()
    }

    fn update_starting_lessons(&mut self) {
        // Find the starting lessons for each course.
        let empty = UstrSet::default();
        for course_id in self.course_lesson_map.keys() {
            let lessons = self.course_lesson_map.get(course_id).unwrap_or(&empty);
            let starting_lessons = lessons
                .iter()
                .copied()
                .filter(|lesson_id| {
                    // The lesson is a starting lesson if the set of lessons in the course and the
                    // dependencies of this lesson are disjoint.
                    let dependencies = self.get_dependencies(*lesson_id);
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

    fn get_lesson_course(&self, lesson_id: Ustr) -> Option<Ustr> {
        self.lesson_course_map.get(&lesson_id).copied()
    }

    fn get_lesson_exercises(&self, lesson_id: Ustr) -> Option<UstrSet> {
        self.lesson_exercise_map.get(&lesson_id).cloned()
    }

    fn get_exercise_lesson(&self, exercise_id: Ustr) -> Option<Ustr> {
        self.exercise_lesson_map.get(&exercise_id).copied()
    }

    fn get_dependencies(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.dependency_graph.get(&unit_id).cloned()
    }

    fn get_dependency_weights(&self, unit_id: Ustr) -> Option<UstrMap<f32>> {
        self.dependency_weights.get(&unit_id).cloned()
    }

    fn get_dependents(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.dependent_graph.get(&unit_id).cloned()
    }

    fn get_dependent_weights(&self, unit_id: Ustr) -> Option<UstrMap<f32>> {
        self.dependent_weights.get(&unit_id).cloned()
    }

    fn get_dependency_sinks(&self) -> UstrSet {
        self.dependency_sinks.clone()
    }

    fn get_superseded(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.superseded_graph.get(&unit_id).cloned()
    }

    fn get_superseding(&self, unit_id: Ustr) -> Option<UstrSet> {
        self.superseding_graph.get(&unit_id).cloned()
    }

    fn check_cycles(&self) -> Result<(), UnitGraphError> {
        self.check_cycles_helper()
            .map_err(UnitGraphError::CheckCycles)
    }

    fn generate_dot_graph(&self) -> String {
        // Initialize the output with the first line of the file.
        let mut output = String::from("digraph dependent_graph {\n");
        let mut courses = self.course_lesson_map.keys().copied().collect::<Vec<_>>();
        courses.sort();

        // Add each course to the DOT graph.
        for course_id in courses {
            // Add an entry for the course node and set the color to red.
            let _ = writeln!(output, "    \"{course_id}\" [color=red, style=filled]");

            // Write the entry in the graph for all the of the dependents of this course.
            let mut dependents = self
                .get_dependents(course_id)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();

            // Write the entry in the graph for all the lessons in the course.
            //
            // A course's lessons are not explicitly attached to the graph. This is not exactly
            // accurate, but properly connecting them in the graph would require each course to have
            // two nodes, one inbound which is connected to the starting lessons and the course's
            // dependencies, and one outbound which is connected to the last lessons in the course
            // (by the order in which they must be traversed to master the entire course) and to the
            // course's dependents. This might be amended, either here in this function or in the
            // implementation of the graph itself, but it is not a high priority.
            dependents.extend(
                self.get_starting_lessons(course_id)
                    .unwrap_or_default()
                    .iter(),
            );
            dependents.sort();
            for dependent in dependents {
                let _ = writeln!(output, "    \"{course_id}\" -> \"{dependent}\"");
            }

            // Repeat the same process for each lesson in this course.
            let mut lessons = self
                .get_course_lessons(course_id)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            lessons.sort();
            for lesson_id in lessons {
                // Add an entry for the lesson node and set the color to blue.
                let _ = writeln!(output, "    \"{lesson_id}\" [color=blue, style=filled]");

                // Add an entry in the graph for all of this lesson's dependents.
                let mut dependents = self
                    .get_dependents(lesson_id)
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<Vec<_>>();
                dependents.sort();
                for dependent in dependents {
                    let _ = writeln!(output, "    \"{lesson_id}\" -> \"{dependent}\"");
                }
            }
        }

        // Close the graph.
        output.push_str("}\n");
        output
    }
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod test {
    use anyhow::Result;
    use indoc::indoc;
    use ustr::{Ustr, UstrSet};

    use crate::{
        data::UnitType,
        graph::{InMemoryUnitGraph, UnitGraph},
    };

    /// Verifies retrieving the correct unit type from the graph.
    #[test]
    fn get_unit_type() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        let id = Ustr::from("id1");
        graph.add_course(id)?;
        graph.add_dependencies(id, UnitType::Course, &[], &[])?;
        assert_eq!(graph.get_unit_type(id), Some(UnitType::Course));
        Ok(())
    }

    /// Verifies the basic functionality of the graph, adding course, lessons, and exercises.
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

        graph.add_course(course_id)?;
        graph.add_dependencies(course_id, UnitType::Course, &[], &[])?;
        graph.add_lesson(lesson1_id, course_id)?;
        graph.add_exercise(lesson1_exercise1_id, lesson1_id)?;
        graph.add_exercise(lesson1_exercise2_id, lesson1_id)?;
        graph.add_lesson(lesson2_id, course_id)?;
        graph.add_exercise(lesson2_exercise1_id, lesson2_id)?;
        graph.add_exercise(lesson2_exercise2_id, lesson2_id)?;

        let course_lessons = graph.get_course_lessons(course_id).unwrap();
        assert_eq!(course_lessons.len(), 2);
        assert!(course_lessons.contains(&lesson1_id));
        assert!(course_lessons.contains(&lesson2_id));

        let lesson1_exercises = graph.get_lesson_exercises(lesson1_id).unwrap();
        assert_eq!(lesson1_exercises.len(), 2);
        assert!(lesson1_exercises.contains(&lesson1_exercise1_id));
        assert!(lesson1_exercises.contains(&lesson1_exercise2_id));
        assert_eq!(
            graph.get_exercise_lesson(lesson1_exercise1_id).unwrap(),
            lesson1_id
        );
        assert_eq!(
            graph.get_exercise_lesson(lesson1_exercise2_id).unwrap(),
            lesson1_id
        );

        let lesson2_exercises = graph.get_lesson_exercises(lesson2_id).unwrap();
        assert_eq!(lesson2_exercises.len(), 2);
        assert!(lesson2_exercises.contains(&lesson2_exercise1_id));
        assert!(lesson2_exercises.contains(&lesson2_exercise2_id));
        assert_eq!(
            graph.get_exercise_lesson(lesson2_exercise1_id).unwrap(),
            lesson2_id
        );
        assert_eq!(
            graph.get_exercise_lesson(lesson2_exercise2_id).unwrap(),
            lesson2_id
        );

        Ok(())
    }

    /// Verifies retrieving the correct dependencies and dependents from the graph.
    #[test]
    fn dependencies() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        let course1_id = Ustr::from("course1");
        let course2_id = Ustr::from("course2");
        let course3_id = Ustr::from("course3");
        let course4_id = Ustr::from("course4");
        let course5_id = Ustr::from("course5");
        graph.add_course(course1_id)?;
        graph.add_course(course2_id)?;
        graph.add_course(course3_id)?;
        graph.add_course(course4_id)?;
        graph.add_course(course5_id)?;
        graph.add_dependencies(course1_id, UnitType::Course, &[], &[])?;
        graph.add_dependencies(course2_id, UnitType::Course, &[course1_id], &[1.0])?;
        graph.add_dependencies(course3_id, UnitType::Course, &[course1_id], &[1.0])?;
        graph.add_dependencies(course4_id, UnitType::Course, &[course2_id], &[1.0])?;
        graph.add_dependencies(course5_id, UnitType::Course, &[course3_id], &[1.0])?;

        {
            let dependents = graph.get_dependents(course1_id).unwrap();
            assert_eq!(dependents.len(), 2);
            assert!(dependents.contains(&course2_id));
            assert!(dependents.contains(&course3_id));
            let dependent_weights = graph.get_dependent_weights(course1_id).unwrap();
            assert_eq!(dependent_weights.len(), 2);
            assert_eq!(dependent_weights[&course2_id], 1.0);
            assert_eq!(dependent_weights[&course3_id], 1.0);

            assert!(graph.get_dependencies(course1_id).unwrap().is_empty());
            assert!(graph.get_dependency_weights(course1_id).is_none());
        }

        {
            let dependents = graph.get_dependents(course2_id).unwrap();
            assert_eq!(dependents.len(), 1);
            assert!(dependents.contains(&course4_id));
            let dependent_weights = graph.get_dependent_weights(course2_id).unwrap();
            assert_eq!(dependent_weights.len(), 1);
            assert_eq!(dependent_weights[&course4_id], 1.0);

            let dependencies = graph.get_dependencies(course2_id).unwrap();
            assert_eq!(dependencies.len(), 1);
            assert!(dependencies.contains(&course1_id));
            let dependency_weights = graph.get_dependency_weights(course2_id).unwrap();
            assert_eq!(dependency_weights.len(), 1);
            assert_eq!(dependency_weights[&course1_id], 1.0);
        }

        {
            let dependents = graph.get_dependents(course3_id).unwrap();
            assert_eq!(dependents.len(), 1);
            assert!(dependents.contains(&course5_id));
            let dependent_weights = graph.get_dependent_weights(course3_id).unwrap();
            assert_eq!(dependent_weights.len(), 1);
            assert_eq!(dependent_weights[&course5_id], 1.0);

            let dependencies = graph.get_dependencies(course3_id).unwrap();
            assert_eq!(dependencies.len(), 1);
            assert!(dependencies.contains(&course1_id));
            let dependency_weights = graph.get_dependency_weights(course3_id).unwrap();
            assert_eq!(dependency_weights.len(), 1);
            assert_eq!(dependency_weights[&course1_id], 1.0);
        }

        {
            assert!(graph.get_dependents(course4_id).is_none());
            assert!(graph.get_dependent_weights(course4_id).is_none());

            let dependencies = graph.get_dependencies(course4_id).unwrap();
            assert_eq!(dependencies.len(), 1);
            assert!(dependencies.contains(&course2_id));
            let dependency_weights = graph.get_dependency_weights(course4_id).unwrap();
            assert_eq!(dependency_weights.len(), 1);
            assert_eq!(dependency_weights[&course2_id], 1.0);
        }

        {
            assert!(graph.get_dependents(course5_id).is_none());
            assert!(graph.get_dependent_weights(course5_id).is_none());

            let dependencies = graph.get_dependencies(course5_id).unwrap();
            assert_eq!(dependencies.len(), 1);
            assert!(dependencies.contains(&course3_id));
            let dependency_weights = graph.get_dependency_weights(course5_id).unwrap();
            assert_eq!(dependency_weights.len(), 1);
            assert_eq!(dependency_weights[&course3_id], 1.0);
        }

        let sinks = graph.get_dependency_sinks();
        assert_eq!(sinks.len(), 1);
        assert!(sinks.contains(&course1_id));

        graph.check_cycles()?;
        Ok(())
    }
    /// Verifies the length of the weights is checked against the dependencies.
    #[test]
    fn unmatched_weights() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        let course1_id = Ustr::from("course1");
        let course2_id = Ustr::from("course2");
        graph.add_course(course1_id)?;
        graph.add_course(course2_id)?;
        assert!(graph
            .add_dependencies(course2_id, UnitType::Course, &[course1_id], &[1.0, 0.5])
            .is_err());
        Ok(())
    }

    /// Verifies generating a DOT graph.
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

        graph.add_lesson(course1_lesson1_id, course1_id)?;
        graph.add_lesson(course1_lesson2_id, course1_id)?;
        graph.add_lesson(course2_lesson1_id, course2_id)?;
        graph.add_lesson(course3_lesson1_id, course3_id)?;
        graph.add_lesson(course3_lesson2_id, course3_id)?;

        graph.add_dependencies(course1_id, UnitType::Course, &[], &[])?;
        graph.add_dependencies(
            course1_lesson2_id,
            UnitType::Lesson,
            &[course1_lesson1_id],
            &[1.0],
        )?;
        graph.add_dependencies(course2_id, UnitType::Course, &[course1_id], &[1.0])?;
        graph.add_dependencies(course3_id, UnitType::Course, &[course2_id], &[1.0])?;
        graph.add_dependencies(
            course3_lesson2_id,
            UnitType::Lesson,
            &[course3_lesson1_id],
            &[1.0],
        )?;
        graph.update_starting_lessons();

        let dot = graph.generate_dot_graph();
        let expected = indoc! {r#"
            digraph dependent_graph {
                "1" [color=red, style=filled]
                "1" -> "1::1"
                "1" -> "2"
                "1::1" [color=blue, style=filled]
                "1::1" -> "1::2"
                "1::2" [color=blue, style=filled]
                "2" [color=red, style=filled]
                "2" -> "2::1"
                "2" -> "3"
                "2::1" [color=blue, style=filled]
                "3" [color=red, style=filled]
                "3" -> "3::1"
                "3::1" [color=blue, style=filled]
                "3::1" -> "3::2"
                "3::2" [color=blue, style=filled]
            }
    "#};
        assert_eq!(dot, expected);
        Ok(())
    }

    #[test]
    fn duplicate_ids() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();

        let course_id = Ustr::from("course_id");
        graph.add_course(course_id)?;
        let _ = graph.add_course(course_id).is_err();

        let lesson_id = Ustr::from("lesson_id");
        graph.add_lesson(lesson_id, course_id)?;
        let _ = graph.add_lesson(lesson_id, course_id).is_err();

        let exercise_id = Ustr::from("exercise_id");
        graph.add_exercise(exercise_id, lesson_id)?;
        let _ = graph.add_exercise(exercise_id, lesson_id).is_err();

        Ok(())
    }

    #[test]
    fn update_unit_type_different_types() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        let unit_id = Ustr::from("unit_id");
        graph.update_unit_type(unit_id, UnitType::Course)?;
        assert!(graph.update_unit_type(unit_id, UnitType::Lesson).is_err());
        Ok(())
    }

    /// Verifies that a cycle in the dependencies is detected and causes an error.
    #[test]
    fn dependencies_cycle() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        let course1_id = Ustr::from("course1");
        let course2_id = Ustr::from("course2");
        let course3_id = Ustr::from("course3");
        let course4_id = Ustr::from("course4");
        let course5_id = Ustr::from("course5");
        graph.add_course(course1_id)?;
        graph.add_course(course2_id)?;
        graph.add_course(course3_id)?;
        graph.add_course(course4_id)?;
        graph.add_course(course5_id)?;
        graph.add_dependencies(course1_id, UnitType::Course, &[], &[])?;
        graph.add_dependencies(course2_id, UnitType::Course, &[course1_id], &[1.0])?;
        graph.add_dependencies(course3_id, UnitType::Course, &[course1_id], &[1.0])?;
        graph.add_dependencies(course4_id, UnitType::Course, &[course2_id], &[1.0])?;
        graph.add_dependencies(course5_id, UnitType::Course, &[course3_id], &[1.0])?;

        // Add a cycle, which should be detected when calling `check_cycles`.
        graph.add_dependencies(course1_id, UnitType::Course, &[course5_id], &[1.0])?;
        assert!(graph.check_cycles().is_err());

        Ok(())
    }

    /// Verifies that a cycle in the superseded graph is detected and causes an error.
    #[test]
    fn superseded_cycle() {
        // Add a cycle, which should be detected when calling `check_cycles`.
        let mut graph = InMemoryUnitGraph::default();
        let course1_id = Ustr::from("course1");
        let course2_id = Ustr::from("course2");
        let course3_id = Ustr::from("course3");
        let course4_id = Ustr::from("course4");
        let course5_id = Ustr::from("course5");
        graph.add_superseded(course2_id, &[course1_id]);
        graph.add_superseded(course3_id, &[course1_id]);
        graph.add_superseded(course4_id, &[course2_id]);
        graph.add_superseded(course5_id, &[course3_id]);
        graph.add_superseded(course1_id, &[course5_id]);
        assert!(graph.check_cycles().is_err());
    }

    /// Verifies that the cycle check fails if a dependent relationship is missing.
    #[test]
    fn missing_dependent_relationship() -> Result<()> {
        let mut graph = InMemoryUnitGraph::default();
        let course_id = Ustr::from("course_id");
        let lesson1_id = Ustr::from("lesson1_id");
        let lesson2_id = Ustr::from("lesson2_id");
        graph.add_course(course_id).unwrap();
        graph.add_lesson(lesson1_id, course_id).unwrap();
        graph.add_lesson(lesson2_id, course_id).unwrap();
        graph.add_dependencies(lesson2_id, UnitType::Lesson, &[lesson1_id], &[1.0])?;

        // Manually remove the dependent relationship to trigger the check and make the cycle
        // detection fail.
        graph.dependent_graph.insert(lesson1_id, UstrSet::default());
        assert!(graph.check_cycles().is_err());
        // Also check that the check fails if the dependents value is `None`.
        graph.dependency_graph.remove(&lesson1_id);
        assert!(graph.check_cycles().is_err());
        Ok(())
    }

    /// Verifies that the cycle check fails if a superseding relationship is missing.
    #[test]
    fn missing_superseding_relationship() {
        let mut graph = InMemoryUnitGraph::default();
        let lesson1_id = Ustr::from("lesson1_id");
        let lesson2_id = Ustr::from("lesson2_id");
        graph.add_superseded(lesson2_id, &[lesson1_id]);

        // Manually remove the superseding relationship to trigger the check and make the cycle
        // detection fail.
        graph
            .superseding_graph
            .insert(lesson1_id, UstrSet::default());
        assert!(graph.check_cycles().is_err());
        // Also check that the check fails if the superseding value is `None`.
        graph.dependency_graph.remove(&lesson1_id);
        assert!(graph.check_cycles().is_err());
    }
}
