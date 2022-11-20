# Trane

[![Github Checks Status](https://img.shields.io/github/checks-status/trane-project/trane/master)](https://github.com/trane-project/trane/actions?query=branch%3Amaster)
[![Coverage Status](https://img.shields.io/coverallsCoverage/github/trane-project/trane)](https://coveralls.io/github/trane-project/trane?branch=master)
[![docs.rs](https://img.shields.io/docsrs/trane)](https://docs.rs/trane)
[![Latest Version](https://img.shields.io/crates/v/trane)](https://crates.io/crates/trane)
[![Stars](https://img.shields.io/github/stars/trane-project/trane?style=social)](https://github.com/trane-project/trane/stargazers)

Trane is an automated practice system for the acquisition of complex and highly hierarchical skills.
It is based on the principles of spaced repetition, mastery learning, and chunking.

Given a set of exercises which have been bundled into lessons and further bundled in courses, as
well as the dependency relationships between those lessons and courses, Trane selects exercises to
present to the user. It makes sure that exercises from a course or lesson are not presented until
the exercises in their dependencies have been sufficiently mastered. It also tries to keep the
difficulty of the exercises balanced, so that most of the selected exercises lie slightly outside
the user's current abilities.

Trane is named after John Coltrane, whose nickname Trane was often used in wordplay with the word
train (as in the vehicle) to describe the overwhelming power of his playing. It is used here as a
play on its homophone (as in "*trane* a new skill").

## Quick Start

For a guide to getting started with using Trane, see the [quick
start](https://trane-project.github.io/quick_start.html) guide at the official site.

For a video showing Trane in action, see the [Tour of
Trane](https://www.youtube.com/watch?v=3ZTUBvYjWnw) video.

## Documentation

Full documentation for The Trane Project, including this library, can be found at the [official
site](https://trane-project.github.io/)

## A Code Tour of Trane

A goal of Trane's code is to be as clean, well-documented, organized, and readable as possible. Most
modules should have module-level documentation at the top of the file, which includes rationale
behind the design choices made by the author. Below is a list of a few modules and files to get you
started with understanding the code:

- `data`: Contains the basic data structures used throughout Trane. Among other things, it defines:
    - Courses, lessons, and exercises and how their content and dependencies.
    - Student scores and exercise trials.
    - The filters that can be used to narrow down the units from which exercises are drawn.
- `graph`: Contains the definition of the graph of units and their dependencies that is traversed by
  Trane as a student makes progress.
- `course_library`: Defines how a collection of courses gathered by a student is written and read
  to and from storage.
- `blacklist`: Defines the list of units which should be ignored and marked as mastered during
  exercise scheduling.
- `practice_stats`: Defines how the student's progress is stored for later used by the scheduler.
- `scorer`: Defines how an exercise is scored based on the scores and timestamps of previous trials.
- `scheduler`: Contains the logic of how exercises that are to be presented to the user are
  selected. The core of Trane's logic sits in this module.
- `review_list`: Defines a list of exercises the student wants to review at a later time.
- `filter_manager`: Defines a way to save and load filters for later use. For example, to save a
  filter to only study exercises for the guitar.
- `lib.rs`: This file defines the public API of the crate, which is the entry point for using Trane.
- `course_builder`: Defines utilities to make it easier to build Trane courses.

If there's a particular part of the code that is confusing, does not follow standard Rust idioms or
conventions, could use better documentation, or whose rationale is not obvious, feel free to open an
issue.

## Contributing

See the [CONTRIBUTING](https://github.com/trane-project/trane/blob/master/CONTRIBUTING.md) file for
more information on how to contribute to Trane.
