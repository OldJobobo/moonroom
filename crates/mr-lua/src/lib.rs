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

#[derive(Debug, Default)]
struct LoadState {
    world: World,
    callbacks: CallbackRegistry,
    include_root: PathBuf,
    include_stack: Vec<PathBuf>,
    included_files: BTreeSet<PathBuf>,
}

#[derive(Debug, Default)]
struct CallbackRegistry {
    before_action: Option<RegistryKey>,
    after_action: Option<RegistryKey>,
    room_desc: BTreeMap<String, RegistryKey>,
    on_enter: BTreeMap<String, RegistryKey>,
    on_look: BTreeMap<String, RegistryKey>,
    on_take: BTreeMap<String, RegistryKey>,
    on_drop: BTreeMap<String, RegistryKey>,
    on_use: BTreeMap<String, RegistryKey>,
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

#[derive(Debug, Clone)]
enum ScriptCommand {
    Flag(String),
    ClearFlag(String),
    SetCounter(String, i64),
    MoveThing(String, String),
    HideThing(String),
    RevealThing(String),
    Goto(String),
    Schedule(u64, String),
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
            visited_rooms: game.state().visited_rooms.clone(),
            random_state: game.state().random_state,
        }
    }
}

impl LuaGame {
    const UNDO_LIMIT: usize = 20;

    pub fn load(path: impl AsRef<Path>) -> Result<Self, LuaLoadError> {
        let loaded = load_game_data(path)?;
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

    pub fn has_flag(&self, name: &str) -> bool {
        self.game.has_flag(name)
    }

    pub fn counter(&self, name: &str) -> i64 {
        self.game.counter(name)
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<(), LuaRunError> {
        let path = path.as_ref();
        let json = serde_json::to_string_pretty(self.game.state()).map_err(|source| {
            LuaRunError::EncodeSave {
                path: path.to_path_buf(),
                source,
            }
        })?;

        fs::write(path, json).map_err(|source| LuaRunError::WriteSave {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn load_from_path(&mut self, path: impl AsRef<Path>) -> Result<(), LuaRunError> {
        let path = path.as_ref();
        let json = fs::read_to_string(path).map_err(|source| LuaRunError::ReadSave {
            path: path.to_path_buf(),
            source,
        })?;
        let state =
            serde_json::from_str::<GameState>(&json).map_err(|source| LuaRunError::ParseSave {
                path: path.to_path_buf(),
                source,
            })?;

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
                ScriptCommand::Schedule(turns, event_name) => {
                    self.game.schedule_event(turns, event_name);
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
        let mut state = game_state.borrow_mut();
        state.world.metadata = Some(GameMetadata {
            title: table.get("title")?,
            author: table.get("author")?,
            start: table.get("start")?,
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

        base.join(relative_path)
    };

    let path = fs::canonicalize(&path).map_err(|source| {
        mlua::Error::runtime(format!(
            "failed to read included Lua file '{}': {source}",
            path.display()
        ))
    })?;

    {
        let state = state.borrow();
        ensure_path_in_root(&state.include_root, &path).map_err(|err| {
            mlua::Error::runtime(format!("failed to include '{}': {err}", path.display()))
        })?;

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
        let source = fs::read_to_string(&path).map_err(|source| LuaLoadError::Io {
            path: path.clone(),
            source,
        })?;

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
        "" | "look" | "l" | "inventory" | "inv" | "i" | "quit" | "exit" | "again" | "g" | "undo"
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
