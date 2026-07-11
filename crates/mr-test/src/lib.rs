use std::{
    fs,
    path::{Path, PathBuf},
};

use mr_core::CommandResult;
#[cfg(test)]
use mr_lua::pack_game_directory;
use mr_lua::{GameSource, LuaGame, package_file_names, package_file_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcript {
    pub steps: Vec<TranscriptStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptStep {
    pub command: String,
    pub expected: String,
    pub assertions: Vec<TranscriptAssertion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscriptAssertion {
    Room(String),
    Scene(Option<String>),
    Chapter(Option<String>),
    Flag(String),
    Counter { name: String, value: i64 },
    Contains(String),
    NotContains(String),
}

impl Transcript {
    pub fn empty() -> Self {
        Self { steps: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestReport {
    pub passed: usize,
    pub failed: Vec<TranscriptFailure>,
}

impl TestReport {
    pub fn is_success(&self) -> bool {
        self.failed.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscriptFailure {
    pub transcript: PathBuf,
    pub step: usize,
    pub command: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TranscriptTestOptions {
    pub filter: Option<String>,
    pub update: bool,
    pub seed: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum TranscriptError {
    #[error("failed to read '{path}': {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid transcript '{path}': {message}")]
    InvalidTranscript { path: PathBuf, message: String },

    #[error("failed to load game '{path}': {message}")]
    LoadGame { path: PathBuf, message: String },

    #[error("failed to run command '{command}' in '{path}': {message}")]
    RunCommand {
        path: PathBuf,
        command: String,
        message: String,
    },
}

pub fn run_game_tests(game_dir: impl AsRef<Path>) -> Result<TestReport, TranscriptError> {
    run_game_tests_with_options(game_dir, &TranscriptTestOptions::default())
}

pub fn run_game_tests_with_options(
    game_dir: impl AsRef<Path>,
    options: &TranscriptTestOptions,
) -> Result<TestReport, TranscriptError> {
    let game_dir = game_dir.as_ref();
    if game_dir
        .extension()
        .is_some_and(|extension| extension == "moon")
    {
        return run_package_tests_with_options(game_dir, options);
    }

    let tests_dir = game_dir.join("tests");
    let mut transcript_paths = Vec::new();

    if !tests_dir.exists() {
        return Ok(TestReport {
            passed: 0,
            failed: Vec::new(),
        });
    }

    let entries = fs::read_dir(&tests_dir).map_err(|source| TranscriptError::Io {
        path: tests_dir.clone(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| TranscriptError::Io {
            path: tests_dir.clone(),
            source,
        })?;
        let path = entry.path();

        if path
            .extension()
            .is_some_and(|extension| extension == "transcript")
            && matches_filter(game_dir, &path, options.filter.as_deref())
        {
            transcript_paths.push(path);
        }
    }

    transcript_paths.sort();

    let mut report = TestReport {
        passed: 0,
        failed: Vec::new(),
    };

    for transcript_path in transcript_paths {
        let failures = run_transcript_with_options(game_dir, &transcript_path, options)?;

        if failures.is_empty() {
            report.passed += 1;
        } else {
            report.failed.extend(failures);
        }
    }

    Ok(report)
}

fn run_package_tests_with_options(
    package_path: &Path,
    options: &TranscriptTestOptions,
) -> Result<TestReport, TranscriptError> {
    if options.update {
        return Err(TranscriptError::InvalidTranscript {
            path: package_path.to_path_buf(),
            message: "cannot update transcripts inside a .moon package; unpack it first"
                .to_string(),
        });
    }

    let mut transcript_paths = package_file_names(package_path)
        .map_err(|err| TranscriptError::LoadGame {
            path: package_path.to_path_buf(),
            message: err.to_string(),
        })?
        .into_iter()
        .filter(|path| {
            path.starts_with("tests/")
                && path.ends_with(".transcript")
                && matches_filter(Path::new(""), Path::new(path), options.filter.as_deref())
        })
        .collect::<Vec<_>>();
    transcript_paths.sort();

    let mut report = TestReport {
        passed: 0,
        failed: Vec::new(),
    };

    for transcript_path in transcript_paths {
        let text = package_file_text(package_path, &transcript_path).map_err(|err| {
            TranscriptError::Io {
                path: package_path.join(&transcript_path),
                source: std::io::Error::other(err.to_string()),
            }
        })?;
        let transcript_display = package_path.join(&transcript_path);
        let failures = run_transcript_text_with_options(
            GameSource::Package(package_path.to_path_buf()),
            &transcript_display,
            &text,
            options,
        )?;

        if failures.is_empty() {
            report.passed += 1;
        } else {
            report.failed.extend(failures);
        }
    }

    Ok(report)
}

pub fn run_transcript(
    game_dir: &Path,
    transcript_path: &Path,
) -> Result<Vec<TranscriptFailure>, TranscriptError> {
    run_transcript_with_options(game_dir, transcript_path, &TranscriptTestOptions::default())
}

pub fn run_transcript_with_options(
    game_dir: &Path,
    transcript_path: &Path,
    options: &TranscriptTestOptions,
) -> Result<Vec<TranscriptFailure>, TranscriptError> {
    let text = fs::read_to_string(transcript_path).map_err(|source| TranscriptError::Io {
        path: transcript_path.to_path_buf(),
        source,
    })?;
    let failures = run_transcript_text_with_options(
        GameSource::Directory(game_dir.to_path_buf()),
        transcript_path,
        &text,
        options,
    )?;

    Ok(failures)
}

fn run_transcript_text_with_options(
    source: GameSource,
    transcript_path: &Path,
    text: &str,
    options: &TranscriptTestOptions,
) -> Result<Vec<TranscriptFailure>, TranscriptError> {
    let transcript = parse_transcript(transcript_path, text)?;
    let mut game = LuaGame::load_source(source).map_err(|err| TranscriptError::LoadGame {
        path: transcript_path.to_path_buf(),
        message: err.to_string(),
    })?;
    let mut failures = Vec::new();
    let mut updated_steps = Vec::new();

    if let Some(seed) = options.seed {
        game.set_random_seed(seed);
    }

    for (step_index, step) in transcript.steps.into_iter().enumerate() {
        let result =
            game.handle_command(&step.command)
                .map_err(|err| TranscriptError::RunCommand {
                    path: transcript_path.to_path_buf(),
                    command: step.command.clone(),
                    message: err.to_string(),
                })?;
        let actual = match result {
            CommandResult::Continue(outcome) => outcome.output,
            CommandResult::Quit(output) => output,
        };

        if !options.update && normalize_output(&actual) != normalize_output(&step.expected) {
            failures.push(TranscriptFailure {
                transcript: transcript_path.to_path_buf(),
                step: step_index + 1,
                command: step.command.clone(),
                expected: step.expected,
                actual: actual.clone(),
            });
        }

        for assertion in &step.assertions {
            if let Some((expected, actual)) = check_assertion(&game, &actual, assertion) {
                failures.push(TranscriptFailure {
                    transcript: transcript_path.to_path_buf(),
                    step: step_index + 1,
                    command: step.command.clone(),
                    expected,
                    actual,
                });
            }
        }

        if options.update {
            updated_steps.push(TranscriptStep {
                command: step.command,
                expected: normalize_output(&actual),
                assertions: step.assertions,
            });
        }
    }

    if options.update && failures.is_empty() {
        let rendered = render_transcript(&Transcript {
            steps: updated_steps,
        });
        fs::write(transcript_path, rendered).map_err(|source| TranscriptError::Io {
            path: transcript_path.to_path_buf(),
            source,
        })?;
    }

    Ok(failures)
}

pub fn load_transcript(path: impl AsRef<Path>) -> Result<Transcript, TranscriptError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| TranscriptError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    parse_transcript(path, &text)
}

pub fn parse_transcript(path: &Path, text: &str) -> Result<Transcript, TranscriptError> {
    let mut transcript = Transcript::empty();
    let mut current_command = None::<String>;
    let mut current_expected = Vec::<String>::new();

    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');

        if let Some(command) = line.strip_prefix("> ") {
            if let Some(previous_command) = current_command.replace(command.to_string()) {
                transcript
                    .steps
                    .push(parse_step(path, previous_command, &current_expected)?);
                current_expected.clear();
            }
        } else if current_command.is_some() {
            current_expected.push(line.to_string());
        } else if !line.trim().is_empty() {
            return Err(TranscriptError::InvalidTranscript {
                path: path.to_path_buf(),
                message: "content before first command".to_string(),
            });
        }
    }

    if let Some(command) = current_command {
        transcript
            .steps
            .push(parse_step(path, command, &current_expected)?);
    }

    Ok(transcript)
}

fn parse_step(
    path: &Path,
    command: String,
    lines: &[String],
) -> Result<TranscriptStep, TranscriptError> {
    let mut expected = Vec::new();
    let mut assertions = Vec::new();

    for line in lines {
        let trimmed = line.trim();

        if let Some(directive) = trimmed.strip_prefix('!') {
            assertions.push(parse_assertion(path, directive)?);
        } else {
            expected.push(line.clone());
        }
    }

    Ok(TranscriptStep {
        command,
        expected: trim_trailing_blank_lines(&expected).join("\n"),
        assertions,
    })
}

fn parse_assertion(path: &Path, directive: &str) -> Result<TranscriptAssertion, TranscriptError> {
    let mut parts = directive.split_whitespace();
    let Some(kind) = parts.next() else {
        return Err(TranscriptError::InvalidTranscript {
            path: path.to_path_buf(),
            message: "empty transcript directive".to_string(),
        });
    };

    match kind {
        "room" => {
            let room_id = parts.collect::<Vec<_>>().join(" ");
            if room_id.is_empty() {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!room requires a room id".to_string(),
                });
            }

            Ok(TranscriptAssertion::Room(room_id))
        }
        "flag" => {
            let flag = parts.collect::<Vec<_>>().join(" ");
            if flag.is_empty() {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!flag requires a flag name".to_string(),
                });
            }

            Ok(TranscriptAssertion::Flag(flag))
        }
        "scene" => {
            let scene = parts.collect::<Vec<_>>().join(" ");
            if scene.is_empty() {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!scene requires a scene name or none".to_string(),
                });
            }

            Ok(TranscriptAssertion::Scene(
                (scene != "none").then_some(scene),
            ))
        }
        "chapter" => {
            let chapter = parts.collect::<Vec<_>>().join(" ");
            if chapter.is_empty() {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!chapter requires a chapter name or none".to_string(),
                });
            }

            Ok(TranscriptAssertion::Chapter(
                (chapter != "none").then_some(chapter),
            ))
        }
        "counter" => {
            let Some(name) = parts.next() else {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!counter requires a counter name and value".to_string(),
                });
            };
            let Some(value) = parts.next() else {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!counter requires a counter name and value".to_string(),
                });
            };

            if parts.next().is_some() {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!counter accepts exactly a counter name and integer value"
                        .to_string(),
                });
            }

            let value = value
                .parse::<i64>()
                .map_err(|_| TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!counter value must be an integer".to_string(),
                })?;

            Ok(TranscriptAssertion::Counter {
                name: name.to_string(),
                value,
            })
        }
        "contains" => {
            let text = parts.collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!contains requires text".to_string(),
                });
            }

            Ok(TranscriptAssertion::Contains(text))
        }
        "not_contains" => {
            let text = parts.collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return Err(TranscriptError::InvalidTranscript {
                    path: path.to_path_buf(),
                    message: "!not_contains requires text".to_string(),
                });
            }

            Ok(TranscriptAssertion::NotContains(text))
        }
        _ => Err(TranscriptError::InvalidTranscript {
            path: path.to_path_buf(),
            message: format!("unknown transcript directive '!{kind}'"),
        }),
    }
}

