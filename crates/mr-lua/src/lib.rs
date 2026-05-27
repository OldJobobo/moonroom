use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    rc::Rc,
};

use mlua::{Function, Lua, RegistryKey, Table, Value};
use mr_core::{
    CommandOutcome, CommandResult, CustomVerb, Exit, ExitDisplaySettings, Game, GameError,
    GameEvent, GameMetadata, GameSettings, GameState, Room, Thing, ThingKind, World,
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
    on_talk: BTreeMap<String, RegistryKey>,
    ask_topics: BTreeMap<String, BTreeMap<String, RegistryKey>>,
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
    Talk,
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
    Goto(String),
    Schedule(u64, String),
    Cancel(String),
    SetRandomState(u64),
}

#[derive(Debug, Clone)]
struct ScriptSession {
    output: Vec<String>,
    commands: Vec<ScriptCommand>,
    flags: BTreeSet<String>,
    counters: BTreeMap<String, i64>,
    inventory: BTreeSet<String>,
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
            inventory: game.state().inventory.clone(),
            current_room: game.state().current_room.clone(),
            visited_rooms: game.state().visited_rooms.clone(),
            random_state: game.state().random_state,
        }
    }
}

impl LuaGame {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, LuaLoadError> {
        let loaded = load_game_data(path)?;
        let game = Game::new(loaded.world)?;

        Ok(Self {
            lua: loaded.lua,
            game,
            callbacks: loaded.callbacks,
        })
    }

    pub fn welcome(&self) -> Result<String, LuaRunError> {
        // Kept as a fallback proof that core can still render a static opening.
        self.game.welcome().map_err(Into::into)
    }

    pub fn opening(&mut self) -> Result<String, LuaRunError> {
        self.render_room()
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
        Ok(())
    }

    pub fn handle_command(&mut self, input: &str) -> Result<CommandResult, LuaRunError> {
        let normalized = normalized_command(input);

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

                Ok(CommandResult::Continue(outcome))
            }
            CommandResult::Quit(output) => Ok(CommandResult::Quit(output)),
        }
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
        function.call::<()>(api).map_err(LuaRunError::Lua)?;
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
                ScriptCommand::Goto(room_id) => self.game.goto(&room_id)?,
                ScriptCommand::Schedule(turns, event_name) => {
                    self.game.schedule_event(turns, event_name);
                }
                ScriptCommand::Cancel(event_name) => self.game.cancel_event(&event_name),
                ScriptCommand::SetRandomState(random_state) => {
                    self.game.set_random_state(random_state);
                }
            }
        }

        Ok(())
    }
}

fn load_game_data(path: impl AsRef<Path>) -> Result<LoadedGame, LuaLoadError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| LuaLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let lua = Lua::new();
    let state = Rc::new(RefCell::new(LoadState::default()));

    register_dsl(&lua, Rc::clone(&state)).map_err(|source| LuaLoadError::Lua {
        path: path.to_path_buf(),
        source,
    })?;

    lua.load(&source)
        .set_name(path.display().to_string())
        .exec()
        .map_err(|source| LuaLoadError::Lua {
            path: path.to_path_buf(),
            source,
        })?;

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
                desc: table.get("desc")?,
                kind: thing_kind(&table)?,
            };

            let on_take = table.get::<Option<Function>>("on_take")?;
            let on_drop = table.get::<Option<Function>>("on_drop")?;
            let on_use = table.get::<Option<Function>>("on_use")?;
            let on_talk = table.get::<Option<Function>>("on_talk")?;
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

            if let Some(on_talk) = on_talk {
                let callback = lua.create_registry_value(on_talk)?;
                state.callbacks.on_talk.insert(id.clone(), callback);
            }

            if let Some(topics) = topics {
                let mut registered_topics = BTreeMap::new();

                for pair in topics.pairs::<String, Function>() {
                    let (topic, callback) = pair?;
                    registered_topics.insert(topic, lua.create_registry_value(callback)?);
                }

                state
                    .callbacks
                    .ask_topics
                    .insert(id.clone(), registered_topics);
            }

            Ok(())
        })
    })?;
    globals.set("thing", thing)?;

    Ok(())
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
}
