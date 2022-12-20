# Trane

[![Github Checks Status](https://img.shields.io/github/checks-status/trane-project/trane/master)](https://github.com/trane-project/trane/actions?query=branch%3Amaster)
[![Coverage Status](https://img.shields.io/coverallsCoverage/github/trane-project/trane)](https://coveralls.io/github/trane-project/trane?branch=master)
[![docs.rs](https://img.shields.io/docsrs/trane)](https://docs.rs/trane)
[![Latest Version](https://img.shields.io/crates/v/trane)](https://crates.io/crates/trane)
[![Stars](https://img.shields.io/github/stars/trane-project/trane?style=social)](https://github.com/trane-project/trane/stargazers)

Trane is an automated practice system for the acquisition of arbitrary, complex, and highly
hierarchical skills. That's quite a mouthful, so let's break it down.

- **Practice system**: Deliberate practice is at the heart of the acquisition of new skills. Trane
  calls itself a practice system because it is designed to guide student's progress through
  arbitrary skills. Trane shows the student an exercise they can practice and then asks them to
  score it based on their mastery of the skill tested by the exercise.
- **Automated**: Knowing what to practice, when to reinforce what has already been practiced, and
  when to move on to the next step is as important as establishing a consistent practice. Trane's
  main feature is to automate this process by providing students with an infinite stream of
  exercises. Internally, Trane uses the student feedback to determine which exercises are most
  appropriate for the current moment.
- **Arbitrary**: Although originally envisioned for practicing Jazz improvisation, Trane is not
  limited to a specific domain. Trane primarily works via plain-text files that are easily sharable
  and extendable. This allows student to create their own materials, to use materials created by
  others, and to seamlessly combine them. 
- **Complex and hierarchical skills**: Consider the job of a master improviser, such as the namesake
  of this software, John Coltrane. Through years of practice, Coltrane developed mastery over a
  large set of interconnected skills. A few examples include the breathing control to play the fiery
  stream of notes that characterize his style, the aural training to recognize and play in any key,
  and the fine motor skills to play the intricate lines of his solos. All these skills came together
  to create his unique and spiritually powerful sound. Trane is designed to allow students to easily
  express these complex relationships and to take advantage of them to guide the student's practice.
  This is perhaps the feature that is at the core of Trane and the main differentiation between it
  and similar software, such as Anki, which already make use of some of the same learning principles.

Trane is based on multiple proven principles of skill acquisition, such as spaced repetition,
mastery learning, and chunking. For example, Trane makes sure that not too many very easy or hard
exercises are shown to a student to avoid both extremes of frustration and boredom. Trane makes sure
to periodically reinforce skills that have already been practiced and to include new skills
automatically when the skills that they depend on have been sufficiently mastered.

If you are familiar with the experience of traversing the skill tree of a video game by grinding and
becoming better at the game, Trane aims to provide a way to help students complete a similar
process, but applied to arbitrary skills, specified in plain-text files that are easy to share and
augment.

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