fn check_assertion(
    game: &LuaGame,
    output: &str,
    assertion: &TranscriptAssertion,
) -> Option<(String, String)> {
    match assertion {
        TranscriptAssertion::Room(room_id) => {
            let actual = game.current_room_id();
            (actual != room_id).then(|| {
                (
                    format!("!room {room_id}"),
                    format!("!room {}", game.current_room_id()),
                )
            })
        }
        TranscriptAssertion::Flag(flag) => (!game.has_flag(flag)).then(|| {
            (
                format!("!flag {flag}"),
                format!("flag '{flag}' was not set"),
            )
        }),
        TranscriptAssertion::Scene(scene) => {
            let actual = game.current_scene().map(str::to_string);
            (actual != *scene).then(|| {
                (
                    format!("!scene {}", optional_assertion_value(scene.as_deref())),
                    format!("!scene {}", optional_assertion_value(actual.as_deref())),
                )
            })
        }
        TranscriptAssertion::Chapter(chapter) => {
            let actual = game.current_chapter().map(str::to_string);
            (actual != *chapter).then(|| {
                (
                    format!("!chapter {}", optional_assertion_value(chapter.as_deref())),
                    format!("!chapter {}", optional_assertion_value(actual.as_deref())),
                )
            })
        }
        TranscriptAssertion::Counter { name, value } => {
            let actual = game.counter(name);
            (actual != *value).then(|| {
                (
                    format!("!counter {name} {value}"),
                    format!("!counter {name} {actual}"),
                )
            })
        }
        TranscriptAssertion::Contains(text) => (!output.contains(text)).then(|| {
            (
                format!("!contains {text}"),
                "command output did not contain expected text".to_string(),
            )
        }),
        TranscriptAssertion::NotContains(text) => output.contains(text).then(|| {
            (
                format!("!not_contains {text}"),
                "command output contained forbidden text".to_string(),
            )
        }),
    }
}

