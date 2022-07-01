#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, ensure, Result};

use crate::data::UnitType;

/// Stores the dependency relationships between units. It only provides basic functions to update
/// the graph and query the outgoing or ingoing edges of a node.
pub(crate) trait UnitGraph {
    /// Retrieves the assigned uid to the given unit_id.
    fn get_uid(&self, unit_id: &str) -> Option<u64>;

    /// Retrieves the human-readable ID of the unit with the given UID.
    fn get_id(&self, unit_uid: u64) -> Option<String>;

    /// Adds a new lesson to the unit graph.
    fn add_lesson(&mut self, lesson_id: &str, course_id: &str) -> Result<()>;

    /// Adds a new exercise to the unit graph.
    fn add_exercise(&mut self, exercise_id: &str, lesson_id: &str) -> Result<()>;

    /// Takes a unit and its dependencies and updates the graph accordingly. Returns an error if
    /// unit_type is UnitType::Exercise as only courses and lessons are allowed to have
    /// dependencies.
    fn add_dependencies(
        &mut self,
        unit_id: &str,
        unit_type: UnitType,
        dependencies: &Vec<String>,
    ) -> Result<()>;

    /// Returns the type of the given unit.
    fn get_unit_type(&self, unit_uid: u64) -> Option<UnitType>;

    /// Returns the lessons belonging to the given course.
    fn get_course_lessons(&self, course_uid: u64) -> Option<HashSet<u64>>;

    /// Returns the lessons in the given course that do not depend upon any of the other lessons in
    /// the course.
    fn get_course_starting_lessons(&self, course_uid: u64) -> Option<HashSet<u64>>;

    /// Returns the course to which the given lesson belongs.
    fn get_lesson_course(&self, lesson_uid: u64) -> Option<u64>;

    /// Returns the exercises belonging to the given lesson.
    fn get_lesson_exercises(&self, lesson_uid: u64) -> Option<HashSet<u64>>;

    /// Returns the dependencies of the given unit.
    fn get_dependencies(&self, unit_uid: u64) -> Option<HashSet<u64>>;

    /// Returns all the units which depend on the given unit.
    fn get_dependents(&self, unit_uid: u64) -> Option<HashSet<u64>>;

    /// Returns the courses which have no dependencies, that is, the courses from which a walk of
    /// the unit graph can be safely started.
    fn get_dependency_sinks(&self) -> HashSet<u64>;

    /// Checks that there are no cycles in the graph.
    fn check_cycles(&self) -> Result<()>;
}

/// Subset of the UnitGraph trait which only provides the functions necessary to debug the graph.
pub trait DebugUnitGraph {
    /// Retrieves the assigned uid to the given unit_id.
    fn get_uid(&self, unit_id: &str) -> Option<u64>;

    /// Retrieves the human-readable ID of the unit with the given UID.
    fn get_id(&self, unit_uid: u64) -> Option<String>;

    /// Returns the type of the given unit.
    fn get_unit_type(&self, unit_uid: u64) -> Option<UnitType>;
}

/// Implements the UnitGraph trait based on two hash maps storing the dependency relationships.
#[derive(Default)]
pub(crate) struct InMemoryUnitGraph {
    /// The mapping of the unit's human-readable ID to its assigned u64 UID.
    uid_map: HashMap<String, u64>,

    /// The mapping of the unit's assigned u64 UID to its human-readable String ID.
    id_map: HashMap<u64, String>,

    /// The mapping of a unit to its type.
    type_map: HashMap<u64, UnitType>,

    /// The mapping of a course to its lessons.
    course_lesson_map: HashMap<u64, HashSet<u64>>,

    /// The mapping of a lesson to its course.
    lesson_course_map: HashMap<u64, u64>,

    /// The mapping of a lesson to its exercises.
    lesson_exercise_map: HashMap<u64, HashSet<u64>>,

    /// The mapping of a unit to its dependencies.
    dependency_graph: HashMap<u64, HashSet<u64>>,

    /// The mappinng of a unit to all the units which depend on it.
    reverse_graph: HashMap<u64, HashSet<u64>>,

    /// The units which have no dependencies, that is, the sinks of the dependency graph.
    dependency_sinks: HashSet<u64>,

    /// An internal counter used to generate new UIDs.
    uid_count: u64,
}

impl InMemoryUnitGraph {
    /// Retrieves the assigned uid for the given unit_id. If the unit_id has no existing mapping, a
    /// new one is created and returned.
    fn get_or_insert_uid(&mut self, unit_id: &str) -> u64 {
        let maybe_uid = self.uid_map.get(unit_id);
        match maybe_uid {
            Some(uid) => *uid,
            None => {
                self.uid_count += 1;
                self.uid_map.insert(unit_id.to_string(), self.uid_count);
                self.id_map.insert(self.uid_count, unit_id.to_string());
                self.uid_count
            }
        }
    }

    /// Updates the set of units with no dependencies.
    fn update_dependency_sinks(&mut self, unit_uid: u64, dependencies: &Vec<String>) {
        let empty = HashSet::new();
        let current_dependencies = self.dependency_graph.get(&unit_uid).unwrap_or(&empty);
        if current_dependencies.is_empty() && dependencies.is_empty() {
            self.dependency_sinks.insert(unit_uid);
        } else {
            self.dependency_sinks.remove(&unit_uid);
        }
    }

