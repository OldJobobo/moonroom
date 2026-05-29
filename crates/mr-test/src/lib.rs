use std::{
    fs,
    path::{Path, PathBuf},
};

use mr_core::CommandResult;
use mr_lua::LuaGame;

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
    Flag(String),
    Counter { name: String, value: i64 },
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
    pub command: String,
    pub expected: String,
    pub actual: String,
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
    let game_dir = game_dir.as_ref();
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
        let failures = run_transcript(game_dir, &transcript_path)?;

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
    let transcript = load_transcript(transcript_path)?;
    let game_file = game_dir.join("game.lua");
    let mut game = LuaGame::load(&game_file).map_err(|err| TranscriptError::LoadGame {
        path: game_file,
        message: err.to_string(),
    })?;
    let mut failures = Vec::new();

    for step in transcript.steps {
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

        if normalize_output(&actual) != normalize_output(&step.expected) {
            failures.push(TranscriptFailure {
                transcript: transcript_path.to_path_buf(),
                command: step.command.clone(),
                expected: step.expected,
                actual,
            });
        }

        for assertion in step.assertions {
            if let Some((expected, actual)) = check_assertion(&game, &assertion) {
                failures.push(TranscriptFailure {
                    transcript: transcript_path.to_path_buf(),
                    command: step.command.clone(),
                    expected,
                    actual,
                });
            }
        }
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
        _ => Err(TranscriptError::InvalidTranscript {
            path: path.to_path_buf(),
            message: format!("unknown transcript directive '!{kind}'"),
        }),
    }
}

fn check_assertion(game: &LuaGame, assertion: &TranscriptAssertion) -> Option<(String, String)> {
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
        TranscriptAssertion::Counter { name, value } => {
            let actual = game.counter(name);
            (actual != *value).then(|| {
                (
                    format!("!counter {name} {value}"),
                    format!("!counter {name} {actual}"),
                )
            })
        }
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
!flag took_coin
!counter visits 2
"#,
        )
        .expect("transcript should parse");

        assert_eq!(transcript.steps.len(), 1);
        assert_eq!(transcript.steps[0].expected, "Taken.");
        assert_eq!(
            transcript.steps[0].assertions,
            vec![
                TranscriptAssertion::Room("garden".to_string()),
                TranscriptAssertion::Flag("took_coin".to_string()),
                TranscriptAssertion::Counter {
                    name: "visits".to_string(),
                    value: 2
                }
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

> north
Garden

A garden.
!room garden
"#,
        )
        .expect("transcript should be written");

        let failures =
            run_transcript(&project_dir, &transcript_path).expect("transcript should run");

        assert!(failures.is_empty());

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn rejects_content_before_first_command() {
        let err = parse_transcript(Path::new("bad.transcript"), "Room\n> look\nRoom")
            .expect_err("transcript should be rejected");

        assert!(err.to_string().contains("content before first command"));
    }
}
