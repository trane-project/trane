# Trane

[![Latest Version](https://img.shields.io/crates/v/trane)](https://crates.io/crates/trane)
[![docs.rs](https://img.shields.io/docsrs/trane)](https://docs.rs/trane)
[![Stars](https://img.shields.io/github/stars/trane-project/trane?style=social)]
(https://github.com/trane-project/trane/stargazers)

## TLDR

Trane is an automated learning system for acquiring complex skills. Think of it like a system to
progress through a video game character's skill tree while making sure all of your previously
mastered skills are kept up to date, but applied to arbitrary skills.

If you want to see it in action, head over to
[trane-cli](https://github.com/trane-project/trane-cli) for the command-line interface and to
[trane-music](https://github.com/trane-project/trane-music) for some music courses you can use to
test it out.

## Documentation

Latest documentation can be found at the [official site](https://trane-project.github.io/)

## Introduction

Trane is an automated learning system for the acquisition of complex and highly hierarchical skills.
It is based on the principles of spaced repetition, mastery learning, and chunking.

Given a set of exercises which have been bundled into lessons and further bundled in courses, as
well as the dependency relationships between those lessons and courses, Trane selects exercises to
present to the user. It makes sure that exercises from a course or lesson are not presented until
the exercises in their dependencies have been sufficiently mastered. It also tries to keep the
difficulty of the exercises balanced, so that the selected exercises lie slightly outside the user's
current abilities.

You can think of this process as progressing through the skill tree of a character in a video game,
but applied to arbitrary skills, specified in plain-text files which are easy to share and augment.

Trane is named after John Coltrane, whose nickname Trane was often used in wordplay with the word
train (as in the vehicle) to describe the overwhelming power of his playing. It is used here as a
play on its homophone (as in "*trane* a new skill").

## Motivation

Trane was conceived after my frustration trying to learn music in general and jazz improvisation in
particular (another reason for its name). While I practiced most days, I didn't feel I was making a
lot of progress. While I made progress in whatever I practiced at the time, other skills and
previously learned songs were in a constant process of being unlearned and forgotten.

I wanted a system that would keep track of all the skill and exercises I needed to practice, letting
me know when my skills were deteriorating, and asking me to practice them. I also wanted to move on
to practice the next set of skills once my current skills were sufficiently mastered.

Initially, I tried to use Anki to help me with these tasks, but quickly found limitations (See Q&A
section below).

## Principles

### Spaced Repetition

Spaced repetition is a long-established way to efficiently memorize new information and to transfer
that information to long-term memory. Trane applies spaced repetition to exercises that require
memorization (e.g. recalling the notes in the chord A7) and to those which require mastery of an
action (e.g. playing a section of a song). How well spaced repetition works for the second type of
question is still unknown, but for exercises that require the simple repetition of the same task
until it is mastered (which covers most of musical training) it should work well.

The space repetition algorithm is fairly simple and relies on computing a score for a given exercise
based on previous trials rather than computing the optimal time at which the exercise needs to be
presented again. This will most likely result in exercises being presented more often than they
would in other spaced repetition software. Trane is not focused on memorization but on the
repetition of individual skills until they are mastered, so I do not believe this to be a problem.

There might be a major revamp of the algorithm once I get enough feedback, both from my own
experience and others'. Until now, I suspect that a majority of learning gains will come about from
selecting exercises based on some average of their previous scores.

### Mastery Learning & Chunking

These two concepts are highly related. Mastery learning states that students must achieve a level of
mastery in a skill before moving on to learning the skills which depend on the current skill (in
Trane these skills are called the dependencies and dependents of a unit). Chunking consists of
breaking up a complex skill into smaller components that can be practiced independently.

Trane applies mastery learning by preventing the user from moving on to the dependents of a unit
until the material in the unit is sufficiently mastered. It also excludes units whose dependencies
have not been fully met. Otherwise, a user might be presented with material that lies too outside
their current abilities and become frustrated. If a user's performance on a previously mastered unit
degrades, Trane will make the user practice the material until it is mastered again.

Trane applies chunking by allowing users to define lessons and courses with arbitrary dependency
relationships. For example, learning to improvise over chord progressions might be broken into units
to learn the notes in each chord, learn the fingerings of each chord, or improvise over single
chords. The user can then define a unit that exercises the union of all the previous skills and
claim the other lessons as a dependency.

### Units Defined in Text Files

A common theme in the spaced repetition literature is that users should create their own flashcards
to better help them memorize their material. While that advice might be useful for spaced repetition
for the sole purpose of memorization, it is not so useful when creating material for Trane.

Given that Trane requires knowing the dependencies among lessons and courses to be effective,
beginners will be at a disadvantage because they will most likely not be aware of those
relationships. Defining all the materials in a plain-text format lets users freely and easily share
their courses and lessons. It also allows the creators of that material to generate the necessary
files programmatically and to extend Trane to support new types of exercises.

Trane comes with utilities for the purpose of facilitating the creation of new courses. For example,
it provides a course builder that follows the circle of fifths and creates a lesson based on the key
and the one that came before in the circle. For example, one can use this builder to generate a
course on the major scale that begins by teaching the C Major scale, followed by the F Major and G
Major scale (the scales with one flat and sharp respectively) and so on.

## Basic Concepts

This section defines basic concepts used in Trane, both for using it and for creating new material.

### Mastery Score.

When presented an exercise, a user performs it and assigns it a score signifying their mastery of
the task. The scores range from one to five, with one meaning the skill is just being introduced
(e.g. reading a section of a music score and figuring out the notes and movements required to play
it) and five meaning complete mastery of the material (e.g. effortlessly playing the section and
improvising on it).

### Units

There are three types of units in Trane:

- Exercise: An exercise is just a task that needs to be performed and assigned a score.
- Lesson: A set of related exercises, which ideally follow the same format.
- Course: A set of lessons on a related topic.

A Trane library is a set of courses stored under the same directory. Trane stores its configuration
under a directory called `.trane` in that directory. Users might want to have multiple separate
libraries if they are learning separate skills (e.g. music and chess), and they want to keep their
practice separate.

Units are defined in JSON files called manifests, which are serialized versions of structs defined
in the data module. The ID, name, description, locations of any external files (e.g. the files
storing the front and back of a flashcard), etc., are defined in those files.

### Blacklist

Each Trane library has a blacklist. A unit in a blacklist can be any exercise, lesson, or course. If
a unit is in the blacklist, Trane will not show any exercises from it. If a lesson or course depend
on a blacklisted unit, the scheduling algorithm will act as if the blacklisted unit has been
mastered.

A unit should be added to the blacklist if the user already has mastered the material (e.g. an
accomplished musician will want to skip the course teaching the notes in the major scale) or if they
have no interest in learning the material (e.g. someone interested in learning the guitar might want
 to skip units which are focused on another instrument).

### Filters

In its normal mode of operation, Trane looks for exercises in the entire library. There are times
when users might want to focus on a smaller section. Filters provide users with the ability to
select specific exercises. There are three types of filters.

- Lesson filter: Only present exercises from the given lesson. For example, users might want to only
  practice exercises from a lesson covering a section of a song.
- Course filter: Only present exercises from the given course. The dependency relationships among
  the lessons in the course are respected. For example, users might want to only practice exercises
  from a course covering an entire song.
- Metadata filter: Courses and lessons can have key-value pairs as metadata. A metadata filter acts
  on this metadata to present exercises exclusively from units which match the filter, while also
  preserving the dependency relationships between those lessons. Lessons which do not pass the
  filter are considered as mastered so that the scheduler can continue the search. For example, a
  user might want to only practice exercises from lessons and courses for the guitar and in a
  specific key.

## Q&A

### What is the current state of the project?

Trane is in an early state and subject to change. However, I do not expect a lot of changes to
happen in the core scheduling logic. The only state stored by Trane depends on the ID of a unit, so
as long as that is not changed, updating the files is the only thing needed to make Trane pick up
updated versions.

### How well does Trane work?

The honest answer is that I am not sure. One of the goals of releasing Trane to the public is to get
feedback and user reports in the hope that I can fine-tune it. I suspect Trane will work fairly well
in learning skills that require the repetition of complex chains of patterns until mastery of each
and the whole is achieved. Playing music mostly follows this pattern, but I would like to figure out
how it can be applied to other skills. 

### How do I use Trane?

At the moment, there is only a command line interface for using Trane. The code and releases are in
the [trane-cli](https://github.com/trane-project/trane-cli) repository.

### How do I get content for Trane.

The repository [trane-music](https://github.com/trane-project/trane-music) contains the first
courses available that you can use to experiment with Trane. More are coming, and I am open to
contributions. I am also looking into creating courses for other skills to figure out how to apply
Trane to skills other than music. Some candidates at the moment are chess, programming, and
languages.

Since Trane courses are just collections of plain-text files, you can also create your own content.
This content can freely reference other courses, even those written by others. For example, you
could add new courses that link to a course in trane-music, or add additional exercises to one of
the lessons.

You can also experiment with augmenting existing educational materials by translating them into
Trane exercises, lessons, and courses. For example, if you are learning the flute and have a book of
études you would like to master, you could break each into a course, each large section of the piece
into a lesson, and each small subsection into an exercise. This process does not require you to port
any of the actual material into Trane. Creating flashcards that say "Play étude 4, measures 12
through 16" is enough. Used in this manner, Trane can integrate materials from multiple sources into
one centralized practice system.

### Are there plans to have a graphic interface?

Eventually. I am not too familiar with front-end or GUI development, so it could take a while.
However, the command-line version should be enough to get going for now. The main thing to be gained
from a graphic interface is to allow external resources (e.g. images or a score from soundslice.com)
to be embedded into the application.

### Why not Anki or another existing software?

Originally, I tried to use Anki for practicing music but quickly found some limitations. First, Anki
and similar software are optimized for memorization, not for practicing the same skill until it is
mastered. Most importantly, defining arbitrary dependencies between subsets of flashcards *and*
having the algorithm use those dependencies to select the flashcards to present is not supported.

The solution given by Anki is to create multiple decks. However, asking users to manually decide
which deck to practice and which decks should be practiced once the current one is sufficiently
mastered sort of defeats the purpose of using an automated system in the first place.