    /// Updates the type of the given unit. Returns an error if the unit already had a type and it's
    /// different than the type provided to this function.
    fn update_unit_type(&mut self, unit_uid: u64, unit_type: UnitType) -> Result<()> {
        match self.type_map.get(&unit_uid) {
            None => {
                self.type_map.insert(unit_uid, unit_type);
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
    fn get_uid(&self, unit_id: &str) -> Option<u64> {
        let uid = self.uid_map.get(&unit_id.to_string())?;
        Some(*uid)
    }

    fn get_id(&self, unit_uid: u64) -> Option<String> {
        let id = self.id_map.get(&unit_uid)?;
        Some(id.to_string())
    }

    fn add_lesson(&mut self, lesson_id: &str, course_id: &str) -> Result<()> {
        let lesson_uid = self.get_or_insert_uid(lesson_id);
        self.update_unit_type(lesson_uid, UnitType::Lesson)?;

        let course_uid = self.get_or_insert_uid(course_id);
        self.update_unit_type(course_uid, UnitType::Course)?;

        self.lesson_course_map.insert(lesson_uid, course_uid);
        self.course_lesson_map
            .entry(course_uid)
            .or_insert(HashSet::new())
            .insert(lesson_uid);
        Ok(())
    }

    fn add_exercise(&mut self, exercise_id: &str, lesson_id: &str) -> Result<()> {
        let exercise_uid = self.get_or_insert_uid(exercise_id);
        self.update_unit_type(exercise_uid, UnitType::Exercise)?;

        let lesson_uid = self.get_or_insert_uid(lesson_id);
        self.update_unit_type(lesson_uid, UnitType::Lesson)?;

        self.lesson_exercise_map
            .entry(lesson_uid)
            .or_insert(HashSet::new())
            .insert(exercise_uid);
        Ok(())
    }

    fn add_dependencies(
        &mut self,
        unit_id: &str,
        unit_type: UnitType,
        dependencies: &Vec<String>,
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

        let unit_uid = self.get_or_insert_uid(unit_id);
        self.update_unit_type(unit_uid, unit_type.clone())?;
        if unit_type == UnitType::Course || unit_type == UnitType::Lesson {
            self.update_dependency_sinks(unit_uid, dependencies);
        }

        let dependency_uids = dependencies
            .into_iter()
            .map(|d| self.get_or_insert_uid(d))
            .collect::<Vec<u64>>();

        self.dependency_graph
            .entry(unit_uid)
            .or_insert(HashSet::new())
            .extend(dependency_uids.clone());
        for dependency_uid in dependency_uids {
            self.reverse_graph
                .entry(dependency_uid)
                .or_insert(HashSet::new())
                .insert(unit_uid);
        }
        Ok(())
    }

    fn get_unit_type(&self, unit_uid: u64) -> Option<UnitType> {
        self.type_map.get(&unit_uid).cloned()
    }

    fn get_course_lessons(&self, course_uid: u64) -> Option<HashSet<u64>> {
        self.course_lesson_map.get(&course_uid).cloned()
    }

    fn get_course_starting_lessons(&self, course_uid: u64) -> Option<HashSet<u64>> {
        let lessons = self.course_lesson_map.get(&course_uid)?;

        let starting_lessons = lessons
            .iter()
            .map(|uid| *uid)
            .filter(|uid| {
                let dependencies = self.get_dependencies(*uid);
                match dependencies {
                    None => true,
                    Some(deps) => lessons.is_disjoint(&deps),
                }
            })
            .collect();
        Some(starting_lessons)
    }

    fn get_lesson_course(&self, lesson_uid: u64) -> Option<u64> {
        self.lesson_course_map.get(&lesson_uid).cloned()
    }

    fn get_lesson_exercises(&self, lesson_uid: u64) -> Option<HashSet<u64>> {
        self.lesson_exercise_map.get(&lesson_uid).cloned()
    }

    fn get_dependencies(&self, unit_uid: u64) -> Option<HashSet<u64>> {
        self.dependency_graph.get(&unit_uid).cloned()
    }

    fn get_dependents(&self, unit_uid: u64) -> Option<HashSet<u64>> {
        self.reverse_graph.get(&unit_uid).cloned()
    }

    fn get_dependency_sinks(&self) -> HashSet<u64> {
        self.dependency_sinks.clone()
    }

    fn check_cycles(&self) -> Result<()> {
        let mut visited: HashSet<u64> = HashSet::new();
        for unit_uid in self.dependency_graph.keys() {
            if visited.contains(unit_uid) {
                continue;
            }

            let mut stack: Vec<Vec<u64>> = Vec::new();
            stack.push(vec![*unit_uid]);
            while let Some(path) = stack.pop() {
                let current_id = *path.last().unwrap();
                if visited.contains(&current_id) {
                    continue;
                } else {
                    visited.insert(current_id);
                }

                let dependencies = self.get_dependencies(current_id);
                if let Some(dependencies) = dependencies {
                    for dependency_uid in dependencies {
                        // Check that the dependencies of unit_uid list it as a dependent.
                        let dependents = self.get_dependents(dependency_uid);
                        if let Some(dependents) = dependents {
                            if !dependents.contains(&current_id) {
                                return Err(anyhow!(
                                    "dependents and dependency graph do not match for unit {}",
                                    current_id,
                                ));
                            }
                        }

                        if path.contains(&dependency_uid) {
                            return Err(anyhow!("cycle in dependency graph detected"));
                        }
                        let mut new_path = path.clone();
                        new_path.push(dependency_uid);
                        stack.push(new_path);
                    }
                }
            }
        }
        Ok(())
    }
}

impl DebugUnitGraph for InMemoryUnitGraph {
    fn get_uid(&self, unit_id: &str) -> Option<u64> {
        let uid = self.uid_map.get(&unit_id.to_string())?;
        Some(*uid)
    }

    fn get_id(&self, unit_uid: u64) -> Option<String> {
        let id = self.id_map.get(&unit_uid)?;
        Some(id.to_string())
    }

    fn get_unit_type(&self, unit_uid: u64) -> Option<UnitType> {
        self.type_map.get(&unit_uid).cloned()
    }
}
