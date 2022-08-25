//! Defines the dependency graph of units of knowledge, their dependency relationships, and basic
//! read and write operations.
//!
//! The dependency graph is perharps the most important part of the design of Trane so its nature
//! and purpose should be well documented. At its core, the goal of Trane is to guide students
//! through the graph of units of knowledge composed of exercises, by having each successive unit
//! teach a skill that can be acquired once the source unit is sufficiently mastered. This process
//! of repetition of mastered exercises and introduction of new ones should lead to the complete
//! mastery of complex meta-skills such as jazz improvisation, chess, piano, etc. that are in fact
//! the mastered integration of many smaller and interlinked skills.
//!
//! This graph is implemented by simulating a directed acyclic graph (DAG) of units and
//! dependency/dependents relationships among them. A unit can be of three types:
//! 1. An excercise, which represents a single task testing a skill which the student is required to
//!    assess when practiced.
//! 2. A lesson, which represents a collection of exercises which test the same skill and can be
//!    practiced in any order.
//! 3. A course, a collection of lessons which are related. It mostly exists to help organize the
//!    material in larger entities which share some context xfasdfasdfasfd.
//!
//! The relationships between the units can be of two types:
//! 1. A course or lesson A is a dependency of course or lesson B if A needs to be sufficiently
//!    mastered before B can be practiced.
//! 2. The reverse relationship. Thus, we say that B is a dependent of A.
//!
//! The graph also provides a number of operations to manipulate the graph, which are only used when
//! reading the Trane library (see course_library.rs), and another few to derive information from
//! the graph ("which are the lessons in a course?" for example). The graph is not in any way
//! responsible on how the exercises are scheduled (see scheduler.rs for information on that) nor it
//! stores any information about a student's practice.

#[cfg(test)]
mod tests;

use std::fmt::Write;

use anyhow::{anyhow, ensure, Result};
use ustr::{Ustr, UstrMap, UstrSet};

use crate::data::UnitType;

/// Stores the units and their dependency relationships (for lessons and courses only). It provides
/// basic functions to update the graph and retrieve information about it for use during scheduling
/// and student's requests.
///
/// The write operations are only used right now when reading the Trane library for the first time.
/// A user that copies new courses to an existing and currently opened library will need to restart
/// the interface for Trane. That limitation might change in the future, but it's not a high priority
/// as the process takes only a few seconds.
pub trait UnitGraph {
    /// Adds a new course to the unit graph. This function will return an error if this function is
    /// not called before the dependencies of this course are added. This is done to properly check
    /// that unit IDs are unique.
    fn add_course(&mut self, course_id: &Ustr) -> Result<()>;

    /// Adds a new lesson to the unit graph. This function is the equivalent of add_course for
    /// lessons. It also requires the ID of the course to which this lesson belongs.
    fn add_lesson(&mut self, lesson_id: &Ustr, course_id: &Ustr) -> Result<()>;

    /// Adds a new exercise to the unit graph. This function is the equivalent of add_course and
    /// add_lesson for exercises. It also requires the ID of the lesson to which this exercise
    /// belongs.
    fn add_exercise(&mut self, exercise_id: &Ustr, lesson_id: &Ustr) -> Result<()>;

    /// Takes a unit and its dependencies and updates the graph accordingly. Returns an error if
    /// unit_type is `UnitType::Exercise` as only courses and lessons are allowed to have
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

    /// Updates the starting lessons for all courses. The starting lessons of course C are those
    /// lessons in C that should be practiced first when C is introduced to the student. The
    /// scheduler uses them to traverse through the other lessons in the course in the correct
    /// order.
    fn update_starting_lessons(&mut self);

    /// Returns the starting lessons for the given course.
    fn get_course_starting_lessons(&self, course_id: &Ustr) -> Option<UstrSet>;

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

    /// Returns the dependency sinks of the graph. A dependency sink are the courses from which a
    /// walk of the entire unit graph needs to start. Because the lessons in a course implicitly
    /// depend on the course, a correct implementation only returns courses.
    fn get_dependency_sinks(&self) -> UstrSet;