fn optional_assertion_value(value: Option<&str>) -> &str {
    value.unwrap_or("none")
}

fn matches_filter(game_dir: &Path, path: &Path, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };

    let relative = path.strip_prefix(game_dir).unwrap_or(path);
    relative.to_string_lossy().contains(filter)
}

fn render_transcript(transcript: &Transcript) -> String {
    let mut rendered = String::new();

    for (index, step) in transcript.steps.iter().enumerate() {
        if index > 0 {
            rendered.push('\n');
        }

        rendered.push_str("> ");
        rendered.push_str(&step.command);
        rendered.push('\n');

        if !step.expected.is_empty() {
            rendered.push_str(&step.expected);
            rendered.push('\n');
        }

        for assertion in &step.assertions {
            rendered.push_str(&render_assertion(assertion));
            rendered.push('\n');
        }
    }

    rendered
}

fn render_assertion(assertion: &TranscriptAssertion) -> String {
    match assertion {
        TranscriptAssertion::Room(room_id) => format!("!room {room_id}"),
        TranscriptAssertion::Scene(scene) => {
            format!("!scene {}", optional_assertion_value(scene.as_deref()))
        }
        TranscriptAssertion::Chapter(chapter) => {
            format!("!chapter {}", optional_assertion_value(chapter.as_deref()))
        }
        TranscriptAssertion::Flag(flag) => format!("!flag {flag}"),
        TranscriptAssertion::Counter { name, value } => format!("!counter {name} {value}"),
        TranscriptAssertion::Contains(text) => format!("!contains {text}"),
        TranscriptAssertion::NotContains(text) => format!("!not_contains {text}"),
    }
}

