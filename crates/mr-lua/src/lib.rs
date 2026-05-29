use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use mlua::{Function, Lua, RegistryKey, Table, Value};
use mr_core::{
    ActorTopic, CommandOutcome, CommandResult, CustomVerb, Exit, ExitDisplaySettings, Game,
    GameError, GameEvent, GameMetadata, GameSettings, GameState, Room, Thing, ThingKind, World,
    random_bounded,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum LuaLoadError {
    #[error("failed to read Lua file '{path}': {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to load Lua file '{path}': {source}")]
    Lua { path: PathBuf, source: mlua::Error },

    #[error("included Lua file '{include}' escapes game directory '{root}'")]
    IncludeEscapesRoot { include: PathBuf, root: PathBuf },

    #[error("failed to parse Moonroom package '{path}': {source}")]
    ParsePackage {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("invalid Moonroom package '{path}': {message}")]
    InvalidPackage { path: PathBuf, message: String },

    #[error(transparent)]
    Game(#[from] GameError),
}

#[derive(Debug, thiserror::Error)]
pub enum LuaRunError {
    #[error("failed to read save file '{path}': {source}")]
    ReadSave {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write save file '{path}': {source}")]
    WriteSave {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to parse save file '{path}': {source}")]
    ParseSave {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error("unsupported save format version {version} in '{path}'")]
    UnsupportedSaveVersion { path: PathBuf, version: u32 },

    #[error("unsupported save format '{format}' in '{path}'")]
    UnsupportedSaveFormat { path: PathBuf, format: String },

    #[error("save file '{path}' belongs to game '{save_game}', not '{current_game}'")]
    WrongGame {
        path: PathBuf,
        save_game: String,
        current_game: String,
    },

    #[error("failed to encode save file '{path}': {source}")]
    EncodeSave {
        path: PathBuf,
        source: serde_json::Error,
    },

    #[error(transparent)]
    Game(#[from] GameError),

    #[error("failed to run Lua callback: {0}")]
    Lua(mlua::Error),
}

const SAVE_FORMAT: &str = "moonroom.save";
const SAVE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveOutputMode {
    Pretty,
    Compact,
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveFile {
    format: String,
    version: u32,
    game: SaveGameIdentity,
    state: GameState,
}

#[derive(Debug, Serialize, Deserialize)]
struct SaveGameIdentity {
    id: String,
    title: String,
    #[serde(default)]
    version: Option<String>,
}

pub const MOON_PACKAGE_FORMAT: &str = "moonroom.moon";
pub const MOON_PACKAGE_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub enum GameSource {
    Directory(PathBuf),
    Package(PathBuf),
    Embedded(&'static [u8]),
}

impl GameSource {
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        if path
            .extension()
            .is_some_and(|extension| extension == "moon")
        {
            Self::Package(path.to_path_buf())
        } else {
            Self::Directory(path.to_path_buf())
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MoonPackage {
    format: String,
    version: u32,
    entry: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    author: Option<String>,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct MoonPackageManifest {
    format: String,
    version: u32,
    entry: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
}

#[derive(Debug)]
struct LoadedPackage {
    entry: PathBuf,
    files: BTreeMap<String, Vec<u8>>,
}

#[derive(Debug, Default)]
struct LoadState {
    world: World,
    callbacks: CallbackRegistry,
    include_root: PathBuf,
    include_stack: Vec<PathBuf>,
    included_files: BTreeSet<PathBuf>,
    package_files: Option<Rc<BTreeMap<String, Vec<u8>>>>,
}

#[derive(Debug, Default)]
struct CallbackRegistry {
    before_action: Option<RegistryKey>,
    after_action: Option<RegistryKey>,
    on_scene_start: Option<RegistryKey>,
    on_scene_end: Option<RegistryKey>,
    on_chapter: Option<RegistryKey>,
    room_desc: BTreeMap<String, RegistryKey>,
    on_enter: BTreeMap<String, RegistryKey>,
    on_look: BTreeMap<String, RegistryKey>,
    on_take: BTreeMap<String, RegistryKey>,
    on_drop: BTreeMap<String, RegistryKey>,
    on_use: BTreeMap<String, RegistryKey>,
    on_use_with: BTreeMap<String, RegistryKey>,
    on_read: BTreeMap<String, RegistryKey>,
    on_open: BTreeMap<String, RegistryKey>,
    on_close: BTreeMap<String, RegistryKey>,
    on_lock: BTreeMap<String, RegistryKey>,
    on_unlock: BTreeMap<String, RegistryKey>,
    on_talk: BTreeMap<String, RegistryKey>,
    on_show: BTreeMap<String, RegistryKey>,
    on_give: BTreeMap<String, RegistryKey>,
    ask_topics: BTreeMap<String, BTreeMap<String, RegistryKey>>,
    tell_topics: BTreeMap<String, BTreeMap<String, RegistryKey>>,
    verbs: BTreeMap<String, RegistryKey>,
    events: BTreeMap<String, RegistryKey>,
}

pub struct LoadedGame {
    lua: Lua,
    world: World,
    callbacks: CallbackRegistry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackSummary {
    pub global: Vec<&'static str>,
    pub rooms: BTreeMap<String, Vec<&'static str>>,
    pub things: BTreeMap<String, Vec<&'static str>>,
    pub ask_topics: BTreeMap<String, Vec<String>>,
    pub tell_topics: BTreeMap<String, Vec<String>>,
    pub verbs: Vec<String>,
    pub events: Vec<String>,
}

pub struct LuaGame {
    lua: Lua,
    game: Game,
    callbacks: CallbackRegistry,
    undo_stack: Vec<GameState>,
    last_command: Option<String>,
}

pub fn load_game(path: impl AsRef<Path>) -> Result<World, LuaLoadError> {
    Ok(load_game_data(path)?.world)
}

pub fn load_game_source(source: GameSource) -> Result<World, LuaLoadError> {
    Ok(load_game_data_from_source(source)?.world)
}

#[derive(Debug, Clone, Copy)]
enum RoomCallback {
    Enter,
    Look,
}

#[derive(Debug, Clone, Copy)]
enum ThingCallback {
    Take,
    Drop,
    Use,
    Read,
    Open,
    Close,
    Lock,
    Unlock,
    Talk,
}

#[derive(Debug, Clone, Copy)]
enum ItemCallback {
    Show,
    Give,
}

#[derive(Debug, Clone, Copy)]
enum ActionCallback {
    Before,
    After,
}

#[derive(Debug, Clone, Copy)]
enum SceneHook {
    Start,
    End,
    Chapter,
}

#[derive(Debug, Clone)]
enum ScriptCommand {
    Flag(String),
    ClearFlag(String),
    SetCounter(String, i64),
    MoveThing(String, String),
    HideThing(String),
    RevealThing(String),
    Goto(String),
    StartScene(String),
    EndScene(Option<String>),
    SetChapter(String),
    Schedule(u64, String),
    ScheduleScene(u64, String, String),
    Cancel(String),
    SetRandomState(u64),
    SetActorMemory(String, String, i64),
}

#[derive(Debug, Clone)]
struct ScriptSession {
    output: Vec<String>,
    commands: Vec<ScriptCommand>,
    flags: BTreeSet<String>,
    counters: BTreeMap<String, i64>,
    actor_memory: BTreeMap<String, BTreeMap<String, i64>>,
    inventory: BTreeSet<String>,
    known_things: BTreeSet<String>,
    hidden_things: BTreeSet<String>,
    current_room: String,
    current_scene: Option<String>,
    current_chapter: Option<String>,
    visited_rooms: BTreeSet<String>,
    random_state: u64,
}

impl ScriptSession {
    fn from_game(game: &Game) -> Self {
        Self {
            output: Vec::new(),
            commands: Vec::new(),
            flags: game.state().flags.clone(),
            counters: game.state().counters.clone(),
            actor_memory: game.state().actor_memory.clone(),
            inventory: game.state().inventory.clone(),
            known_things: game.world().things.keys().cloned().collect(),
            hidden_things: game.state().hidden_things.clone(),
            current_room: game.state().current_room.clone(),
            current_scene: game.state().current_scene.clone(),
            current_chapter: game.state().current_chapter.clone(),
            visited_rooms: game.state().visited_rooms.clone(),
            random_state: game.state().random_state,
        }
    }
}

impl LuaGame {
    const UNDO_LIMIT: usize = 20;

    pub fn load(path: impl AsRef<Path>) -> Result<Self, LuaLoadError> {
        let loaded = load_game_data(path)?;

        Self::from_loaded_game(loaded)
    }

    pub fn load_source(source: GameSource) -> Result<Self, LuaLoadError> {
        let loaded = load_game_data_from_source(source)?;

        Self::from_loaded_game(loaded)
    }

    fn from_loaded_game(loaded: LoadedGame) -> Result<Self, LuaLoadError> {
        let game = Game::new(loaded.world)?;

        Ok(Self {
            lua: loaded.lua,
            game,
            callbacks: loaded.callbacks,
            undo_stack: Vec::new(),
            last_command: None,
        })
    }

    pub fn welcome(&self) -> Result<String, LuaRunError> {
        // Kept as a fallback proof that core can still render a static opening.
        self.game.welcome().map_err(Into::into)
    }

    pub fn opening(&mut self) -> Result<String, LuaRunError> {
        self.render_room()
    }

    pub fn current_room_id(&self) -> &str {
        self.game.current_room_id()
    }

    pub fn world(&self) -> &World {
        self.game.world()
    }

    pub fn callback_summary(&self) -> CallbackSummary {
        let mut global = Vec::new();

        if self.callbacks.before_action.is_some() {
            global.push("before_action");
        }

        if self.callbacks.after_action.is_some() {
            global.push("after_action");
        }

        if self.callbacks.on_scene_start.is_some() {
            global.push("on_scene_start");
        }

        if self.callbacks.on_scene_end.is_some() {
            global.push("on_scene_end");
        }

        if self.callbacks.on_chapter.is_some() {
            global.push("on_chapter");
        }

        CallbackSummary {
            global,
            rooms: self.room_callback_summary(),
            things: self.thing_callback_summary(),
            ask_topics: topic_callback_summary(&self.callbacks.ask_topics),
            tell_topics: topic_callback_summary(&self.callbacks.tell_topics),
            verbs: self.callbacks.verbs.keys().cloned().collect(),
            events: self.callbacks.events.keys().cloned().collect(),
        }
    }

    pub fn current_scene(&self) -> Option<&str> {
        self.game.current_scene()
    }

    pub fn current_chapter(&self) -> Option<&str> {
        self.game.current_chapter()
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.game.has_flag(name)
    }

    pub fn counter(&self, name: &str) -> i64 {
        self.game.counter(name)
    }

    pub fn set_random_seed(&mut self, seed: u64) {
        self.game.set_random_seed(seed);
        self.clear_history();
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<(), LuaRunError> {
        self.save_to_path_with_mode(path, SaveOutputMode::Pretty)
    }

    pub fn save_to_path_with_mode(
        &self,
        path: impl AsRef<Path>,
        mode: SaveOutputMode,
    ) -> Result<(), LuaRunError> {
        let path = path.as_ref();
        let save = SaveFile {
            format: SAVE_FORMAT.to_string(),
            version: SAVE_FORMAT_VERSION,
            game: self.save_game_identity(),
            state: self.game.state().clone(),
        };
        let json = match mode {
            SaveOutputMode::Pretty => serde_json::to_string_pretty(&save),
            SaveOutputMode::Compact => serde_json::to_string(&save),
        }
        .map_err(|source| LuaRunError::EncodeSave {
            path: path.to_path_buf(),
            source,
        })?;

        fs::write(path, json).map_err(|source| LuaRunError::WriteSave {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn save_preview_with_mode(&self, mode: SaveOutputMode) -> Result<String, LuaRunError> {
        let save = SaveFile {
            format: SAVE_FORMAT.to_string(),
            version: SAVE_FORMAT_VERSION,
            game: self.save_game_identity(),
            state: self.game.state().clone(),
        };

        match mode {
            SaveOutputMode::Pretty => serde_json::to_string_pretty(&save),
            SaveOutputMode::Compact => serde_json::to_string(&save),
        }
        .map_err(|source| LuaRunError::EncodeSave {
            path: PathBuf::from("<memory>"),
            source,
        })
    }

    fn save_game_identity(&self) -> SaveGameIdentity {
        let metadata = self
            .game
            .world()
            .metadata
            .as_ref()
            .expect("loaded games have validated metadata");

        SaveGameIdentity {
            id: metadata
                .id
                .clone()
                .unwrap_or_else(|| metadata.title.clone()),
            title: metadata.title.clone(),
            version: metadata.version.clone(),
        }
    }

    fn state_from_save_json(&self, path: &Path, json: &str) -> Result<GameState, LuaRunError> {
        let value = serde_json::from_str::<serde_json::Value>(json).map_err(|source| {
            LuaRunError::ParseSave {
                path: path.to_path_buf(),
                source,
            }
        })?;

        if value.get("state").is_none() {
            return serde_json::from_value::<GameState>(value).map_err(|source| {
                LuaRunError::ParseSave {
                    path: path.to_path_buf(),
                    source,
                }
            });
        }

        let save =
            serde_json::from_value::<SaveFile>(value).map_err(|source| LuaRunError::ParseSave {
                path: path.to_path_buf(),
                source,
            })?;

        if save.format != SAVE_FORMAT {
            return Err(LuaRunError::UnsupportedSaveFormat {
                path: path.to_path_buf(),
                format: save.format,
            });
        }

        if save.version != SAVE_FORMAT_VERSION {
            return Err(LuaRunError::UnsupportedSaveVersion {
                path: path.to_path_buf(),
                version: save.version,
            });
        }

        let current = self.save_game_identity();
        if save.game.id != current.id {
            return Err(LuaRunError::WrongGame {
                path: path.to_path_buf(),
                save_game: save.game.id,
                current_game: current.id,
            });
        }

        Ok(save.state)
    }

    pub fn load_from_path(&mut self, path: impl AsRef<Path>) -> Result<(), LuaRunError> {
        let path = path.as_ref();
        let json = fs::read_to_string(path).map_err(|source| LuaRunError::ReadSave {
            path: path.to_path_buf(),
            source,
        })?;
        let state = self.state_from_save_json(path, &json)?;

        self.game.replace_state(state)?;
        self.clear_history();
        Ok(())
    }

    pub fn handle_command(&mut self, input: &str) -> Result<CommandResult, LuaRunError> {
        let normalized = normalized_command(input);

        if matches!(normalized.as_str(), "again" | "g") {
            let Some(command) = self.last_command.clone() else {
                return Ok(CommandResult::Continue(CommandOutcome::new(
                    "Nothing to do again.",
                )));
            };

            return self.handle_command(&command);
        }

        if normalized == "undo" {
            let Some(state) = self.undo_stack.pop() else {
                return Ok(CommandResult::Continue(CommandOutcome::new(
                    "Nothing to undo.",
                )));
            };

            self.game.restore_state_for_undo(state)?;
            return Ok(CommandResult::Continue(CommandOutcome::new("Undone.")));
        }

        let advances_turn = command_advances_turn(&normalized);
        let pre_command_state = advances_turn.then(|| self.game.state().clone());

        if !normalized.is_empty()
            && !is_quit_command(&normalized)
            && let Some(output) = self.run_action_callback(ActionCallback::Before, &normalized)?
        {
            return Ok(CommandResult::Continue(CommandOutcome::new(output)));
        }

        let result = self.game.handle_command(input)?;

        match result {
            CommandResult::Continue(mut outcome) => {
                let events = outcome.events.clone();

                for event in events {
                    match event {
                        GameEvent::Look { room_id } => {
                            outcome.output = self.render_room()?;
                            if let Some(output) =
                                self.run_room_callback(&room_id, RoomCallback::Look)?
                            {
                                outcome.output = append_output(outcome.output, output);
                            }
                        }
                        GameEvent::EnterRoom { room_id } => {
                            outcome.output = self.render_room()?;
                            if let Some(output) =
                                self.run_room_callback(&room_id, RoomCallback::Enter)?
                            {
                                outcome.output = append_output(outcome.output, output);
                            }
                        }
                        GameEvent::Take { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Take)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Drop { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Drop)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Use { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Use)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::UseWith { item_id, target_id } => {
                            if let Some(output) =
                                self.run_use_with_callback(&item_id, &target_id)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Read { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Read)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Open { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Open)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Close { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Close)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Lock { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Lock)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Unlock { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Unlock)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Talk { thing_id } => {
                            if let Some(output) =
                                self.run_thing_callback(&thing_id, ThingCallback::Talk)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Ask { thing_id, topic } => {
                            if let Some(output) = self.run_ask_callback(&thing_id, &topic)? {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Tell { thing_id, topic } => {
                            if let Some(output) = self.run_tell_callback(&thing_id, &topic)? {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Show { thing_id, item_id } => {
                            if let Some(output) =
                                self.run_item_callback(&thing_id, &item_id, ItemCallback::Show)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::Give { thing_id, item_id } => {
                            if let Some(output) =
                                self.run_item_callback(&thing_id, &item_id, ItemCallback::Give)?
                            {
                                outcome.output = output;
                            }
                        }
                        GameEvent::CustomVerb { verb_id, input } => {
                            outcome.output = self
                                .run_verb_callback(&verb_id, &input)?
                                .unwrap_or_else(|| "Nothing happens.".to_string());
                        }
                        GameEvent::Timer { event_name } => {
                            if let Some(output) = self.run_event_callback(&event_name)? {
                                outcome.output = append_output(outcome.output, output);
                            }
                        }
                    }
                }

                if !normalized.is_empty()
                    && let Some(output) =
                        self.run_action_callback(ActionCallback::After, &normalized)?
                {
                    outcome.output = append_output(outcome.output, output);
                }

                self.remember_successful_command(pre_command_state, normalized);
                Ok(CommandResult::Continue(outcome))
            }
            CommandResult::Quit(output) => Ok(CommandResult::Quit(output)),
        }
    }

    fn remember_successful_command(
        &mut self,
        pre_command_state: Option<GameState>,
        command: String,
    ) {
        let Some(state) = pre_command_state else {
            return;
        };

        self.undo_stack.push(state);

        if self.undo_stack.len() > Self::UNDO_LIMIT {
            self.undo_stack.remove(0);
        }

        self.last_command = Some(command.clone());
        self.game.remember_command(command);
    }

    fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.last_command = None;
        self.game.clear_history();
    }

    fn room_callback_summary(&self) -> BTreeMap<String, Vec<&'static str>> {
        let mut rooms = BTreeMap::<String, Vec<&'static str>>::new();

        for room_id in self.callbacks.room_desc.keys() {
            rooms.entry(room_id.clone()).or_default().push("desc");
        }

        for room_id in self.callbacks.on_enter.keys() {
            rooms.entry(room_id.clone()).or_default().push("on_enter");
        }

        for room_id in self.callbacks.on_look.keys() {
            rooms.entry(room_id.clone()).or_default().push("on_look");
        }

        rooms
    }

    fn thing_callback_summary(&self) -> BTreeMap<String, Vec<&'static str>> {
        let mut things = BTreeMap::<String, Vec<&'static str>>::new();
        add_callback_names(&mut things, &self.callbacks.on_take, "on_take");
        add_callback_names(&mut things, &self.callbacks.on_drop, "on_drop");
        add_callback_names(&mut things, &self.callbacks.on_use, "on_use");
        add_callback_names(&mut things, &self.callbacks.on_use_with, "on_use_with");
        add_callback_names(&mut things, &self.callbacks.on_read, "on_read");
        add_callback_names(&mut things, &self.callbacks.on_open, "on_open");
        add_callback_names(&mut things, &self.callbacks.on_close, "on_close");
        add_callback_names(&mut things, &self.callbacks.on_lock, "on_lock");
        add_callback_names(&mut things, &self.callbacks.on_unlock, "on_unlock");
        add_callback_names(&mut things, &self.callbacks.on_talk, "on_talk");
        add_callback_names(&mut things, &self.callbacks.on_show, "on_show");
        add_callback_names(&mut things, &self.callbacks.on_give, "on_give");
        things
    }

    fn render_room(&mut self) -> Result<String, LuaRunError> {
        let room_id = self.game.current_room_id().to_string();
        let room = self
            .game
            .world()
            .rooms
            .get(&room_id)
            .expect("current room is validated by core");

        let desc = if self.callbacks.room_desc.contains_key(&room_id) {
            self.run_room_desc(&room_id)?
        } else {
            room.desc.clone()
        };

        self.game.room_view(desc).map_err(Into::into)
    }

    fn run_room_desc(&mut self, room_id: &str) -> Result<String, LuaRunError> {
        let callback = self
            .callbacks
            .room_desc
            .get(room_id)
            .expect("checked by caller");
        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        let desc = function.call::<String>(api).map_err(LuaRunError::Lua)?;
        self.apply_script_commands(&session)?;
        Ok(desc)
    }

    fn run_room_callback(
        &mut self,
        room_id: &str,
        kind: RoomCallback,
    ) -> Result<Option<String>, LuaRunError> {
        let callbacks = match kind {
            RoomCallback::Enter => &self.callbacks.on_enter,
            RoomCallback::Look => &self.callbacks.on_look,
        };

        let Some(callback) = callbacks.get(room_id) else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function.call::<()>(api).map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_thing_callback(
        &mut self,
        thing_id: &str,
        kind: ThingCallback,
    ) -> Result<Option<String>, LuaRunError> {
        let callbacks = match kind {
            ThingCallback::Take => &self.callbacks.on_take,
            ThingCallback::Drop => &self.callbacks.on_drop,
            ThingCallback::Use => &self.callbacks.on_use,
            ThingCallback::Read => &self.callbacks.on_read,
            ThingCallback::Open => &self.callbacks.on_open,
            ThingCallback::Close => &self.callbacks.on_close,
            ThingCallback::Lock => &self.callbacks.on_lock,
            ThingCallback::Unlock => &self.callbacks.on_unlock,
            ThingCallback::Talk => &self.callbacks.on_talk,
        };

        let Some(callback) = callbacks.get(thing_id) else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function.call::<()>(api).map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_verb_callback(
        &mut self,
        verb_id: &str,
        input: &str,
    ) -> Result<Option<String>, LuaRunError> {
        let Some(callback) = self.callbacks.verbs.get(verb_id) else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function
            .call::<()>((api, input))
            .map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_use_with_callback(
        &mut self,
        item_id: &str,
        target_id: &str,
    ) -> Result<Option<String>, LuaRunError> {
        let Some(callback) = self.callbacks.on_use_with.get(item_id) else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function
            .call::<()>((api, item_id, target_id))
            .map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_event_callback(&mut self, event_name: &str) -> Result<Option<String>, LuaRunError> {
        let Some(callback) = self.callbacks.events.get(event_name) else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function.call::<()>(api).map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_ask_callback(
        &mut self,
        thing_id: &str,
        topic: &str,
    ) -> Result<Option<String>, LuaRunError> {
        let Some(callback) = self
            .callbacks
            .ask_topics
            .get(thing_id)
            .and_then(|topics| topics.get(topic))
        else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function
            .call::<()>((api, topic))
            .map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_tell_callback(
        &mut self,
        thing_id: &str,
        topic: &str,
    ) -> Result<Option<String>, LuaRunError> {
        let Some(callback) = self
            .callbacks
            .tell_topics
            .get(thing_id)
            .and_then(|topics| topics.get(topic))
        else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function
            .call::<()>((api, topic))
            .map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_item_callback(
        &mut self,
        thing_id: &str,
        item_id: &str,
        kind: ItemCallback,
    ) -> Result<Option<String>, LuaRunError> {
        let callbacks = match kind {
            ItemCallback::Show => &self.callbacks.on_show,
            ItemCallback::Give => &self.callbacks.on_give,
        };

        let Some(callback) = callbacks.get(thing_id) else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function
            .call::<()>((api, item_id))
            .map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_action_callback(
        &mut self,
        kind: ActionCallback,
        input: &str,
    ) -> Result<Option<String>, LuaRunError> {
        let callback = match kind {
            ActionCallback::Before => self.callbacks.before_action.as_ref(),
            ActionCallback::After => self.callbacks.after_action.as_ref(),
        };
        let Some(callback) = callback else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function
            .call::<()>((api, input))
            .map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn run_scene_hook(
        &mut self,
        kind: SceneHook,
        name: &str,
    ) -> Result<Option<String>, LuaRunError> {
        let callback = match kind {
            SceneHook::Start => self.callbacks.on_scene_start.as_ref(),
            SceneHook::End => self.callbacks.on_scene_end.as_ref(),
            SceneHook::Chapter => self.callbacks.on_chapter.as_ref(),
        };
        let Some(callback) = callback else {
            return Ok(None);
        };

        let function = self
            .lua
            .registry_value::<Function>(callback)
            .map_err(LuaRunError::Lua)?;
        let (api, session) = self.game_api().map_err(LuaRunError::Lua)?;
        function.call::<()>((api, name)).map_err(LuaRunError::Lua)?;
        self.take_script_output(&session)
    }

    fn game_api(&self) -> mlua::Result<(Table, Rc<RefCell<ScriptSession>>)> {
        let api = self.lua.create_table()?;
        let session = Rc::new(RefCell::new(ScriptSession::from_game(&self.game)));

        let say_session = Rc::clone(&session);
        api.set(
            "say",
            self.lua.create_function(move |_lua, text: String| {
                say_session.borrow_mut().output.push(text);
                Ok(())
            })?,
        )?;

        let flag_session = Rc::clone(&session);
        api.set(
            "flag",
            self.lua.create_function(move |_lua, name: String| {
                let mut session = flag_session.borrow_mut();
                session.flags.insert(name.clone());
                session.commands.push(ScriptCommand::Flag(name));
                Ok(())
            })?,
        )?;

        let clear_flag_session = Rc::clone(&session);
        api.set(
            "clear_flag",
            self.lua.create_function(move |_lua, name: String| {
                let mut session = clear_flag_session.borrow_mut();
                session.flags.remove(&name);
                session.commands.push(ScriptCommand::ClearFlag(name));
                Ok(())
            })?,
        )?;

        let has_flag_session = Rc::clone(&session);
        api.set(
            "has_flag",
            self.lua.create_function(move |_lua, name: String| {
                Ok(has_flag_session.borrow().flags.contains(&name))
            })?,
        )?;

        let counter_session = Rc::clone(&session);
        api.set(
            "counter",
            self.lua.create_function(move |_lua, name: String| {
                Ok(counter_session
                    .borrow()
                    .counters
                    .get(&name)
                    .copied()
                    .unwrap_or(0))
            })?,
        )?;

        let set_counter_session = Rc::clone(&session);
        api.set(
            "set_counter",
            self.lua
                .create_function(move |_lua, (name, value): (String, i64)| {
                    let mut session = set_counter_session.borrow_mut();
                    session.counters.insert(name.clone(), value);
                    session
                        .commands
                        .push(ScriptCommand::SetCounter(name, value));
                    Ok(())
                })?,
        )?;

        let inc_counter_session = Rc::clone(&session);
        api.set(
            "inc_counter",
            self.lua
                .create_function(move |_lua, (name, amount): (String, Option<i64>)| {
                    let mut session = inc_counter_session.borrow_mut();
                    let amount = amount.unwrap_or(1);
                    let value = session.counters.entry(name.clone()).or_default();
                    *value += amount;
                    let value = *value;
                    session
                        .commands
                        .push(ScriptCommand::SetCounter(name, value));
                    Ok(value)
                })?,
        )?;

        let actor_memory_session = Rc::clone(&session);
        api.set(
            "actor_memory",
            self.lua
                .create_function(move |_lua, (actor_id, key): (String, String)| {
                    Ok(actor_memory_session
                        .borrow()
                        .actor_memory
                        .get(&actor_id)
                        .and_then(|memory| memory.get(&key))
                        .copied()
                        .unwrap_or(0))
                })?,
        )?;

        let set_actor_memory_session = Rc::clone(&session);
        api.set(
            "set_actor_memory",
            self.lua.create_function(
                move |_lua, (actor_id, key, value): (String, String, i64)| {
                    let mut session = set_actor_memory_session.borrow_mut();
                    session
                        .actor_memory
                        .entry(actor_id.clone())
                        .or_default()
                        .insert(key.clone(), value);
                    session
                        .commands
                        .push(ScriptCommand::SetActorMemory(actor_id, key, value));
                    Ok(())
                },
            )?,
        )?;

        let inc_actor_memory_session = Rc::clone(&session);
        api.set(
            "inc_actor_memory",
            self.lua.create_function(
                move |_lua, (actor_id, key, amount): (String, String, Option<i64>)| {
                    let mut session = inc_actor_memory_session.borrow_mut();
                    let amount = amount.unwrap_or(1);
                    let value = session
                        .actor_memory
                        .entry(actor_id.clone())
                        .or_default()
                        .entry(key.clone())
                        .or_default();
                    *value += amount;
                    let value = *value;
                    session
                        .commands
                        .push(ScriptCommand::SetActorMemory(actor_id, key, value));
                    Ok(value)
                },
            )?,
        )?;

        let move_session = Rc::clone(&session);
        api.set(
            "move",
            self.lua
                .create_function(move |_lua, (thing_id, location_id): (String, String)| {
                    let mut session = move_session.borrow_mut();
                    if location_id == "inventory" {
                        session.inventory.insert(thing_id.clone());
                    } else {
                        session.inventory.remove(&thing_id);
                    }
                    session
                        .commands
                        .push(ScriptCommand::MoveThing(thing_id, location_id));
                    Ok(())
                })?,
        )?;

        let goto_session = Rc::clone(&session);
        api.set(
            "goto",
            self.lua.create_function(move |_lua, room_id: String| {
                let mut session = goto_session.borrow_mut();
                session.current_room = room_id.clone();
                session.visited_rooms.insert(room_id.clone());
                session.commands.push(ScriptCommand::Goto(room_id));
                Ok(())
            })?,
        )?;

        let scene_session = Rc::clone(&session);
        api.set(
            "scene",
            self.lua.create_function(move |_lua, ()| {
                Ok(scene_session.borrow().current_scene.clone())
            })?,
        )?;

        let start_scene_session = Rc::clone(&session);
        api.set(
            "start_scene",
            self.lua.create_function(move |_lua, name: String| {
                let mut session = start_scene_session.borrow_mut();
                session.current_scene = Some(name.clone());
                session.commands.push(ScriptCommand::StartScene(name));
                Ok(())
            })?,
        )?;

        let end_scene_session = Rc::clone(&session);
        api.set(
            "end_scene",
            self.lua
                .create_function(move |_lua, name: Option<String>| {
                    let mut session = end_scene_session.borrow_mut();
                    let target = name.or_else(|| session.current_scene.clone());

                    if target.is_some() {
                        session.current_scene = None;
                        session.commands.push(ScriptCommand::EndScene(target));
                    }

                    Ok(())
                })?,
        )?;

        let chapter_session = Rc::clone(&session);
        api.set(
            "chapter",
            self.lua
                .create_function(move |_lua, name: Option<String>| {
                    let mut session = chapter_session.borrow_mut();

                    if let Some(name) = name {
                        session.current_chapter = Some(name.clone());
                        session.commands.push(ScriptCommand::SetChapter(name));
                    }

                    Ok(session.current_chapter.clone())
                })?,
        )?;

        let schedule_session = Rc::clone(&session);
        api.set(
            "schedule",
            self.lua
                .create_function(move |_lua, (turns, event_name): (u64, String)| {
                    schedule_session
                        .borrow_mut()
                        .commands
                        .push(ScriptCommand::Schedule(turns, event_name));
                    Ok(())
                })?,
        )?;

        let schedule_scene_session = Rc::clone(&session);
        api.set(
            "schedule_scene",
            self.lua
                .create_function(move |_lua, (turns, event_name): (u64, String)| {
                    let scene = schedule_scene_session
                        .borrow()
                        .current_scene
                        .clone()
                        .ok_or_else(|| {
                            mlua::Error::runtime("game.schedule_scene requires an active scene")
                        })?;
                    schedule_scene_session
                        .borrow_mut()
                        .commands
                        .push(ScriptCommand::ScheduleScene(turns, event_name, scene));
                    Ok(())
                })?,
        )?;

        let cancel_session = Rc::clone(&session);
        api.set(
            "cancel",
            self.lua.create_function(move |_lua, event_name: String| {
                cancel_session
                    .borrow_mut()
                    .commands
                    .push(ScriptCommand::Cancel(event_name));
                Ok(())
            })?,
        )?;

        let has_session = Rc::clone(&session);
        api.set(
            "has",
            self.lua.create_function(move |_lua, thing_id: String| {
                Ok(has_session.borrow().inventory.contains(&thing_id))
            })?,
        )?;

        let visible_session = Rc::clone(&session);
        api.set(
            "visible",
            self.lua.create_function(move |_lua, thing_id: String| {
                let session = visible_session.borrow();
                Ok(session.known_things.contains(&thing_id)
                    && !session.hidden_things.contains(&thing_id))
            })?,
        )?;

        let hide_session = Rc::clone(&session);
        api.set(
            "hide",
            self.lua.create_function(move |_lua, thing_id: String| {
                let mut session = hide_session.borrow_mut();
                if session.known_things.contains(&thing_id) {
                    session.hidden_things.insert(thing_id.clone());
                    session.commands.push(ScriptCommand::HideThing(thing_id));
                }
                Ok(())
            })?,
        )?;

        let reveal_session = Rc::clone(&session);
        api.set(
            "reveal",
            self.lua.create_function(move |_lua, thing_id: String| {
                let mut session = reveal_session.borrow_mut();
                if session.known_things.contains(&thing_id) {
                    session.hidden_things.remove(&thing_id);
                    session.commands.push(ScriptCommand::RevealThing(thing_id));
                }
                Ok(())
            })?,
        )?;

        let room_session = Rc::clone(&session);
        api.set(
            "room",
            self.lua
                .create_function(move |_lua, ()| Ok(room_session.borrow().current_room.clone()))?,
        )?;

        let visited_session = Rc::clone(&session);
        api.set(
            "visited",
            self.lua.create_function(move |_lua, room_id: String| {
                Ok(visited_session.borrow().visited_rooms.contains(&room_id))
            })?,
        )?;

        let turn = self.game.state().turn;
        api.set("turn", self.lua.create_function(move |_lua, ()| Ok(turn))?)?;

        let random_session = Rc::clone(&session);
        api.set(
            "random",
            self.lua
                .create_function(move |_lua, (min, max): (i64, i64)| {
                    let Some(width) = max.checked_sub(min).and_then(|value| value.checked_add(1))
                    else {
                        return Err(mlua::Error::runtime(
                            "game.random range is too large or inverted",
                        ));
                    };

                    if width <= 0 {
                        return Err(mlua::Error::runtime("game.random min must be <= max"));
                    }

                    let upper_exclusive = u64::try_from(width)
                        .map_err(|_| mlua::Error::runtime("game.random range must fit in u64"))?;
                    let mut session = random_session.borrow_mut();
                    let (next_state, value) = random_bounded(session.random_state, upper_exclusive);
                    session.random_state = next_state;
                    session
                        .commands
                        .push(ScriptCommand::SetRandomState(next_state));
                    Ok(min + i64::try_from(value).expect("random value came from i64 width"))
                })?,
        )?;

        Ok((api, session))
    }

    fn take_script_output(
        &mut self,
        session: &Rc<RefCell<ScriptSession>>,
    ) -> Result<Option<String>, LuaRunError> {
        self.apply_script_commands(session)?;
        let output = session.borrow().output.join("\n");

        Ok((!output.is_empty()).then_some(output))
    }

    fn apply_script_commands(
        &mut self,
        session: &Rc<RefCell<ScriptSession>>,
    ) -> Result<(), LuaRunError> {
        let commands = session.borrow().commands.clone();

        for command in commands {
            match command {
                ScriptCommand::Flag(name) => self.game.flag(name),
                ScriptCommand::ClearFlag(name) => self.game.clear_flag(&name),
                ScriptCommand::SetCounter(name, value) => self.game.set_counter(name, value),
                ScriptCommand::MoveThing(thing_id, location_id) => {
                    self.game.move_thing(&thing_id, location_id);
                }
                ScriptCommand::HideThing(thing_id) => {
                    self.game.hide_thing(&thing_id);
                }
                ScriptCommand::RevealThing(thing_id) => {
                    self.game.reveal_thing(&thing_id);
                }
                ScriptCommand::Goto(room_id) => self.game.goto(&room_id)?,
                ScriptCommand::StartScene(name) => {
                    self.game.start_scene(name.clone());
                    if let Some(output) = self.run_scene_hook(SceneHook::Start, &name)? {
                        session.borrow_mut().output.push(output);
                    }
                }
                ScriptCommand::EndScene(name) => {
                    if let Some(ended) = self.game.end_scene(name.as_deref())
                        && let Some(output) = self.run_scene_hook(SceneHook::End, &ended)?
                    {
                        session.borrow_mut().output.push(output);
                    }
                }
                ScriptCommand::SetChapter(name) => {
                    self.game.set_chapter(name.clone());
                    if let Some(output) = self.run_scene_hook(SceneHook::Chapter, &name)? {
                        session.borrow_mut().output.push(output);
                    }
                }
                ScriptCommand::Schedule(turns, event_name) => {
                    self.game.schedule_event(turns, event_name);
                }
                ScriptCommand::ScheduleScene(turns, event_name, scene) => {
                    self.game.schedule_scene_event(turns, event_name, scene);
                }
                ScriptCommand::Cancel(event_name) => self.game.cancel_event(&event_name),
                ScriptCommand::SetRandomState(random_state) => {
                    self.game.set_random_state(random_state);
                }
                ScriptCommand::SetActorMemory(actor_id, key, value) => {
                    self.game.set_actor_memory(actor_id, key, value);
                }
            }
        }

        Ok(())
    }
}

fn load_game_data(path: impl AsRef<Path>) -> Result<LoadedGame, LuaLoadError> {
    let path = path.as_ref();
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    let root = fs::canonicalize(root).map_err(|source| LuaLoadError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    let entrypoint = fs::canonicalize(path).map_err(|source| LuaLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    ensure_path_in_root(&root, &entrypoint)?;

    let lua = Lua::new();
    let state = Rc::new(RefCell::new(LoadState {
        include_root: root,
        ..LoadState::default()
    }));

    register_dsl(&lua, Rc::clone(&state)).map_err(|source| LuaLoadError::Lua {
        path: entrypoint.clone(),
        source,
    })?;

    load_lua_file(&lua, Rc::clone(&state), entrypoint)?;

    let mut state = state.borrow_mut();
    let world = std::mem::take(&mut state.world);
    let callbacks = std::mem::take(&mut state.callbacks);
    drop(state);

    Ok(LoadedGame {
        lua,
        world,
        callbacks,
    })
}

fn load_game_data_from_source(source: GameSource) -> Result<LoadedGame, LuaLoadError> {
    match source {
        GameSource::Directory(path) => load_game_data(path.join("game.lua")),
        GameSource::Package(path) => {
            let bytes = fs::read(&path).map_err(|source| LuaLoadError::Io {
                path: path.clone(),
                source,
            })?;
            let package = decode_moon_package(&path, &bytes)?;
            load_game_data_from_package(&path, package)
        }
        GameSource::Embedded(bytes) => {
            let path = PathBuf::from("<embedded .moon>");
            let package = decode_moon_package(&path, bytes)?;
            load_game_data_from_package(&path, package)
        }
    }
}

fn load_game_data_from_package(
    package_path: &Path,
    package: LoadedPackage,
) -> Result<LoadedGame, LuaLoadError> {
    let entrypoint = package.entry;
    let files = Rc::new(package.files);
    let lua = Lua::new();
    let state = Rc::new(RefCell::new(LoadState {
        include_root: package_path.to_path_buf(),
        package_files: Some(Rc::clone(&files)),
        ..LoadState::default()
    }));

    register_dsl(&lua, Rc::clone(&state)).map_err(|source| LuaLoadError::Lua {
        path: entrypoint.clone(),
        source,
    })?;

    load_lua_file(&lua, Rc::clone(&state), entrypoint)?;

    let mut state = state.borrow_mut();
    let world = std::mem::take(&mut state.world);
    let callbacks = std::mem::take(&mut state.callbacks);
    drop(state);

    Ok(LoadedGame {
        lua,
        world,
        callbacks,
    })
}

pub fn pack_game_directory(
    game_dir: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(), LuaLoadError> {
    let bytes = pack_game_directory_to_bytes(game_dir)?;
    let output = output.as_ref();

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|source| LuaLoadError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(output, bytes).map_err(|source| LuaLoadError::Io {
        path: output.to_path_buf(),
        source,
    })
}

pub fn pack_game_directory_to_bytes(game_dir: impl AsRef<Path>) -> Result<Vec<u8>, LuaLoadError> {
    let game_dir = game_dir.as_ref();
    let root = fs::canonicalize(game_dir).map_err(|source| LuaLoadError::Io {
        path: game_dir.to_path_buf(),
        source,
    })?;
    let world = load_game_source(GameSource::Directory(root.clone()))?;
    let metadata = world.metadata.as_ref();
    let mut files = BTreeMap::new();

    collect_package_files(&root, &root, &mut files)?;

    if !files.contains_key("game.lua") {
        return Err(LuaLoadError::InvalidPackage {
            path: game_dir.to_path_buf(),
            message: "game directory must contain game.lua".to_string(),
        });
    }

    let title = metadata.map(|metadata| metadata.title.clone());
    let author = metadata.and_then(|metadata| metadata.author.clone());
    let manifest = MoonPackageManifest {
        format: MOON_PACKAGE_FORMAT.to_string(),
        version: MOON_PACKAGE_VERSION,
        entry: "game.lua".to_string(),
        title: title.clone(),
        author: author.clone(),
    };
    let manifest_json =
        serde_json::to_vec_pretty(&manifest).map_err(|source| LuaLoadError::ParsePackage {
            path: game_dir.to_path_buf(),
            source,
        })?;
    files.insert("moon.json".to_string(), hex_encode(&manifest_json));

    let package = MoonPackage {
        format: MOON_PACKAGE_FORMAT.to_string(),
        version: MOON_PACKAGE_VERSION,
        entry: "game.lua".to_string(),
        title,
        author,
        files,
    };

    serde_json::to_vec_pretty(&package).map_err(|source| LuaLoadError::ParsePackage {
        path: game_dir.to_path_buf(),
        source,
    })
}

pub fn unpack_game_package(
    package_path: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
) -> Result<(), LuaLoadError> {
    let package_path = package_path.as_ref();
    let output_dir = output_dir.as_ref();
    let package = read_moon_package(package_path)?;

    if output_dir.exists()
        && output_dir
            .read_dir()
            .map_err(|source| LuaLoadError::Io {
                path: output_dir.to_path_buf(),
                source,
            })?
            .next()
            .is_some()
    {
        return Err(LuaLoadError::InvalidPackage {
            path: output_dir.to_path_buf(),
            message: "unpack output directory already exists and is not empty".to_string(),
        });
    }

    fs::create_dir_all(output_dir).map_err(|source| LuaLoadError::Io {
        path: output_dir.to_path_buf(),
        source,
    })?;

    for (path, bytes) in package.files {
        let relative =
            normalize_package_path(path).map_err(|message| LuaLoadError::InvalidPackage {
                path: package_path.to_path_buf(),
                message,
            })?;
        let output_path = output_dir.join(relative);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| LuaLoadError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        fs::write(&output_path, bytes).map_err(|source| LuaLoadError::Io {
            path: output_path,
            source,
        })?;
    }

    Ok(())
}

pub fn package_file_names(package_path: impl AsRef<Path>) -> Result<Vec<String>, LuaLoadError> {
    let package = read_moon_package(package_path)?;

    Ok(package.files.keys().cloned().collect())
}

pub fn package_file_text(
    package_path: impl AsRef<Path>,
    file: impl AsRef<Path>,
) -> Result<String, LuaLoadError> {
    let package_path = package_path.as_ref();
    let file = normalize_package_path(file).map_err(|message| LuaLoadError::InvalidPackage {
        path: package_path.to_path_buf(),
        message,
    })?;
    let package = read_moon_package(package_path)?;
    let key = package_path_key(&file);
    let Some(bytes) = package.files.get(&key) else {
        return Err(LuaLoadError::InvalidPackage {
            path: package_path.to_path_buf(),
            message: format!("package does not contain '{key}'"),
        });
    };

    String::from_utf8(bytes.clone()).map_err(|_| LuaLoadError::InvalidPackage {
        path: package_path.to_path_buf(),
        message: format!("package file '{key}' is not valid UTF-8"),
    })
}

fn collect_package_files(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, String>,
) -> Result<(), LuaLoadError> {
    let mut entries = Vec::new();

    for entry in fs::read_dir(dir).map_err(|source| LuaLoadError::Io {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| LuaLoadError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        entries.push(entry.path());
    }

    entries.sort();

    for path in entries {
        let metadata = fs::metadata(&path).map_err(|source| LuaLoadError::Io {
            path: path.clone(),
            source,
        })?;

        if metadata.is_dir() {
            collect_package_files(root, &path, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| LuaLoadError::InvalidPackage {
                    path: root.to_path_buf(),
                    message: format!("package file '{}' escaped the game root", path.display()),
                })?;
            let relative = normalize_package_path(relative).map_err(|message| {
                LuaLoadError::InvalidPackage {
                    path: root.to_path_buf(),
                    message,
                }
            })?;
            let bytes = fs::read(&path).map_err(|source| LuaLoadError::Io {
                path: path.clone(),
                source,
            })?;

            files.insert(package_path_key(&relative), hex_encode(&bytes));
        }
    }

    Ok(())
}

fn read_moon_package(path: impl AsRef<Path>) -> Result<LoadedPackage, LuaLoadError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|source| LuaLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    decode_moon_package(path, &bytes)
}

fn decode_moon_package(path: &Path, bytes: &[u8]) -> Result<LoadedPackage, LuaLoadError> {
    let package: MoonPackage =
        serde_json::from_slice(bytes).map_err(|source| LuaLoadError::ParsePackage {
            path: path.to_path_buf(),
            source,
        })?;

    if package.format != MOON_PACKAGE_FORMAT {
        return Err(LuaLoadError::InvalidPackage {
            path: path.to_path_buf(),
            message: format!("unsupported package format '{}'", package.format),
        });
    }

    if package.version != MOON_PACKAGE_VERSION {
        return Err(LuaLoadError::InvalidPackage {
            path: path.to_path_buf(),
            message: format!("unsupported package version {}", package.version),
        });
    }

    let entry =
        normalize_package_path(&package.entry).map_err(|message| LuaLoadError::InvalidPackage {
            path: path.to_path_buf(),
            message,
        })?;
    let mut files = BTreeMap::new();

    for (file_path, encoded) in package.files {
        let file_path =
            normalize_package_path(file_path).map_err(|message| LuaLoadError::InvalidPackage {
                path: path.to_path_buf(),
                message,
            })?;
        let bytes = hex_decode(&encoded).map_err(|message| LuaLoadError::InvalidPackage {
            path: path.to_path_buf(),
            message,
        })?;

        files.insert(package_path_key(&file_path), bytes);
    }

    if !files.contains_key(&package_path_key(&entry)) {
        return Err(LuaLoadError::InvalidPackage {
            path: path.to_path_buf(),
            message: format!("package entry '{}' is missing", entry.display()),
        });
    }

    Ok(LoadedPackage { entry, files })
}

fn register_dsl(lua: &Lua, state: Rc<RefCell<LoadState>>) -> mlua::Result<()> {
    let globals = lua.globals();

    let include_state = Rc::clone(&state);
    let include = lua.create_function(move |lua, relative_path: String| {
        include_lua_file(lua, Rc::clone(&include_state), &relative_path)
    })?;
    globals.set("include", include)?;

    let game_state = Rc::clone(&state);
    let game = lua.create_function(move |lua, table: Table| {
        let before_action = table.get::<Option<Function>>("before_action")?;
        let after_action = table.get::<Option<Function>>("after_action")?;
        let on_scene_start = table.get::<Option<Function>>("on_scene_start")?;
        let on_scene_end = table.get::<Option<Function>>("on_scene_end")?;
        let on_chapter = table.get::<Option<Function>>("on_chapter")?;
        let mut state = game_state.borrow_mut();
        state.world.metadata = Some(GameMetadata {
            title: table.get("title")?,
            author: table.get("author")?,
            start: table.get("start")?,
            id: table.get("id")?,
            version: table.get("version")?,
        });
        state.world.settings = table
            .get::<Option<Table>>("settings")?
            .map(table_to_game_settings)
            .transpose()?
            .unwrap_or_default();

        if let Some(before_action) = before_action {
            state.callbacks.before_action = Some(lua.create_registry_value(before_action)?);
        }

        if let Some(after_action) = after_action {
            state.callbacks.after_action = Some(lua.create_registry_value(after_action)?);
        }

        if let Some(on_scene_start) = on_scene_start {
            state.callbacks.on_scene_start = Some(lua.create_registry_value(on_scene_start)?);
        }

        if let Some(on_scene_end) = on_scene_end {
            state.callbacks.on_scene_end = Some(lua.create_registry_value(on_scene_end)?);
        }

        if let Some(on_chapter) = on_chapter {
            state.callbacks.on_chapter = Some(lua.create_registry_value(on_chapter)?);
        }

        Ok(())
    })?;
    globals.set("game", game)?;

    let room_state = Rc::clone(&state);
    let room = lua.create_function(move |lua, id: String| {
        let room_state = Rc::clone(&room_state);
        lua.create_function(move |lua, table: Table| {
            let exits = table
                .get::<Option<Table>>("exits")?
                .map(table_to_exit_map)
                .transpose()?
                .unwrap_or_default();
            let desc = table.get::<Value>("desc")?;

            let room = Room {
                id: id.clone(),
                name: table.get("name")?,
                desc: match &desc {
                    Value::String(desc) => desc.to_str()?.to_string(),
                    Value::Function(_) => String::new(),
                    Value::Nil => String::new(),
                    _ => {
                        return Err(mlua::Error::runtime(
                            "room desc must be a string or function",
                        ));
                    }
                },
                exits,
            };

            let on_enter = table.get::<Option<Function>>("on_enter")?;
            let on_look = table.get::<Option<Function>>("on_look")?;
            let mut state = room_state.borrow_mut();
            state.world.rooms.insert(id.clone(), room);

            if let Value::Function(desc) = desc {
                let callback = lua.create_registry_value(desc)?;
                state.callbacks.room_desc.insert(id.clone(), callback);
            }

            if let Some(on_enter) = on_enter {
                let callback = lua.create_registry_value(on_enter)?;
                state.callbacks.on_enter.insert(id.clone(), callback);
            }

            if let Some(on_look) = on_look {
                let callback = lua.create_registry_value(on_look)?;
                state.callbacks.on_look.insert(id.clone(), callback);
            }

            Ok(())
        })
    })?;
    globals.set("room", room)?;

    let verb_state = Rc::clone(&state);
    let verb = lua.create_function(move |lua, id: String| {
        let verb_state = Rc::clone(&verb_state);
        lua.create_function(move |lua, table: Table| {
            let aliases = table
                .get::<Option<Table>>("aliases")?
                .map(table_to_string_vec)
                .transpose()?
                .unwrap_or_default();
            let on_action = table.get::<Function>("on_action")?;

            let mut state = verb_state.borrow_mut();
            state.world.verbs.insert(
                id.clone(),
                CustomVerb {
                    id: id.clone(),
                    aliases,
                },
            );
            state
                .callbacks
                .verbs
                .insert(id.clone(), lua.create_registry_value(on_action)?);
            Ok(())
        })
    })?;
    globals.set("verb", verb)?;

    let event_state = Rc::clone(&state);
    let event = lua.create_function(move |lua, id: String| {
        let event_state = Rc::clone(&event_state);
        lua.create_function(move |lua, table: Table| {
            let on_trigger = table.get::<Function>("on_trigger")?;
            event_state
                .borrow_mut()
                .callbacks
                .events
                .insert(id.clone(), lua.create_registry_value(on_trigger)?);
            Ok(())
        })
    })?;
    globals.set("event", event)?;

    let thing = lua.create_function(move |lua, id: String| {
        let thing_state = Rc::clone(&state);
        lua.create_function(move |lua, table: Table| {
            let aliases = table
                .get::<Option<Table>>("aliases")?
                .map(table_to_string_vec)
                .transpose()?
                .unwrap_or_default();

            let thing = Thing {
                id: id.clone(),
                name: table.get("name")?,
                aliases,
                location: table.get("location")?,
                portable: table.get::<Option<bool>>("portable")?.unwrap_or(false),
                wearable: table.get::<Option<bool>>("wearable")?.unwrap_or(false),
                actor: table.get::<Option<bool>>("actor")?.unwrap_or(false),
                hidden: table.get::<Option<bool>>("hidden")?.unwrap_or(false),
                desc: table.get("desc")?,
                read: table.get("read")?,
                openable: table.get::<Option<bool>>("openable")?.unwrap_or(false),
                open: table.get::<Option<bool>>("open")?.unwrap_or(false),
                lockable: table.get::<Option<bool>>("lockable")?.unwrap_or(false),
                locked: table.get::<Option<bool>>("locked")?.unwrap_or(false),
                key: table.get("key")?,
                kind: thing_kind(&table)?,
            };

            let on_take = table.get::<Option<Function>>("on_take")?;
            let on_drop = table.get::<Option<Function>>("on_drop")?;
            let on_use = table.get::<Option<Function>>("on_use")?;
            let on_use_with = table.get::<Option<Function>>("on_use_with")?;
            let on_read = table.get::<Option<Function>>("on_read")?;
            let on_open = table.get::<Option<Function>>("on_open")?;
            let on_close = table.get::<Option<Function>>("on_close")?;
            let on_lock = table.get::<Option<Function>>("on_lock")?;
            let on_unlock = table.get::<Option<Function>>("on_unlock")?;
            let on_talk = table.get::<Option<Function>>("on_talk")?;
            let on_show = table.get::<Option<Function>>("on_show")?;
            let on_give = table.get::<Option<Function>>("on_give")?;
            let topics = table.get::<Option<Table>>("topics")?;
            let mut state = thing_state.borrow_mut();
            state.world.things.insert(id.clone(), thing);

            if let Some(on_take) = on_take {
                let callback = lua.create_registry_value(on_take)?;
                state.callbacks.on_take.insert(id.clone(), callback);
            }

            if let Some(on_drop) = on_drop {
                let callback = lua.create_registry_value(on_drop)?;
                state.callbacks.on_drop.insert(id.clone(), callback);
            }

            if let Some(on_use) = on_use {
                let callback = lua.create_registry_value(on_use)?;
                state.callbacks.on_use.insert(id.clone(), callback);
            }

            if let Some(on_use_with) = on_use_with {
                let callback = lua.create_registry_value(on_use_with)?;
                state.callbacks.on_use_with.insert(id.clone(), callback);
            }

            if let Some(on_read) = on_read {
                let callback = lua.create_registry_value(on_read)?;
                state.callbacks.on_read.insert(id.clone(), callback);
            }

            if let Some(on_open) = on_open {
                let callback = lua.create_registry_value(on_open)?;
                state.callbacks.on_open.insert(id.clone(), callback);
            }

            if let Some(on_close) = on_close {
                let callback = lua.create_registry_value(on_close)?;
                state.callbacks.on_close.insert(id.clone(), callback);
            }

            if let Some(on_lock) = on_lock {
                let callback = lua.create_registry_value(on_lock)?;
                state.callbacks.on_lock.insert(id.clone(), callback);
            }

            if let Some(on_unlock) = on_unlock {
                let callback = lua.create_registry_value(on_unlock)?;
                state.callbacks.on_unlock.insert(id.clone(), callback);
            }

            if let Some(on_talk) = on_talk {
                let callback = lua.create_registry_value(on_talk)?;
                state.callbacks.on_talk.insert(id.clone(), callback);
            }

            if let Some(on_show) = on_show {
                let callback = lua.create_registry_value(on_show)?;
                state.callbacks.on_show.insert(id.clone(), callback);
            }

            if let Some(on_give) = on_give {
                let callback = lua.create_registry_value(on_give)?;
                state.callbacks.on_give.insert(id.clone(), callback);
            }

            if let Some(topics) = topics {
                let mut registered_ask_topics = BTreeMap::new();
                let mut registered_tell_topics = BTreeMap::new();
                let mut topic_metadata = BTreeMap::new();

                for pair in topics.pairs::<String, Value>() {
                    let (topic, value) = pair?;
                    match value {
                        Value::Function(callback) => {
                            registered_ask_topics
                                .insert(topic.clone(), lua.create_registry_value(callback)?);
                            topic_metadata.insert(
                                topic.clone(),
                                ActorTopic {
                                    id: topic,
                                    aliases: Vec::new(),
                                    requires: None,
                                },
                            );
                        }
                        Value::Table(table) => {
                            let aliases = table
                                .get::<Option<Table>>("aliases")?
                                .map(table_to_string_vec)
                                .transpose()?
                                .unwrap_or_default();
                            let requires = table.get::<Option<String>>("requires")?;
                            let on_ask =
                                table
                                    .get::<Option<Function>>("on_ask")?
                                    .or(table.get::<Option<Function>>("ask")?);
                            let on_tell =
                                table
                                    .get::<Option<Function>>("on_tell")?
                                    .or(table.get::<Option<Function>>("tell")?);

                            if let Some(on_ask) = on_ask {
                                registered_ask_topics
                                    .insert(topic.clone(), lua.create_registry_value(on_ask)?);
                            }

                            if let Some(on_tell) = on_tell {
                                registered_tell_topics
                                    .insert(topic.clone(), lua.create_registry_value(on_tell)?);
                            }

                            topic_metadata.insert(
                                topic.clone(),
                                ActorTopic {
                                    id: topic,
                                    aliases,
                                    requires,
                                },
                            );
                        }
                        _ => {
                            return Err(mlua::Error::runtime(
                                "actor topic must be a function or table",
                            ));
                        }
                    }
                }

                if !registered_ask_topics.is_empty() {
                    state
                        .callbacks
                        .ask_topics
                        .insert(id.clone(), registered_ask_topics);
                }

                if !registered_tell_topics.is_empty() {
                    state
                        .callbacks
                        .tell_topics
                        .insert(id.clone(), registered_tell_topics);
                }

                if !topic_metadata.is_empty() {
                    state.world.actor_topics.insert(id.clone(), topic_metadata);
                }
            }

            Ok(())
        })
    })?;
    globals.set("thing", thing)?;

    Ok(())
}

fn include_lua_file(
    lua: &Lua,
    state: Rc<RefCell<LoadState>>,
    relative_path: &str,
) -> mlua::Result<()> {
    let path = {
        let state = state.borrow();
        if Path::new(relative_path).is_absolute() {
            return Err(mlua::Error::runtime(
                "include path must be relative to the game directory",
            ));
        }

        let Some(base) = state.include_stack.last().and_then(|path| path.parent()) else {
            return Err(mlua::Error::runtime(
                "include called without an active Lua file",
            ));
        };

        if state.package_files.is_some() {
            normalize_package_path(base.join(relative_path)).map_err(|message| {
                mlua::Error::runtime(format!("failed to include '{relative_path}': {message}"))
            })?
        } else {
            base.join(relative_path)
        }
    };

    let path = if state.borrow().package_files.is_some() {
        path
    } else {
        fs::canonicalize(&path).map_err(|source| {
            mlua::Error::runtime(format!(
                "failed to read included Lua file '{}': {source}",
                path.display()
            ))
        })?
    };

    {
        let state = state.borrow();
        if state.package_files.is_none() {
            ensure_path_in_root(&state.include_root, &path).map_err(|err| {
                mlua::Error::runtime(format!("failed to include '{}': {err}", path.display()))
            })?;
        }

        if state.include_stack.contains(&path) {
            return Err(mlua::Error::runtime(format!(
                "cyclic include of '{}'",
                path.display()
            )));
        }
    }

    load_lua_file(lua, state, path).map_err(mlua::Error::external)
}

fn load_lua_file(
    lua: &Lua,
    state: Rc<RefCell<LoadState>>,
    path: PathBuf,
) -> Result<(), LuaLoadError> {
    {
        let mut state = state.borrow_mut();

        if !state.included_files.insert(path.clone()) {
            return Ok(());
        }

        state.include_stack.push(path.clone());
    }

    let result = (|| {
        let source = {
            let state = state.borrow();

            if let Some(files) = &state.package_files {
                let key = package_path_key(&path);
                let Some(bytes) = files.get(&key) else {
                    return Err(LuaLoadError::InvalidPackage {
                        path: state.include_root.clone(),
                        message: format!("missing Lua file '{}'", path.display()),
                    });
                };

                String::from_utf8(bytes.clone()).map_err(|_| LuaLoadError::InvalidPackage {
                    path: state.include_root.clone(),
                    message: format!("Lua file '{}' is not valid UTF-8", path.display()),
                })?
            } else {
                fs::read_to_string(&path).map_err(|source| LuaLoadError::Io {
                    path: path.clone(),
                    source,
                })?
            }
        };

        lua.load(&source)
            .set_name(path.display().to_string())
            .exec()
            .map_err(|source| LuaLoadError::Lua {
                path: path.clone(),
                source,
            })
    })();

    state.borrow_mut().include_stack.pop();
    result
}

fn ensure_path_in_root(root: &Path, path: &Path) -> Result<(), LuaLoadError> {
    if path.starts_with(root) {
        return Ok(());
    }

    Err(LuaLoadError::IncludeEscapesRoot {
        include: path.to_path_buf(),
        root: root.to_path_buf(),
    })
}

fn normalize_package_path(path: impl AsRef<Path>) -> Result<PathBuf, String> {
    let path = path.as_ref();

    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("package paths must be relative".to_string());
    }

    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(format!(
                    "package path '{}' escapes the root",
                    path.display()
                ));
            }
            _ => return Err(format!("invalid package path '{}'", path.display())),
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err("package paths must not be empty".to_string());
    }

    Ok(normalized)
}

fn package_path_key(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }

    encoded
}

fn hex_decode(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err("hex package payload has an odd length".to_string());
    }

    let mut bytes = Vec::with_capacity(text.len() / 2);
    let mut chars = text.bytes();

    while let Some(high) = chars.next() {
        let low = chars.next().expect("hex length was checked");
        let high = hex_value(high)?;
        let low = hex_value(low)?;
        bytes.push((high << 4) | low);
    }

    Ok(bytes)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("hex package payload contains a non-hex character".to_string()),
    }
}

fn table_to_exit_map(table: Table) -> mlua::Result<BTreeMap<String, Exit>> {
    let mut exits = BTreeMap::new();

    for pair in table.pairs::<String, Value>() {
        let (direction, value) = pair?;
        let exit = match value {
            Value::String(to) => Exit::open(to.to_str()?.to_string()),
            Value::Table(table) => Exit {
                to: table.get("to")?,
                requires: table.get("requires")?,
                locked_msg: table.get("locked_msg")?,
            },
            _ => {
                return Err(mlua::Error::runtime(
                    "exit values must be a room id string or table",
                ));
            }
        };
        exits.insert(direction, exit);
    }

    Ok(exits)
}

fn table_to_game_settings(table: Table) -> mlua::Result<GameSettings> {
    Ok(GameSettings {
        exits: table
            .get::<Option<Table>>("exits")?
            .map(table_to_exit_display_settings)
            .transpose()?
            .unwrap_or_default(),
    })
}

fn table_to_exit_display_settings(table: Table) -> mlua::Result<ExitDisplaySettings> {
    let defaults = ExitDisplaySettings::default();

    Ok(ExitDisplaySettings {
        show: table.get::<Option<bool>>("show")?.unwrap_or(defaults.show),
        label: table
            .get::<Option<String>>("label")?
            .unwrap_or(defaults.label),
    })
}

fn table_to_string_vec(table: Table) -> mlua::Result<Vec<String>> {
    table.sequence_values::<String>().collect()
}

fn thing_kind(table: &Table) -> mlua::Result<ThingKind> {
    let container = table.get::<Option<bool>>("container")?.unwrap_or(false);
    let supporter = table.get::<Option<bool>>("supporter")?.unwrap_or(false);

    match (container, supporter) {
        (true, true) => Err(mlua::Error::runtime(
            "thing cannot be both container and supporter",
        )),
        (true, false) => Ok(ThingKind::Container),
        (false, true) => Ok(ThingKind::Supporter),
        (false, false) => Ok(ThingKind::Object),
    }
}

fn append_output(mut first: String, second: String) -> String {
    if first.is_empty() {
        return second;
    }

    if !second.is_empty() {
        first.push_str("\n\n");
        first.push_str(&second);
    }

    first
}

fn add_callback_names(
    callbacks: &mut BTreeMap<String, Vec<&'static str>>,
    source: &BTreeMap<String, RegistryKey>,
    name: &'static str,
) {
    for id in source.keys() {
        callbacks.entry(id.clone()).or_default().push(name);
    }
}

fn topic_callback_summary(
    source: &BTreeMap<String, BTreeMap<String, RegistryKey>>,
) -> BTreeMap<String, Vec<String>> {
    source
        .iter()
        .map(|(actor_id, topics)| (actor_id.clone(), topics.keys().cloned().collect()))
        .collect()
}

fn is_quit_command(input: &str) -> bool {
    matches!(
        input.split_whitespace().next().unwrap_or_default(),
        "quit" | "exit"
    )
}

fn command_advances_turn(input: &str) -> bool {
    let verb = input.split_whitespace().next().unwrap_or_default();

    !matches!(
        verb,
        "" | "look"
            | "l"
            | "examine"
            | "x"
            | "inventory"
            | "inv"
            | "i"
            | "quit"
            | "exit"
            | "again"
            | "g"
            | "undo"
    )
}

fn normalized_command(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf, process};

    const DIALOGUE_TEST_GAME: &str = r#"
game {
  title = "Dialogue Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A test room."
}

thing "coin" {
  name = "silver coin",
  aliases = { "coin" },
  location = "start",
  portable = true
}

thing "caretaker" {
  name = "caretaker",
  aliases = { "caretaker" },
  location = "start",
  portable = false,
  actor = true,

  on_show = function(game, item)
    game.say("The caretaker studies the " .. item .. ".")
  end,

  on_give = function(game, item)
    game.move(item, "caretaker")
    game.say("The caretaker accepts the " .. item .. ".")
  end,

  topics = {
    coin = {
      aliases = { "silver coin" },

      ask = function(game, topic)
        local count = game.actor_memory("caretaker", "asked:" .. topic)
        game.say("Ask count: " .. count .. ".")
      end,

      tell = function(game, topic)
        game.say("The caretaker remembers " .. topic .. ".")
      end
    },

    house = {
      aliases = { "glass house" },
      requires = "knows_house",

      ask = function(game)
        game.say("The house has been waiting.")
      end
    }
  }
}
"#;

    const SAVE_ID_TEST_GAME: &str = r#"
game {
  id = "GAME_ID",
  title = "Save Identity Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A test room."
}
"#;

    #[test]
    fn house_uses_flags_callbacks_and_custom_verbs() {
        let game_file =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/house/game.lua");
        let mut game = LuaGame::load(game_file).expect("example loads");

        let CommandResult::Continue(outcome) = game
            .handle_command("listen")
            .expect("before action succeeds")
        else {
            panic!("before action should continue");
        };
        assert_eq!(
            outcome.output,
            "Rain ticks against the glass with patient fingers."
        );

        let CommandResult::Continue(outcome) = game
            .handle_command("take key")
            .expect("take command succeeds")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "The key is colder than it should be.");

        let CommandResult::Continue(outcome) =
            game.handle_command("look").expect("look command succeeds")
        else {
            panic!("look should continue");
        };
        assert!(outcome.output.contains("The table is bare now."));

        let CommandResult::Continue(outcome) = game
            .handle_command("polish key")
            .expect("custom verb succeeds")
        else {
            panic!("custom verb should continue");
        };
        assert!(outcome.output.contains("You polish the key"));

        let CommandResult::Continue(outcome) =
            game.handle_command("use key").expect("use succeeds")
        else {
            panic!("use should continue");
        };
        assert!(outcome.output.contains("polished key warms"));
        assert!(outcome.output.contains("house seems to listen"));

        let CommandResult::Continue(outcome) =
            game.handle_command("north").expect("movement succeeds")
        else {
            panic!("movement should continue");
        };
        assert!(outcome.output.contains("polished key throws a small gleam"));
        assert!(outcome.output.contains("floorboards answer"));

        let CommandResult::Continue(outcome) = game
            .handle_command("talk to caretaker")
            .expect("talk succeeds")
        else {
            panic!("talk should continue");
        };
        assert!(outcome.output.contains("That key remembers more doors"));

        let CommandResult::Continue(outcome) = game
            .handle_command("ask caretaker about key")
            .expect("ask succeeds")
        else {
            panic!("ask should continue");
        };
        assert!(outcome.output.contains("cut for the study"));

        let CommandResult::Continue(outcome) = game
            .handle_command("flip key")
            .expect("random verb succeeds")
        else {
            panic!("random verb should continue");
        };
        assert_eq!(outcome.output, "The key lands teeth-up in your palm.");
    }

    #[test]
    fn again_and_undo_include_lua_callback_state() {
        let game_file =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/house/game.lua");
        let mut game = LuaGame::load(game_file).expect("example loads");

        game.handle_command("take key").expect("take succeeds");
        assert!(game.game.has("brass_key"));
        assert!(game.game.has_flag("touched_key"));

        let CommandResult::Continue(outcome) = game.handle_command("undo").expect("undo succeeds")
        else {
            panic!("undo should continue");
        };
        assert_eq!(outcome.output, "Undone.");
        assert!(!game.game.has("brass_key"));
        assert!(!game.game.has_flag("touched_key"));

        let CommandResult::Continue(outcome) =
            game.handle_command("again").expect("again succeeds")
        else {
            panic!("again should continue");
        };
        assert_eq!(outcome.output, "The key is colder than it should be.");
        assert!(game.game.has("brass_key"));
        assert!(game.game.has_flag("touched_key"));
    }

    #[test]
    fn game_settings_can_enable_exit_display() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-settings-test-{}.lua", process::id()));
        fs::write(
            &game_file,
            r#"
game {
  title = "Settings Test",
  start = "start",
  settings = {
    exits = {
      show = true,
      label = "Paths"
    }
  }
}

room "start" {
  name = "Start",
  desc = "A test room.",
  exits = {
    east = "east_room",
    north = "north_room"
  }
}

room "east_room" {
  name = "East",
  desc = "East."
}

room "north_room" {
  name = "North",
  desc = "North."
}
"#,
        )
        .expect("test game file should write");

        let mut game = LuaGame::load(&game_file).expect("settings game loads");
        let opening = game.opening().expect("opening renders");

        assert!(opening.contains("Paths: east, north."));
    }

    #[test]
    fn game_can_include_project_local_lua_files() {
        let game_dir =
            std::env::temp_dir().join(format!("moonroom-include-test-{}", process::id()));
        fs::create_dir_all(&game_dir).expect("test game dir should create");
        fs::write(
            game_dir.join("game.lua"),
            r#"
game {
  title = "Include Test",
  start = "start"
}

include "rooms.lua"
include "things.lua"
"#,
        )
        .expect("entrypoint should write");
        fs::write(
            game_dir.join("rooms.lua"),
            r#"
room "start" {
  name = "Start",
  desc = "A split room."
}
"#,
        )
        .expect("rooms file should write");
        fs::write(
            game_dir.join("things.lua"),
            r#"
thing "coin" {
  name = "coin",
  aliases = { "coin" },
  location = "start",
  portable = true
}
"#,
        )
        .expect("things file should write");

        let game = LuaGame::load(game_dir.join("game.lua")).expect("split game should load");

        assert!(game.game.world().rooms.contains_key("start"));
        assert!(game.game.world().things.contains_key("coin"));
    }

    #[test]
    fn moon_packages_load_project_local_includes() {
        let game_dir =
            std::env::temp_dir().join(format!("moonroom-package-test-{}", process::id()));
        let package_path = game_dir.with_extension("moon");
        let unpacked_dir =
            game_dir.with_file_name(format!("moonroom-package-unpack-test-{}", process::id()));
        fs::create_dir_all(&game_dir).expect("test game dir should create");
        fs::write(
            game_dir.join("game.lua"),
            r#"
game {
  title = "Package Test",
  start = "start"
}

include "rooms/start.lua"
"#,
        )
        .expect("entrypoint should write");
        fs::create_dir_all(game_dir.join("rooms")).expect("rooms dir should create");
        fs::write(
            game_dir.join("rooms").join("start.lua"),
            r#"
room "start" {
  name = "Start",
  desc = "A packaged room."
}
"#,
        )
        .expect("room file should write");

        pack_game_directory(&game_dir, &package_path).expect("package should write");
        let mut game = LuaGame::load_source(GameSource::Package(package_path.clone()))
            .expect("packaged game should load");
        let opening = game.opening().expect("opening should render");

        assert!(opening.contains("A packaged room."));
        assert!(
            package_file_names(&package_path)
                .expect("package files should list")
                .contains(&"moon.json".to_string())
        );

        unpack_game_package(&package_path, &unpacked_dir).expect("package should unpack");
        assert!(unpacked_dir.join("game.lua").exists());
        assert!(unpacked_dir.join("moon.json").exists());

        let _ = fs::remove_dir_all(game_dir);
        let _ = fs::remove_file(package_path);
        let _ = fs::remove_dir_all(unpacked_dir);
    }

    #[test]
    fn things_can_define_read_text_and_read_callbacks() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-read-test-{}.lua", process::id()));
        fs::write(
            &game_file,
            r#"
game {
  title = "Read Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A test room."
}

thing "note" {
  name = "paper note",
  aliases = { "note" },
  location = "start",
  portable = true,
  desc = "A folded paper note.",
  read = "The note says hello.",

  on_read = function(game)
    game.flag("note_read")
  end
}
"#,
        )
        .expect("test game file should write");

        let mut game = LuaGame::load(&game_file).expect("read game loads");
        let CommandResult::Continue(outcome) =
            game.handle_command("read note").expect("read succeeds")
        else {
            panic!("read should continue");
        };

        assert_eq!(outcome.output, "The note says hello.");
        assert!(game.game.has_flag("note_read"));
    }

    #[test]
    fn things_can_define_open_lock_state_and_callbacks() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-open-test-{}.lua", process::id()));
        fs::write(
            &game_file,
            r#"
game {
  title = "Open Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A test room."
}

thing "brass_key" {
  name = "brass key",
  aliases = { "key" },
  location = "start",
  portable = true
}

thing "chest" {
  name = "cedar chest",
  aliases = { "chest" },
  location = "start",
  portable = false,
  container = true,
  openable = true,
  open = false,
  lockable = true,
  locked = true,
  key = "brass_key",

  on_open = function(game)
    game.flag("chest_opened")
  end,

  on_close = function(game)
    game.flag("chest_closed")
  end,

  on_lock = function(game)
    game.flag("chest_locked")
  end,

  on_unlock = function(game)
    game.flag("chest_unlocked")
  end
}
"#,
        )
        .expect("test game file should write");

        let mut game = LuaGame::load(&game_file).expect("open game loads");
        game.handle_command("take key").expect("take succeeds");
        game.handle_command("unlock chest with key")
            .expect("unlock succeeds");
        game.handle_command("open chest").expect("open succeeds");
        game.handle_command("close chest").expect("close succeeds");
        game.handle_command("lock chest").expect("lock succeeds");

        assert!(game.game.has_flag("chest_unlocked"));
        assert!(game.game.has_flag("chest_opened"));
        assert!(game.game.has_flag("chest_closed"));
        assert!(game.game.has_flag("chest_locked"));
    }

    #[test]
    fn things_can_define_use_with_callbacks() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-use-with-test-{}.lua", process::id()));
        fs::write(
            &game_file,
            r#"
game {
  title = "Use With Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A test room."
}

thing "key" {
  name = "brass key",
  aliases = { "key" },
  location = "start",
  portable = true,

  on_use_with = function(game, item, target)
    game.flag("used:" .. item .. ":" .. target)
    game.say("You use the " .. item .. " on the " .. target .. ".")
  end
}

thing "door" {
  name = "green door",
  aliases = { "door" },
  location = "start",
  portable = false
}
"#,
        )
        .expect("test game file should write");

        let mut game = LuaGame::load(&game_file).expect("use-with game loads");
        let CommandResult::Continue(outcome) = game
            .handle_command("use key on door")
            .expect("use-with succeeds")
        else {
            panic!("use-with should continue");
        };

        assert_eq!(outcome.output, "You use the key on the door.");
        assert!(game.game.has_flag("used:key:door"));
    }

    #[test]
    fn callbacks_can_reveal_hidden_things() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-hidden-test-{}.lua", process::id()));
        fs::write(
            &game_file,
            r#"
game {
  title = "Hidden Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A test room."
}

thing "note" {
  name = "hidden note",
  aliases = { "note" },
  location = "start",
  portable = true,
  hidden = true
}

verb "search" {
  on_action = function(game, input)
    if game.visible("note") then
      game.say("You already found the note.")
    else
      game.reveal("note")
      game.say("You find a hidden note.")
    end
  end
}
"#,
        )
        .expect("test game file should write");

        let mut game = LuaGame::load(&game_file).expect("hidden game loads");
        let opening = game.opening().expect("opening renders");
        assert!(!opening.contains("hidden note"));

        let CommandResult::Continue(outcome) =
            game.handle_command("take note").expect("take command")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "You don't see that here.");

        let CommandResult::Continue(outcome) =
            game.handle_command("search").expect("search command")
        else {
            panic!("search should continue");
        };
        assert_eq!(outcome.output, "You find a hidden note.");
        assert!(game.game.visible("note"));

        let CommandResult::Continue(outcome) =
            game.handle_command("take note").expect("take command")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "You take the hidden note.");
    }

    #[test]
    fn actors_can_use_richer_dialogue_callbacks_and_memory() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-dialogue-test-{}.lua", process::id()));
        fs::write(&game_file, DIALOGUE_TEST_GAME).expect("test game file should write");

        let mut game = LuaGame::load(&game_file).expect("dialogue game loads");
        game.handle_command("take coin").expect("take succeeds");

        let CommandResult::Continue(outcome) = game
            .handle_command("ask caretaker about silver coin")
            .expect("ask succeeds")
        else {
            panic!("ask should continue");
        };
        assert_eq!(outcome.output, "Ask count: 1.");
        assert_eq!(game.game.actor_memory("caretaker", "asked:coin"), 1);

        let CommandResult::Continue(outcome) = game
            .handle_command("tell caretaker about coin")
            .expect("tell succeeds")
        else {
            panic!("tell should continue");
        };
        assert_eq!(outcome.output, "The caretaker remembers coin.");

        let CommandResult::Continue(outcome) = game
            .handle_command("ask caretaker about glass house")
            .expect("gated ask succeeds")
        else {
            panic!("ask should continue");
        };
        assert_eq!(
            outcome.output,
            "The caretaker has nothing to say about house."
        );

        game.game.flag("knows_house");
        let CommandResult::Continue(outcome) = game
            .handle_command("ask caretaker about house")
            .expect("available ask succeeds")
        else {
            panic!("ask should continue");
        };
        assert_eq!(outcome.output, "The house has been waiting.");

        let CommandResult::Continue(outcome) = game
            .handle_command("show coin to caretaker")
            .expect("show succeeds")
        else {
            panic!("show should continue");
        };
        assert_eq!(outcome.output, "The caretaker studies the coin.");
        assert_eq!(game.game.actor_memory("caretaker", "shown:coin"), 1);

        let CommandResult::Continue(outcome) = game
            .handle_command("give coin to caretaker")
            .expect("give succeeds")
        else {
            panic!("give should continue");
        };
        assert_eq!(outcome.output, "The caretaker accepts the coin.");
        assert_eq!(game.game.actor_memory("caretaker", "given:coin"), 1);
        assert!(!game.game.has("coin"));
    }

    #[test]
    fn callbacks_can_manage_scenes_chapters_and_scene_timers() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-scene-test-{}.lua", process::id()));
        fs::write(
            &game_file,
            r#"
game {
  title = "Scene Test",
  start = "start",

  on_scene_start = function(game, scene)
    game.say("Scene started: " .. scene .. ".")
  end,

  on_scene_end = function(game, scene)
    game.say("Scene ended: " .. scene .. ".")
  end,

  on_chapter = function(game, chapter)
    game.say("Chapter: " .. chapter .. ".")
  end
}

room "start" {
  name = "Start",
  desc = "A test room."
}

verb "begin" {
  on_action = function(game)
    game.chapter("arrival")
    game.start_scene("opening")
    game.schedule_scene(1, "bell")
    game.say("The scene begins.")
  end
}

verb "wait" {
  on_action = function(game)
    game.say("You wait in " .. game.scene() .. ".")
  end
}

verb "leave" {
  on_action = function(game)
    game.end_scene("opening")
  end
}

event "bell" {
  on_trigger = function(game)
    game.say("A scene bell rings.")
  end
}
"#,
        )
        .expect("test game file should write");

        let mut game = LuaGame::load(&game_file).expect("scene game loads");
        let CommandResult::Continue(outcome) =
            game.handle_command("begin").expect("begin succeeds")
        else {
            panic!("begin should continue");
        };
        assert_eq!(
            outcome.output,
            "The scene begins.\nChapter: arrival.\nScene started: opening."
        );
        assert_eq!(game.current_chapter(), Some("arrival"));
        assert_eq!(game.current_scene(), Some("opening"));

        let CommandResult::Continue(outcome) = game.handle_command("wait").expect("wait succeeds")
        else {
            panic!("wait should continue");
        };
        assert_eq!(
            outcome.output,
            "You wait in opening.\n\nA scene bell rings."
        );

        let CommandResult::Continue(outcome) =
            game.handle_command("leave").expect("leave succeeds")
        else {
            panic!("leave should continue");
        };
        assert_eq!(outcome.output, "Scene ended: opening.");
        assert_eq!(game.current_scene(), None);
    }

    #[test]
    fn save_files_are_versioned_and_keep_loading_legacy_state() {
        let game_file =
            std::env::temp_dir().join(format!("moonroom-save-test-{}.lua", process::id()));
        fs::write(
            &game_file,
            r#"
game {
  id = "save-test",
  version = "1.0.0",
  title = "Save Test",
  start = "start"
}

room "start" {
  name = "Start",
  desc = "A test room."
}

thing "coin" {
  name = "coin",
  aliases = { "coin" },
  location = "start",
  portable = true
}
"#,
        )
        .expect("test game file should write");

        let save_file =
            std::env::temp_dir().join(format!("moonroom-save-test-{}.json", process::id()));
        let legacy_file =
            std::env::temp_dir().join(format!("moonroom-legacy-save-test-{}.json", process::id()));
        let compact_file =
            std::env::temp_dir().join(format!("moonroom-compact-save-test-{}.json", process::id()));

        let mut game = LuaGame::load(&game_file).expect("save game loads");
        game.handle_command("take coin").expect("take succeeds");
        game.save_to_path(&save_file).expect("save succeeds");

        let saved = fs::read_to_string(&save_file).expect("save should read");
        assert!(saved.contains(r#""format": "moonroom.save""#));
        assert!(saved.contains(r#""version": 1"#));
        assert!(saved.contains(r#""id": "save-test""#));

        let mut loaded = LuaGame::load(&game_file).expect("save game reloads");
        loaded.load_from_path(&save_file).expect("load succeeds");
        assert!(loaded.game.has("coin"));

        fs::write(
            &legacy_file,
            serde_json::to_string(game.game.state()).expect("legacy state should encode"),
        )
        .expect("legacy save should write");
        let mut legacy_loaded = LuaGame::load(&game_file).expect("save game reloads");
        legacy_loaded
            .load_from_path(&legacy_file)
            .expect("legacy load succeeds");
        assert!(legacy_loaded.game.has("coin"));

        game.save_to_path_with_mode(&compact_file, SaveOutputMode::Compact)
            .expect("compact save succeeds");
        let compact = fs::read_to_string(&compact_file).expect("compact save should read");
        assert!(!compact.contains('\n'));
    }

    #[test]
    fn save_files_reject_different_game_ids() {
        let first_game_file =
            std::env::temp_dir().join(format!("moonroom-save-first-{}.lua", process::id()));
        let second_game_file =
            std::env::temp_dir().join(format!("moonroom-save-second-{}.lua", process::id()));
        let save_file =
            std::env::temp_dir().join(format!("moonroom-wrong-game-save-{}.json", process::id()));

        fs::write(
            &first_game_file,
            SAVE_ID_TEST_GAME.replace("GAME_ID", "first"),
        )
        .expect("first game should write");
        fs::write(
            &second_game_file,
            SAVE_ID_TEST_GAME.replace("GAME_ID", "second"),
        )
        .expect("second game should write");

        let game = LuaGame::load(&first_game_file).expect("first game loads");
        game.save_to_path(&save_file).expect("save succeeds");

        let mut other_game = LuaGame::load(&second_game_file).expect("second game loads");
        let err = other_game
            .load_from_path(&save_file)
            .expect_err("wrong game save should fail");

        assert!(err.to_string().contains("belongs to game 'first'"));
    }

    #[test]
    fn include_rejects_files_outside_game_directory() {
        let game_dir =
            std::env::temp_dir().join(format!("moonroom-include-escape-test-{}", process::id()));
        fs::create_dir_all(&game_dir).expect("test game dir should create");
        fs::write(
            std::env::temp_dir().join("moonroom-outside.lua"),
            r#"room "outside" { name = "Outside", desc = "Outside." }"#,
        )
        .expect("outside file should write");
        fs::write(
            game_dir.join("game.lua"),
            r#"
game {
  title = "Escape Test",
  start = "outside"
}

include "../moonroom-outside.lua"
"#,
        )
        .expect("entrypoint should write");

        let Err(err) = LuaGame::load(game_dir.join("game.lua")) else {
            panic!("include should fail");
        };

        assert!(err.to_string().contains("escapes game directory"));
    }
}
