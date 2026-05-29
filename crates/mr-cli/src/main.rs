use std::{
    env, fs,
    io::{self, BufRead, IsTerminal, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use mr_core::{CommandResult, ThingKind, WorldValidationReport};
use mr_lua::{
    GameSource, LuaGame, SaveOutputMode, load_game_source, pack_game_directory,
    pack_game_directory_to_bytes, unpack_game_package,
};
use rustyline::{DefaultEditor, error::ReadlineError};

const EMBED_MARKER: &[u8] = b"MOONROOM_EMBEDDED_MOON_V1";

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
        #[arg(value_name = "GAME")]
        game_dir: PathBuf,
    },
    Test {
        #[arg(value_name = "GAME")]
        game_dir: PathBuf,

        #[arg(short, long, value_name = "TEXT")]
        filter: Option<String>,

        #[arg(long)]
        update: bool,

        #[arg(long, value_name = "SEED")]
        seed: Option<u64>,
    },
    Check {
        #[arg(value_name = "GAME")]
        game_dir: PathBuf,
    },
    Inspect {
        #[arg(value_name = "GAME")]
        game_dir: PathBuf,
    },
    Transcript {
        #[arg(value_name = "GAME_DIR")]
        game_dir: PathBuf,

        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
    Pack {
        #[arg(value_name = "GAME_DIR")]
        game_dir: PathBuf,

        #[arg(short, long, value_name = "PATH")]
        output: PathBuf,
    },
    Unpack {
        #[arg(value_name = "PACKAGE")]
        package: PathBuf,

        #[arg(short, long, value_name = "DIR")]
        output: PathBuf,
    },
    Build {
        #[arg(value_name = "GAME_DIR")]
        game_dir: PathBuf,

        #[arg(long)]
        standalone: bool,

        #[arg(short, long, value_name = "PATH")]
        output: PathBuf,
    },
    New {
        #[arg(value_name = "NAME")]
        name: String,
    },
}

fn main() -> anyhow::Result<()> {
    if env::args_os().len() == 1
        && let Some(package) = read_embedded_package()?
    {
        let package: &'static [u8] = Box::leak(package.into_boxed_slice());
        let mut game = LuaGame::load_source(GameSource::Embedded(package))
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        play(&mut game)?;
        return Ok(());
    }

    let cli = Cli::parse();

    match cli.command {
        Command::Play { game_dir } => {
            let mut game = LuaGame::load_source(GameSource::from_path(&game_dir))
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            play(&mut game)?;
        }
        Command::Test {
            game_dir,
            filter,
            update,
            seed,
        } => {
            let report = mr_test::run_game_tests_with_options(
                &game_dir,
                &mr_test::TranscriptTestOptions {
                    filter,
                    update,
                    seed,
                },
            )?;

            if report.is_success() {
                println!("{} transcript(s) passed.", report.passed);
            } else {
                for failure in &report.failed {
                    eprintln!(
                        "{}:{} failed on command '{}'",
                        failure.transcript.display(),
                        failure.step,
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
        Command::Check { game_dir } => {
            let report = check_project(&game_dir)?;
            print_check_report(&game_dir, &report);

            if report.has_errors() {
                anyhow::bail!(
                    "validation failed with {} error(s).",
                    report.errors().count()
                );
            }
        }
        Command::Inspect { game_dir } => {
            let game = LuaGame::load_source(GameSource::from_path(&game_dir))
                .map_err(|err| anyhow::anyhow!("{err}"))?;
            print_inspection(&game);
        }
        Command::Transcript { game_dir, output } => {
            let game_file = game_dir.join("game.lua");
            let mut game = LuaGame::load(&game_file).map_err(|err| anyhow::anyhow!("{err}"))?;
            let output =
                output.unwrap_or_else(|| game_dir.join("tests").join("recorded.transcript"));
            record_transcript(&mut game, &output)?;
            println!("Recorded transcript to {}.", output.display());
        }
        Command::Pack { game_dir, output } => {
            pack_game_directory(&game_dir, &output).map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("Packed {} to {}.", game_dir.display(), output.display());
        }
        Command::Unpack { package, output } => {
            unpack_game_package(&package, &output).map_err(|err| anyhow::anyhow!("{err}"))?;
            println!("Unpacked {} to {}.", package.display(), output.display());
        }
        Command::Build {
            game_dir,
            standalone,
            output,
        } => {
            if !standalone {
                anyhow::bail!("only --standalone builds are supported");
            }

            build_standalone(&game_dir, &output)?;
            println!(
                "Built standalone game from {} at {}.",
                game_dir.display(),
                output.display()
            );
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

fn check_project(game_dir: &Path) -> anyhow::Result<WorldValidationReport> {
    let world = load_game_source(GameSource::from_path(game_dir))
        .map_err(|err| anyhow::anyhow!("{err}"))?;

    Ok(world.validate())
}

fn read_embedded_package() -> anyhow::Result<Option<Vec<u8>>> {
    let exe = env::current_exe()?;
    let bytes = fs::read(exe)?;

    if bytes.len() < 8 + EMBED_MARKER.len() {
        return Ok(None);
    }

    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&bytes[bytes.len() - 8..]);
    let package_len = u64::from_le_bytes(len_bytes) as usize;
    let Some(marker_start) = bytes
        .len()
        .checked_sub(8)
        .and_then(|end| end.checked_sub(EMBED_MARKER.len()))
        .and_then(|end| end.checked_sub(package_len))
    else {
        return Ok(None);
    };
    let marker_range = marker_start + package_len..marker_start + package_len + EMBED_MARKER.len();

    if bytes.get(marker_range) != Some(EMBED_MARKER) {
        return Ok(None);
    }

    Ok(Some(
        bytes[marker_start..marker_start + package_len].to_vec(),
    ))
}

fn build_standalone(game_dir: &Path, output: &Path) -> anyhow::Result<()> {
    let package = pack_game_directory_to_bytes(game_dir).map_err(|err| anyhow::anyhow!("{err}"))?;
    let current_exe = env::current_exe()?;
    let mut executable = fs::read(&current_exe)?;

    executable.extend_from_slice(&package);
    executable.extend_from_slice(EMBED_MARKER);
    executable.extend_from_slice(&(package.len() as u64).to_le_bytes());

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    fs::write(output, executable)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(output)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(output, permissions)?;
    }

    Ok(())
}

fn print_check_report(game_dir: &Path, report: &WorldValidationReport) {
    if report.issues.is_empty() {
        println!("{}: no issues found.", game_dir.display());
        return;
    }

    for issue in report.errors() {
        eprintln!("error: {}", issue.message);
    }

    for issue in report.warnings() {
        eprintln!("warning: {}", issue.message);
    }

    if !report.has_errors() {
        println!(
            "{}: {} warning(s), no errors.",
            game_dir.display(),
            report.warnings().count()
        );
    }
}

fn print_inspection(game: &LuaGame) {
    let world = game.world();
    let callbacks = game.callback_summary();

    if let Some(metadata) = &world.metadata {
        println!("{}", metadata.title);
        if let Some(author) = &metadata.author {
            println!("by {author}");
        }
        if let Some(id) = &metadata.id {
            println!("id: {id}");
        }
        if let Some(version) = &metadata.version {
            println!("version: {version}");
        }
        println!("start: {}", metadata.start);
    } else {
        println!("Untitled Moonroom game");
    }

    println!();
    println!("Rooms ({})", world.rooms.len());
    for room in world.rooms.values() {
        let exits = if room.exits.is_empty() {
            "none".to_string()
        } else {
            room.exits.keys().cloned().collect::<Vec<_>>().join(", ")
        };
        let room_callbacks = callbacks
            .rooms
            .get(&room.id)
            .map(|names| names.join(", "))
            .unwrap_or_else(|| "none".to_string());
        println!(
            "- {}: {} [exits: {exits}; callbacks: {room_callbacks}]",
            room.id, room.name
        );
    }

    println!();
    println!("Things ({})", world.things.len());
    for thing in world.things.values() {
        let traits = thing_traits(thing);
        let thing_callbacks = callbacks
            .things
            .get(&thing.id)
            .map(|names| names.join(", "))
            .unwrap_or_else(|| "none".to_string());
        println!(
            "- {}: {} @ {} [{}; callbacks: {}]",
            thing.id,
            thing.name,
            thing.location,
            traits.join(", "),
            thing_callbacks
        );
    }

    println!();
    println!("Verbs ({})", world.verbs.len());
    for verb in world.verbs.values() {
        println!("- {} [aliases: {}]", verb.id, display_list(&verb.aliases));
    }

    println!();
    println!("Actors");
    for (actor_id, topics) in &world.actor_topics {
        let ask = callbacks
            .ask_topics
            .get(actor_id)
            .map(|topics| topics.join(", "))
            .unwrap_or_else(|| "none".to_string());
        let tell = callbacks
            .tell_topics
            .get(actor_id)
            .map(|topics| topics.join(", "))
            .unwrap_or_else(|| "none".to_string());
        println!(
            "- {actor_id} topics: {} [ask: {ask}; tell: {tell}]",
            topics.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }

    println!();
    println!("Events ({})", callbacks.events.len());
    for event in callbacks.events {
        println!("- {event}");
    }

    println!();
    println!(
        "Global callbacks: {}",
        display_static_list(&callbacks.global)
    );
}

fn thing_traits(thing: &mr_core::Thing) -> Vec<&'static str> {
    let mut traits = Vec::new();

    match thing.kind {
        ThingKind::Object => traits.push("object"),
        ThingKind::Container => traits.push("container"),
        ThingKind::Supporter => traits.push("supporter"),
    }

    if thing.portable {
        traits.push("portable");
    }

    if thing.wearable {
        traits.push("wearable");
    }

    if thing.actor {
        traits.push("actor");
    }

    if thing.hidden {
        traits.push("hidden");
    }

    if thing.openable {
        traits.push("openable");
    }

    if thing.lockable {
        traits.push("lockable");
    }

    traits
}

fn display_list(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
}

fn display_static_list(items: &[&str]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(", ")
    }
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
    let id = title
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");

    format!(
        r#"game {{
  id = "{id}",
  version = "0.1.0",
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
  "diagnostics.globals": ["game", "room", "thing", "verb", "event", "include"]
}
"#;

fn record_transcript(game: &mut LuaGame, output: &Path) -> anyhow::Result<()> {
    if output.exists() {
        anyhow::bail!("{} already exists", output.display());
    }

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    println!(
        "{}",
        game.opening().map_err(|err| anyhow::anyhow!("{err}"))?
    );

    let mut transcript = String::new();

    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        record_transcript_interactive(game, &mut transcript)?;
    } else {
        record_transcript_plain_stdin(game, &mut transcript)?;
    }

    fs::write(output, transcript)?;
    Ok(())
}

fn record_transcript_interactive(
    game: &mut LuaGame,
    transcript: &mut String,
) -> anyhow::Result<()> {
    let mut editor = DefaultEditor::new()?;

    loop {
        match editor.readline("\n> ") {
            Ok(input) => {
                if !input.trim().is_empty() {
                    editor.add_history_entry(input.as_str())?;
                }

                if record_transcript_input(&input, game, transcript)? == ReplAction::Quit {
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

fn record_transcript_plain_stdin(
    game: &mut LuaGame,
    transcript: &mut String,
) -> anyhow::Result<()> {
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let input = line?;
        if record_transcript_input(&input, game, transcript)? == ReplAction::Quit {
            break;
        }
    }

    Ok(())
}

fn record_transcript_input(
    input: &str,
    game: &mut LuaGame,
    transcript: &mut String,
) -> anyhow::Result<ReplAction> {
    let input = input.trim();

    if input.is_empty() {
        return Ok(ReplAction::Continue);
    }

    let result = game
        .handle_command(input)
        .map_err(|err| anyhow::anyhow!("{err}"))?;
    let (output, action) = match result {
        CommandResult::Continue(outcome) => (outcome.output, ReplAction::Continue),
        CommandResult::Quit(output) => (output, ReplAction::Quit),
    };

    if !output.is_empty() {
        println!("{output}");
    }

    append_transcript_block(transcript, input, &output);
    Ok(action)
}

fn append_transcript_block(transcript: &mut String, command: &str, output: &str) {
    if !transcript.is_empty() {
        transcript.push('\n');
    }

    transcript.push_str("> ");
    transcript.push_str(command);
    transcript.push('\n');

    if !output.is_empty() {
        transcript.push_str(output.trim_end());
        transcript.push('\n');
    }
}

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
            let (mode, path) = parse_save_command(words.collect::<Vec<_>>())?;
            game.save_to_path_with_mode(&path, mode)
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

fn parse_save_command(args: Vec<&str>) -> anyhow::Result<(SaveOutputMode, String)> {
    let mut mode = SaveOutputMode::Pretty;
    let mut path = None::<String>;

    for arg in args {
        match arg {
            "--pretty" => mode = SaveOutputMode::Pretty,
            "--compact" | "-c" => mode = SaveOutputMode::Compact,
            path_arg if path.is_none() => path = Some(path_arg.to_string()),
            extra => anyhow::bail!("unexpected save argument '{extra}'"),
        }
    }

    Ok((mode, path.unwrap_or_else(|| "save.json".to_string())))
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
    fn transcript_recording_writes_command_blocks() {
        let project_dir = unique_temp_dir("moonroom-record");
        create_project(&project_dir).expect("template should be created");
        let mut game = LuaGame::load(project_dir.join("game.lua")).expect("template should load");
        let mut transcript = String::new();

        let action = record_transcript_input("take coin", &mut game, &mut transcript)
            .expect("record should run command");

        assert_eq!(action, ReplAction::Continue);
        assert_eq!(
            transcript,
            "> take coin\nThe coin vanishes into your palm.\n"
        );

        fs::remove_dir_all(project_dir).expect("temporary project should be removed");
    }

    #[test]
    fn save_command_accepts_output_modes() {
        assert_eq!(
            parse_save_command(vec!["--compact", "slot.json"]).expect("save args parse"),
            (SaveOutputMode::Compact, "slot.json".to_string())
        );
        assert_eq!(
            parse_save_command(vec!["--pretty"]).expect("save args parse"),
            (SaveOutputMode::Pretty, "save.json".to_string())
        );
        assert!(parse_save_command(vec!["one.json", "two.json"]).is_err());
    }

    #[test]
    fn check_project_reports_invalid_world_graph() {
        let project_dir = unique_temp_dir("moonroom-check");
        fs::create_dir_all(&project_dir).expect("test project should be created");
        fs::write(
            project_dir.join("game.lua"),
            r#"
game {
  title = "Broken",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A broken room.",
  exits = {
    north = "missing",
    east = {
      to = "start",
      requires = "missing_key"
    }
  }
}

thing "coin" {
  name = "coin",
  aliases = { "token" },
  location = "start",
  portable = true
}

thing "medal" {
  name = "medal",
  aliases = { "token" },
  location = "missing",
  portable = true
}
"#,
        )
        .expect("test game should be written");

        let report = check_project(&project_dir).expect("check should run");

        assert!(report.has_errors());
        assert!(
            report
                .errors()
                .any(|issue| issue.message.contains("exit 'north' targets missing room"))
        );
        assert!(report.errors().any(|issue| {
            issue
                .message
                .contains("requires missing thing 'missing_key'")
        }));
        assert!(report.errors().any(|issue| {
            issue
                .message
                .contains("thing 'medal' starts in missing location")
        }));
        assert!(
            report
                .warnings()
                .any(|issue| issue.message.contains("alias 'token' is shared"))
        );

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
