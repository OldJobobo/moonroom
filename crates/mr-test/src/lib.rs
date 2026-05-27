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
                command: step.command,
                expected: step.expected,
                actual,
            });
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
                transcript.steps.push(TranscriptStep {
                    command: previous_command,
                    expected: trim_trailing_blank_lines(&current_expected).join("\n"),
                });
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
        transcript.steps.push(TranscriptStep {
            command,
            expected: trim_trailing_blank_lines(&current_expected).join("\n"),
        });
    }

    Ok(transcript)
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
        assert_eq!(transcript.steps[1].command, "take coin");
        assert_eq!(transcript.steps[1].expected, "Taken.");
    }

    #[test]
    fn rejects_content_before_first_command() {
        let err = parse_transcript(Path::new("bad.transcript"), "Room\n> look\nRoom")
            .expect_err("transcript should be rejected");

        assert!(err.to_string().contains("content before first command"));
    }
}
