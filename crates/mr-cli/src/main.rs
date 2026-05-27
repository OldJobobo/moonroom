use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use mr_core::CommandResult;
use mr_lua::LuaGame;
use rustyline::{DefaultEditor, error::ReadlineError};

#[derive(Debug, Parser)]
#[command(
    name = "moonroom",
    version,
    about = "Play and test Moonroom interactive fiction games."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Play {
        #[arg(value_name = "GAME_DIR")]
        game_dir: PathBuf,
    },
    Test {
        #[arg(value_name = "GAME_DIR")]
        game_dir: PathBuf,
    },
    New {
        #[arg(value_name = "NAME")]
        name: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Play { game_dir } => {
            let game_file = game_dir.join("game.lua");
            let mut game = LuaGame::load(&game_file).map_err(|err| anyhow::anyhow!("{err}"))?;
            play(&mut game)?;
        }
        Command::Test { game_dir } => {
            let report = mr_test::run_game_tests(&game_dir)?;

            if report.is_success() {
                println!("{} transcript(s) passed.", report.passed);
            } else {
                for failure in &report.failed {
                    eprintln!(
                        "{} failed on command '{}'",
                        failure.transcript.display(),
                        failure.command
                    );
                    eprintln!("expected:\n{}\n", failure.expected);
                    eprintln!("actual:\n{}\n", failure.actual);
                }

                anyhow::bail!(
                    "{} transcript(s) passed, {} command(s) failed.",
                    report.passed,
                    report.failed.len()
                );
            }
        }
        Command::New { name } => {
            let path = PathBuf::from(name);
            create_project(&path)?;
            println!("Created {}.", path.display());
            println!("Run it with:");
            println!("  moonroom play {}", path.display());
            println!("Test it with:");
            println!("  moonroom test {}", path.display());
        }
    }

    Ok(())
}

fn create_project(path: &Path) -> anyhow::Result<()> {
    if path.exists() && path.read_dir()?.next().is_some() {
        anyhow::bail!("{} already exists and is not empty", path.display());
    }

    let tests_dir = path.join("tests");
    fs::create_dir_all(&tests_dir)?;

    let title = project_title(path);
    write_new_file(&path.join("game.lua"), &template_game(&title))?;
    write_new_file(&tests_dir.join("opening.transcript"), TEMPLATE_TRANSCRIPT)?;
    write_new_file(&path.join(".luarc.json"), TEMPLATE_LUARC)?;
    write_new_file(&path.join("README.md"), &template_readme(&title))?;

    Ok(())
}

fn write_new_file(path: &Path, contents: &str) -> anyhow::Result<()> {
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }

    fs::write(path, contents)?;
    Ok(())
}

fn project_title(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Untitled Game")
        .split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn template_game(title: &str) -> String {
    format!(
        r#"game {{
  title = "{title}",
  author = "Anonymous",
  start = "start"
}}

room "start" {{
  name = "Starting Room",
  desc = function(game)
    if game.has_flag("took_coin") then
      return "A quiet room waits around you. The table is empty."
    end

    return "A quiet room waits around you. A silver coin rests on the table."
  end,
  exits = {{
    north = {{
      to = "garden",
      requires = "silver_coin",
      locked_msg = "The garden gate will not open until you take the coin."
    }}
  }}
}}

room "garden" {{
  name = "Garden",
  desc = "Moonlight gathers on the wet leaves.",
  exits = {{
    south = "start"
  }},

  on_enter = function(game)
    if game.counter("garden_visits") == 0 then
      game.say("The gate clicks shut behind you.")
    end

    game.inc_counter("garden_visits", 1)
  end
}}

thing "silver_coin" {{
  name = "silver coin",
  aliases = {{ "coin", "silver coin" }},
  location = "start",
  portable = true,
  desc = "The coin is stamped with a crescent moon.",

  on_take = function(game)
    if not game.has_flag("draft_scheduled") then
      game.flag("draft_scheduled")
      game.schedule(2, "cold_draft")
    end

    game.flag("took_coin")
    game.say("The coin vanishes into your palm.")
  end,

  on_use = function(game)
    game.say("The coin flashes once, bright as a pinhole moon.")
  end
}}

thing "wool_scarf" {{
  name = "wool scarf",
  aliases = {{ "scarf", "wool scarf" }},
  location = "start",
  portable = true,
  wearable = true,
  desc = "The scarf smells faintly of smoke."
}}

thing "stone_pedestal" {{
  name = "stone pedestal",
  aliases = {{ "pedestal", "stone pedestal" }},
  location = "start",
  portable = false,
  supporter = true,
  desc = "The pedestal is just wide enough to hold a small object."
}}

thing "gardener" {{
  name = "gardener",
  aliases = {{ "gardener" }},
  location = "garden",
  portable = false,
  actor = true,
  desc = "The gardener watches the moonlit leaves.",

  on_talk = function(game)
    game.say("\"Keep your pockets light and your eyes open,\" the gardener says.")
  end,

  topics = {{
    coin = function(game)
      game.say("\"The coin opens more than gates,\" the gardener says.")
    end,

    garden = function(game)
      game.say("\"It grows best when no one is watching,\" the gardener says.")
    end
  }}
}}

event "cold_draft" {{
  on_trigger = function(game)
    game.say("A cold draft slips under the door.")
  end
}}
"#
    )
}

