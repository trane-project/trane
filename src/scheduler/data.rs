//! Module which defines the data needed to run the schuduler.
use anyhow::Result;
use std::{cell::RefCell, collections::HashSet, rc::Rc};

use crate::{
    blacklist::Blacklist,
    course_library::CourseLibrary,
    data::{
        CourseManifest, ExerciseManifest, ExerciseTrial, LessonManifest, MasteryScore, UnitType,
    },
    graph::UnitGraph,
    practice_stats::PracticeStats,
};

/// A struct encapsulating all the state needed to schedule exercises.
#[derive(Clone)]
pub(crate) struct SchedulerData {
    /// The course library storing manifests and info about units.
    pub course_library: Rc<RefCell<dyn CourseLibrary>>,

    /// The dependency graph of courses and lessons.
    pub unit_graph: Rc<RefCell<dyn UnitGraph>>,

    /// The list of previous exercise results.
    pub practice_stats: Rc<RefCell<dyn PracticeStats>>,

    /// The list of units to skip during scheduling.
    pub blacklist: Rc<RefCell<dyn Blacklist>>,
}

impl CourseLibrary for SchedulerData {
    fn get_course_manifest(&self, course_id: &str) -> Option<CourseManifest> {
        self.course_library.borrow().get_course_manifest(course_id)
    }

    fn get_lesson_manifest(&self, lesson_id: &str) -> Option<LessonManifest> {
        self.course_library.borrow().get_lesson_manifest(lesson_id)
    }

    fn get_exercise_manifest(&self, exercise_id: &str) -> Option<ExerciseManifest> {
        self.course_library
            .borrow()
            .get_exercise_manifest(exercise_id)
    }
}

impl UnitGraph for SchedulerData {
    fn get_uid(&self, unit_id: &str) -> Option<u64> {
        self.unit_graph.borrow().get_uid(unit_id)
    }

    fn get_id(&self, unit_uid: u64) -> Option<String> {
        self.unit_graph.borrow().get_id(unit_uid)
    }

    fn add_lesson(&mut self, lesson_id: &str, course_id: &str) -> Result<()> {
        self.unit_graph
            .borrow_mut()
            .add_lesson(lesson_id, course_id)
    }

    fn add_exercise(&mut self, exercise_id: &str, lesson_id: &str) -> Result<()> {
        self.unit_graph
            .borrow_mut()
            .add_exercise(exercise_id, lesson_id)
    }

    fn add_dependencies(
        &mut self,
        unit_id: &str,
        unit_type: UnitType,
        dependencies: &[String],
    ) -> anyhow::Result<()> {
        self.unit_graph
            .borrow_mut()
            .add_dependencies(unit_id, unit_type, dependencies)
    }

    fn get_unit_type(&self, unit_uid: u64) -> Option<UnitType> {
        self.unit_graph.borrow().get_unit_type(unit_uid)
    }

    fn get_course_lessons(&self, course_uid: u64) -> Option<HashSet<u64>> {
        self.unit_graph.borrow().get_course_lessons(course_uid)
    }

    fn get_course_starting_lessons(&self, course_uid: u64) -> Option<HashSet<u64>> {
        self.unit_graph
            .borrow()
            .get_course_starting_lessons(course_uid)
    }

    fn get_lesson_course(&self, lesson_uid: u64) -> Option<u64> {
        self.unit_graph.borrow().get_lesson_course(lesson_uid)
    }

    fn get_lesson_exercises(&self, lesson_uid: u64) -> Option<HashSet<u64>> {
        self.unit_graph.borrow().get_lesson_exercises(lesson_uid)
    }

    fn get_dependencies(&self, unit_uid: u64) -> Option<HashSet<u64>> {
        self.unit_graph.borrow().get_dependencies(unit_uid)
    }

    fn get_dependents(&self, unit_uid: u64) -> Option<HashSet<u64>> {
        self.unit_graph.borrow().get_dependents(unit_uid)
    }

    fn get_dependency_sinks(&self) -> HashSet<u64> {
        self.unit_graph.borrow().get_dependency_sinks()
    }

    fn check_cycles(&self) -> Result<()> {
        self.unit_graph.borrow().check_cycles()
    }
}

impl PracticeStats for SchedulerData {
    fn get_scores(&self, exercise_id: &str, num_scores: usize) -> Result<Vec<ExerciseTrial>> {
        self.practice_stats
            .borrow()
            .get_scores(exercise_id, num_scores)
    }

    fn record_exercise_score(
        &self,
        exercise_id: &str,
        score: MasteryScore,
        timestamp: i64,
    ) -> Result<()> {
        self.practice_stats
            .borrow_mut()
            .record_exercise_score(exercise_id, score, timestamp)
    }
}

impl Blacklist for SchedulerData {
    fn add_unit(&mut self, unit_id: &str) -> Result<()> {
        self.blacklist.borrow_mut().add_unit(unit_id)
    }

    fn remove_unit(&mut self, unit_id: &str) -> Result<()> {
        self.blacklist.borrow_mut().remove_unit(unit_id)
    }

    fn blacklisted(&self, unit_id: &str) -> Result<bool> {
        self.blacklist.borrow().blacklisted(unit_id)
    }

    fn all_entries(&self) -> Result<Vec<String>> {
        self.blacklist.borrow().all_entries()
    }
}