fn trim_trailing_blank_lines(lines: &[String]) -> Vec<String> {
    let mut end = lines.len();

    while end > 0 && lines[end - 1].trim().is_empty() {
        end -= 1;
    }

    lines[..end].to_vec()
}

fn normalize_output(output: &str) -> String {
    output.trim_end().replace("\r\n", "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_command_blocks() {
        let transcript = parse_transcript(
            Path::new("example.transcript"),
            r#"> look
Room

Description.

> take coin
Taken.
"#,
        )
        .expect("transcript should parse");

        assert_eq!(transcript.steps.len(), 2);
        assert_eq!(transcript.steps[0].command, "look");
        assert_eq!(transcript.steps[0].expected, "Room\n\nDescription.");
        assert!(transcript.steps[0].assertions.is_empty());
        assert_eq!(transcript.steps[1].command, "take coin");
        assert_eq!(transcript.steps[1].expected, "Taken.");
    }

    #[test]
    fn parses_state_assertion_directives() {
        let transcript = parse_transcript(
            Path::new("example.transcript"),
            r#"> take coin
Taken.
!room garden
!scene opening
!chapter one
!flag took_coin
!counter visits 2
!contains Taken
!not_contains dropped
"#,
        )
        .expect("transcript should parse");

        assert_eq!(transcript.steps.len(), 1);
        assert_eq!(transcript.steps[0].expected, "Taken.");
        assert_eq!(
            transcript.steps[0].assertions,
            vec![
                TranscriptAssertion::Room("garden".to_string()),
                TranscriptAssertion::Scene(Some("opening".to_string())),
                TranscriptAssertion::Chapter(Some("one".to_string())),
                TranscriptAssertion::Flag("took_coin".to_string()),
                TranscriptAssertion::Counter {
                    name: "visits".to_string(),
                    value: 2
                },
                TranscriptAssertion::Contains("Taken".to_string()),
                TranscriptAssertion::NotContains("dropped".to_string())
            ]
        );
    }

    #[test]
    fn runs_state_assertion_directives() {
        let project_dir =
            std::env::temp_dir().join(format!("moonroom-directive-test-{}", std::process::id()));
        let tests_dir = project_dir.join("tests");
        fs::create_dir_all(&tests_dir).expect("test project should be created");
        fs::write(
            project_dir.join("game.lua"),
            r#"
game {
  title = "Directive Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A start room.",
  exits = {
    north = "garden"
  }
}

room "garden" {
  name = "Garden",
  desc = "A garden."
}

thing "coin" {
  name = "coin",
  aliases = { "coin" },
  location = "start",
  portable = true,

  on_take = function(game)
    game.flag("took_coin")
    game.set_counter("coins", 1)
    game.chapter("one")
    game.start_scene("opening")
  end
}
"#,
        )
        .expect("game should be written");
        let transcript_path = tests_dir.join("directives.transcript");
        fs::write(
            &transcript_path,
            r#"> take coin
You take the coin.
!flag took_coin
!counter coins 1
!chapter one
!scene opening

> north
Garden

A garden.
!room garden
!chapter one
!scene opening
"#,
        )
        .expect("transcript should be written");

        let failures =
            run_transcript(&project_dir, &transcript_path).expect("transcript should run");

        assert!(failures.is_empty());

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn transcript_tests_cover_deterministic_ambiguity() {
        let project_dir = unique_temp_dir("moonroom-ambiguity-transcript-test");
        let tests_dir = project_dir.join("tests");
        fs::create_dir_all(&tests_dir).expect("test project should be created");
        fs::write(
            project_dir.join("game.lua"),
            r#"game { title = "Ambiguity", start = "start" }
room "start" { name = "Start", desc = "A room." }
thing "brass_key" { name = "brass key", aliases = { "key" }, location = "start", portable = true }
thing "iron_key" { name = "iron key", aliases = { "key" }, location = "start", portable = true }
"#,
        )
        .expect("game should be written");
        let transcript_path = tests_dir.join("ambiguity.transcript");
        fs::write(
            &transcript_path,
            "> take key\nWhich key do you mean: brass key or iron key?\n\n> take iron key\nYou take the iron key.\n",
        )
        .expect("transcript should be written");

        let failures =
            run_transcript(&project_dir, &transcript_path).expect("transcript should run");
        assert!(failures.is_empty());

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn filters_transcripts_by_path_text() {
        let project_dir = unique_temp_dir("moonroom-filter-test");
        let tests_dir = project_dir.join("tests");
        fs::create_dir_all(&tests_dir).expect("test project should be created");
        fs::write(
            project_dir.join("game.lua"),
            r#"
game {
  title = "Filter Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A start room."
}
"#,
        )
        .expect("game should be written");
        fs::write(
            tests_dir.join("opening.transcript"),
            r#"> look
Start

A start room.
"#,
        )
        .expect("matching transcript should be written");
        fs::write(
            tests_dir.join("ignored.transcript"),
            r#"> look
Wrong output.
"#,
        )
        .expect("ignored transcript should be written");

        let report = run_game_tests_with_options(
            &project_dir,
            &TranscriptTestOptions {
                filter: Some("opening".to_string()),
                ..TranscriptTestOptions::default()
            },
        )
        .expect("filtered tests should run");

        assert!(report.is_success());
        assert_eq!(report.passed, 1);

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn update_mode_rewrites_expected_output() {
        let project_dir = unique_temp_dir("moonroom-update-test");
        let tests_dir = project_dir.join("tests");
        fs::create_dir_all(&tests_dir).expect("test project should be created");
        fs::write(
            project_dir.join("game.lua"),
            r#"
game {
  title = "Update Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A corrected room."
}
"#,
        )
        .expect("game should be written");
        let transcript_path = tests_dir.join("opening.transcript");
        fs::write(
            &transcript_path,
            r#"> look
Outdated text.
!contains corrected
"#,
        )
        .expect("transcript should be written");

        let failures = run_transcript_with_options(
            &project_dir,
            &transcript_path,
            &TranscriptTestOptions {
                update: true,
                ..TranscriptTestOptions::default()
            },
        )
        .expect("transcript should run");
        let updated = fs::read_to_string(&transcript_path).expect("transcript should be readable");

        assert!(failures.is_empty());
        assert!(updated.contains("A corrected room."));
        assert!(updated.contains("!contains corrected"));
        assert!(!updated.contains("Outdated text."));

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn seed_override_controls_random_output() {
        let project_dir = unique_temp_dir("moonroom-seed-test");
        let tests_dir = project_dir.join("tests");
        fs::create_dir_all(&tests_dir).expect("test project should be created");
        fs::write(
            project_dir.join("game.lua"),
            r#"
game {
  title = "Seed Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A start room."
}

verb "roll" {
  aliases = { "roll" },

  on_action = function(game, input)
    game.say("Rolled " .. game.random(1, 6) .. ".")
  end
}
"#,
        )
        .expect("game should be written");
        let transcript_path = tests_dir.join("seed.transcript");
        fs::write(
            &transcript_path,
            r#"> roll
Rolled 2.
"#,
        )
        .expect("transcript should be written");

        let failures = run_transcript_with_options(
            &project_dir,
            &transcript_path,
            &TranscriptTestOptions {
                seed: Some(10),
                ..TranscriptTestOptions::default()
            },
        )
        .expect("transcript should run");

        assert!(failures.is_empty());

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn runs_transcript_tests_from_moon_package() {
        let project_dir = unique_temp_dir("moonroom-package-transcript-test");
        let package_path = project_dir.with_extension("moon");
        let tests_dir = project_dir.join("tests");
        fs::create_dir_all(&tests_dir).expect("test project should be created");
        fs::write(
            project_dir.join("game.lua"),
            r#"
game {
  title = "Package Transcript Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A packaged transcript room."
}
"#,
        )
        .expect("game should be written");
        fs::write(
            tests_dir.join("opening.transcript"),
            r#"> look
Start

A packaged transcript room.
!contains packaged
"#,
        )
        .expect("transcript should be written");

        pack_game_directory(&project_dir, &package_path).expect("package should be written");
        let report = run_game_tests(&package_path).expect("package transcript tests should run");

        assert!(report.is_success());
        assert_eq!(report.passed, 1);

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
        fs::remove_file(package_path).expect("temporary package should be removed");
    }

    #[test]
    fn rejects_content_before_first_command() {
        let err = parse_transcript(Path::new("bad.transcript"), "Room\n> look\nRoom")
            .expect_err("transcript should be rejected");

        assert!(err.to_string().contains("content before first command"));
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{prefix}-{}", std::process::id()))
    }
}