fn template_readme(title: &str) -> String {
    format!(
        r#"# {title}

Run the game:

```bash
moonroom play .
```

Run transcript tests:

```bash
moonroom test .
```
"#
    )
}

const TEMPLATE_TRANSCRIPT: &str = r#"> look
Starting Room

A quiet room waits around you. A silver coin rests on the table.

You can see a silver coin, a stone pedestal, and a wool scarf.

> take scarf
You take the wool scarf.

> wear scarf
You put on the wool scarf.

> i
You are carrying a wool scarf (worn).

> remove scarf
You remove the wool scarf.

> take coin
The coin vanishes into your palm.

> use coin
The coin flashes once, bright as a pinhole moon.

> north
Garden

Moonlight gathers on the wet leaves.

You can see a gardener.

The gate clicks shut behind you.

A cold draft slips under the door.

> talk to gardener
"Keep your pockets light and your eyes open," the gardener says.

> ask gardener about coin
"The coin opens more than gates," the gardener says.

> south
Starting Room

A quiet room waits around you. The table is empty.

You can see a stone pedestal.

> look
Starting Room

A quiet room waits around you. The table is empty.

You can see a stone pedestal.

> north
Garden

Moonlight gathers on the wet leaves.

You can see a gardener.
"#;

const TEMPLATE_LUARC: &str = r#"{
  "$schema": "https://raw.githubusercontent.com/LuaLS/vscode-lua/master/setting/schema.json",
  "runtime.version": "Lua 5.4",
  "diagnostics.globals": ["game", "room", "thing", "verb", "event"]
}
"#;

fn play(game: &mut LuaGame) -> anyhow::Result<()> {
    println!(
        "{}",
        game.opening().map_err(|err| anyhow::anyhow!("{err}"))?
    );

    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        play_interactive(game)
    } else {
        play_plain_stdin(game)
    }
}

fn play_interactive(game: &mut LuaGame) -> anyhow::Result<()> {
    let mut editor = DefaultEditor::new()?;

    loop {
        match editor.readline("\n> ") {
            Ok(input) => {
                if !input.trim().is_empty() {
                    editor.add_history_entry(input.as_str())?;
                }

                if run_input(&input, game)? == ReplAction::Quit {
                    break;
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
            }
            Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(())
}

fn play_plain_stdin(game: &mut LuaGame) -> anyhow::Result<()> {
    let stdin = io::stdin();

    loop {
        print!("\n> ");
        io::stdout().flush()?;

        let mut input = String::new();
        let bytes = stdin.read_line(&mut input)?;

        if bytes == 0 {
            println!();
            break;
        }

        if run_input(&input, game)? == ReplAction::Quit {
            break;
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReplAction {
    Continue,
    Quit,
}

fn run_input(input: &str, game: &mut LuaGame) -> anyhow::Result<ReplAction> {
    if handle_cli_command(input, game)? {
        return Ok(ReplAction::Continue);
    }

    match game
        .handle_command(input)
        .map_err(|err| anyhow::anyhow!("{err}"))?
    {
        CommandResult::Continue(outcome) => {
            if !outcome.output.is_empty() {
                println!("{}", outcome.output);
            }
            Ok(ReplAction::Continue)
        }
        CommandResult::Quit(output) => {
            println!("{output}");
            Ok(ReplAction::Quit)
        }
    }
}

fn handle_cli_command(input: &str, game: &mut LuaGame) -> anyhow::Result<bool> {
    let trimmed = input.trim();
    let mut words = trimmed.split_whitespace();
    let Some(command) = words.next() else {
        return Ok(false);
    };

    match command {
        "save" => {
            let path = words.next().unwrap_or("save.json");
            game.save_to_path(path)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("Saved to {path}.");
            Ok(true)
        }
        "load" => {
            let path = words.next().unwrap_or("save.json");
            game.load_from_path(path)
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("Loaded from {path}.");
            println!();
            println!(
                "{}",
                game.opening().map_err(|err| anyhow::anyhow!("{err}"))?
            );
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn new_project_template_runs_transcript_tests() {
        let project_dir = unique_temp_dir("moonroom-template");
        create_project(&project_dir).expect("template should be created");

        let report = mr_test::run_game_tests(&project_dir).expect("template tests should run");
        assert!(report.is_success());
        assert_eq!(report.passed, 1);

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn project_title_humanizes_path_name() {
        assert_eq!(
            project_title(Path::new("/tmp/the-glass_house")),
            "The Glass House"
        );
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();

        env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }
}