    /// Performs a cycle check on the graph, done currently when opening the Trane library.
    fn check_cycles(&self) -> Result<()>;

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

/// Implements the UnitGraph by describing the units and relationships as an adjacency list stored
/// in hash maps. All of it is stored in memory, as the memory benchmarks say that less than 20 MB
/// of memory are used even when opening a large Trane library.
#[derive(Default)]
pub(crate) struct InMemoryUnitGraph {
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

    /// The mappinng of a unit to all its dependents.
    dependent_graph: UstrMap<UstrSet>,

    /// The set of all dependency sinks in the graph.
    dependency_sinks: UstrSet,
}

/// An implementation of the UnitGraph trait which stores the graph in memory.
impl InMemoryUnitGraph {
    /// Updates the dependency sinks of the given unit when the given unit and dependencies are
    /// added to the graph. If it's called
    fn update_dependency_sinks(&mut self, unit_id: &Ustr, dependencies: &[Ustr]) {
        let empty = UstrSet::default();
        let current_dependencies = self.dependency_graph.get(unit_id).unwrap_or(&empty);
        if current_dependencies.is_empty() && dependencies.is_empty() {
            self.dependency_sinks.insert(*unit_id);
        } else {
            self.dependency_sinks.remove(unit_id);
        }

        // If a course is mentioned as a dependency, but it's missing, it should be a dependency
        // sink. To ensure this requirement, the function is called recursively on all the
        // dependents with a dependency list. It's safe to do this for all courses because a call
        // to this function for a course with an empty dependency list followed by another with the
        // actual list has the same result as only executing the second call but makes sure that any
        // missing courses are added and never removed from the dependency sinks.
        for dependency_id in dependencies {
            self.update_dependency_sinks(dependency_id, &[]);
        }
    }

    /// Updates the type of the given unit. Returns an error if the unit already had a type, and
    /// it's different from the type provided in the function call.
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
        self.dependency_graph
            .entry(*unit_id)
            .or_insert_with(UstrSet::default)
            .extend(dependencies);
        for dependency_id in dependencies {
            self.dependent_graph
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
                    // The lesson is a starting lesson if the set of lessons in the course and the
                    // dependencies of the lesson are disjoint.
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
        self.dependent_graph.get(unit_id).cloned()
    }

    fn get_dependency_sinks(&self) -> UstrSet {
        self.dependency_sinks.clone()
    }

    fn check_cycles(&self) -> Result<()> {
        // Perform a depth-first search of the dependency graph from each unit. Return an error if
        // the same unit is encountered twice during the search.
        let mut visited = UstrSet::default();
        for unit_id in self.dependency_graph.keys() {
            if visited.contains(unit_id) {
                continue;
            }

            // The stacks store a path of traversed units and is initialized with `unit_id`.
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
                            // Verify that the dependency and dependent graphs agree with each other
                            // by checking that all the dependencies of the current unit list it as
                            // a dependent.
                            if !dependents.contains(&current_id) {
                                return Err(anyhow!(
                                    "unit {} lists unit {} as a dependency but the reverse \
                                    relationship does not exist",
                                    current_id,
                                    dependency_id
                                ));
                            }
                        }

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
        // Initialize the output with the first line of the file.
        let mut output = String::from("digraph dependent_graph {\n");
        let mut courses = self.course_lesson_map.keys().cloned().collect::<Vec<_>>();
        courses.sort();

        for course_id in courses {
            // Write the entry in the graph for all the of the dependents of this course.
            let mut dependents = self
                .get_dependents(&course_id)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();

            // A course's lessons are attached to the graph by making the starting lessons a
            // dependent of the course. This is not exactly accurate, but properly adding them to
            // the graph would require each course to have two nodes, one inbound, connected to the
            // starting lessons, and one outbound, connected to the last lessons in the course (by
            // the order in which they must be traversed to master the entire course) and to the
            // dependents of the course. This might eventually be amended, either here in this
            // function or in the implementation of the graph itself.
            dependents.extend(
                self.get_course_starting_lessons(&course_id)
                    .unwrap_or_default()
                    .iter(),
            );
            dependents.sort();

            for dependent in dependents {
                let _ = writeln!(output, "    \"{}\" -> \"{}\"", course_id, dependent);
            }

            // Repeat the same process for each lesson in this course.
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
