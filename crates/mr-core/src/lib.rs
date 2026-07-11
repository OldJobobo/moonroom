use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameMetadata {
    pub title: String,
    pub author: Option<String>,
    pub start: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSettings {
    pub exits: ExitDisplaySettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitDisplaySettings {
    pub show: bool,
    pub label: String,
}

impl Default for ExitDisplaySettings {
    fn default() -> Self {
        Self {
            show: false,
            label: "Available exits".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub exits: BTreeMap<String, Exit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exit {
    pub to: String,
    pub requires: Option<String>,
    pub locked_msg: Option<String>,
}

impl Exit {
    pub fn open(to: impl Into<String>) -> Self {
        Self {
            to: to.into(),
            requires: None,
            locked_msg: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomVerb {
    pub id: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorTopic {
    pub id: String,
    pub aliases: Vec<String>,
    pub requires: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Thing {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub location: String,
    pub portable: bool,
    pub wearable: bool,
    pub actor: bool,
    pub hidden: bool,
    pub desc: Option<String>,
    pub read: Option<String>,
    pub openable: bool,
    pub open: bool,
    pub lockable: bool,
    pub locked: bool,
    pub key: Option<String>,
    pub kind: ThingKind,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThingKind {
    #[default]
    Object,
    Container,
    Supporter,
}

impl ThingKind {
    fn preposition(&self) -> &'static str {
        match self {
            ThingKind::Object | ThingKind::Container => "in",
            ThingKind::Supporter => "on",
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct World {
    pub metadata: Option<GameMetadata>,
    pub settings: GameSettings,
    pub rooms: BTreeMap<String, Room>,
    pub things: BTreeMap<String, Thing>,
    pub verbs: BTreeMap<String, CustomVerb>,
    #[serde(default)]
    pub actor_topics: BTreeMap<String, BTreeMap<String, ActorTopic>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameState {
    pub current_room: String,
    #[serde(default)]
    pub current_scene: Option<String>,
    #[serde(default)]
    pub current_chapter: Option<String>,
    #[serde(default)]
    pub visited_rooms: BTreeSet<String>,
    pub inventory: BTreeSet<String>,
    pub worn: BTreeSet<String>,
    pub thing_locations: BTreeMap<String, String>,
    #[serde(default)]
    pub open_things: BTreeSet<String>,
    #[serde(default)]
    pub locked_things: BTreeSet<String>,
    #[serde(default)]
    pub hidden_things: BTreeSet<String>,
    pub flags: BTreeSet<String>,
    pub counters: BTreeMap<String, i64>,
    #[serde(default)]
    pub actor_memory: BTreeMap<String, BTreeMap<String, i64>>,
    pub timers: Vec<ScheduledEvent>,
    #[serde(default = "GameState::default_random_seed")]
    pub random_seed: u64,
    #[serde(default = "GameState::default_random_seed")]
    pub random_state: u64,
    pub turn: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledEvent {
    pub name: String,
    pub due_turn: u64,
    #[serde(default)]
    pub scene: Option<String>,
}

impl GameState {
    pub const DEFAULT_RANDOM_SEED: u64 = 0x4d6f_6f6e_726f_6f6d;

    pub fn default_random_seed() -> u64 {
        Self::DEFAULT_RANDOM_SEED
    }

    pub fn new(start_room: impl Into<String>, things: &BTreeMap<String, Thing>) -> Self {
        let start_room = start_room.into();

        Self {
            current_room: start_room.clone(),
            current_scene: None,
            current_chapter: None,
            visited_rooms: BTreeSet::from([start_room]),
            inventory: BTreeSet::new(),
            worn: BTreeSet::new(),
            thing_locations: things
                .iter()
                .map(|(id, thing)| (id.clone(), thing.location.clone()))
                .collect(),
            open_things: things
                .iter()
                .filter(|(_, thing)| thing.open)
                .map(|(id, _)| id.clone())
                .collect(),
            locked_things: things
                .iter()
                .filter(|(_, thing)| thing.locked)
                .map(|(id, _)| id.clone())
                .collect(),
            hidden_things: things
                .iter()
                .filter(|(_, thing)| thing.hidden)
                .map(|(id, _)| id.clone())
                .collect(),
            flags: BTreeSet::new(),
            counters: BTreeMap::new(),
            actor_memory: BTreeMap::new(),
            timers: Vec::new(),
            random_seed: Self::DEFAULT_RANDOM_SEED,
            random_state: Self::DEFAULT_RANDOM_SEED,
            turn: 0,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error("game metadata has not been defined")]
    MissingMetadata,

    #[error("start room '{0}' does not exist")]
    MissingStartRoom(String),

    #[error("room '{0}' does not exist")]
    MissingRoom(String),
}

impl World {
    pub fn initial_state(&self) -> Result<GameState, WorldError> {
        let metadata = self.metadata.as_ref().ok_or(WorldError::MissingMetadata)?;

        if !self.rooms.contains_key(&metadata.start) {
            return Err(WorldError::MissingStartRoom(metadata.start.clone()));
        }

        Ok(GameState::new(metadata.start.clone(), &self.things))
    }

    pub fn validate(&self) -> WorldValidationReport {
        let mut report = WorldValidationReport::default();

        match &self.metadata {
            Some(metadata) => {
                if !self.rooms.contains_key(&metadata.start) {
                    report.error(format!("start room '{}' does not exist", metadata.start));
                }

                if metadata
                    .id
                    .as_deref()
                    .is_some_and(|id| id.trim().is_empty())
                {
                    report.error("game id cannot be empty");
                }

                if metadata
                    .version
                    .as_deref()
                    .is_some_and(|version| version.trim().is_empty())
                {
                    report.error("game version cannot be empty");
                }
            }
            None => report.error("game metadata has not been defined"),
        }

        for (room_id, room) in &self.rooms {
            if room.id != *room_id {
                report.error(format!(
                    "room map key '{room_id}' does not match room id '{}'",
                    room.id
                ));
            }

            for (direction, exit) in &room.exits {
                if !self.rooms.contains_key(&exit.to) {
                    report.error(format!(
                        "room '{room_id}' exit '{direction}' targets missing room '{}'",
                        exit.to
                    ));
                }

                if let Some(required) = &exit.requires
                    && !self.things.contains_key(required)
                {
                    report.error(format!(
                        "room '{room_id}' exit '{direction}' requires missing thing '{required}'"
                    ));
                }
            }
        }

        for (thing_id, thing) in &self.things {
            if thing.id != *thing_id {
                report.error(format!(
                    "thing map key '{thing_id}' does not match thing id '{}'",
                    thing.id
                ));
            }

            if thing.location == *thing_id {
                report.error(format!("thing '{thing_id}' cannot contain itself"));
            } else if !self.rooms.contains_key(&thing.location)
                && !self.things.contains_key(&thing.location)
                && thing.location != "inventory"
            {
                report.error(format!(
                    "thing '{thing_id}' starts in missing location '{}'",
                    thing.location
                ));
            }

            if thing.open && !thing.openable {
                report.error(format!("thing '{thing_id}' is open but not openable"));
            }

            if thing.locked && !thing.lockable {
                report.error(format!("thing '{thing_id}' is locked but not lockable"));
            }

            if thing.open && thing.locked {
                report.error(format!("thing '{thing_id}' cannot start open and locked"));
            }

            if let Some(key_id) = &thing.key {
                if key_id == thing_id {
                    report.error(format!("thing '{thing_id}' cannot use itself as a key"));
                } else if !self.things.contains_key(key_id) {
                    report.error(format!(
                        "thing '{thing_id}' references missing key '{key_id}'"
                    ));
                }
            }
        }

        for thing_id in self.things.keys() {
            let mut seen = BTreeSet::new();
            let mut current = thing_id.as_str();

            while let Some(thing) = self.things.get(current) {
                if !seen.insert(current.to_string()) {
                    report.error(format!(
                        "thing '{thing_id}' is in a recursive containment cycle"
                    ));
                    break;
                }

                current = &thing.location;
            }
        }

        for (actor_id, topics) in &self.actor_topics {
            let Some(actor) = self.things.get(actor_id) else {
                report.error(format!("actor topics reference missing actor '{actor_id}'"));
                continue;
            };

            if !actor.actor {
                report.error(format!(
                    "actor topics reference non-actor thing '{actor_id}'"
                ));
            }

            let mut topic_vocabulary = BTreeMap::<String, Vec<String>>::new();

            for (topic_id, topic) in topics {
                if topic.id != *topic_id {
                    report.error(format!(
                        "actor '{actor_id}' topic map key '{topic_id}' does not match topic id '{}'",
                        topic.id
                    ));
                }

                let mut terms = vec![topic.id.as_str()];
                terms.extend(topic.aliases.iter().map(String::as_str));

                for term in terms {
                    let normalized = normalize_noun_phrase(term);

                    if !normalized.is_empty() {
                        topic_vocabulary
                            .entry(normalized)
                            .or_default()
                            .push(topic.id.clone());
                    }
                }
            }

            for (term, mut topic_ids) in topic_vocabulary {
                topic_ids.sort();
                topic_ids.dedup();

                if topic_ids.len() > 1 {
                    report.warning(format!(
                        "actor '{actor_id}' topic alias '{term}' is shared by {}",
                        topic_ids.join(", ")
                    ));
                }
            }
        }

        let mut vocabulary = BTreeMap::<String, Vec<String>>::new();

        for thing in self.things.values() {
            let mut terms = vec![thing.id.as_str(), thing.name.as_str()];
            terms.extend(thing.aliases.iter().map(String::as_str));

            for term in terms {
                let normalized = normalize_noun_phrase(term);

                if !normalized.is_empty() {
                    vocabulary
                        .entry(normalized)
                        .or_default()
                        .push(thing.id.clone());
                }
            }
        }

        for (term, mut thing_ids) in vocabulary {
            thing_ids.sort();
            thing_ids.dedup();

            if thing_ids.len() > 1 {
                report.warning(format!(
                    "object name or alias '{term}' is shared by {}",
                    thing_ids.join(", ")
                ));
            }
        }

        report
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct WorldValidationReport {
    pub issues: Vec<WorldValidationIssue>,
}

impl WorldValidationReport {
    pub fn is_success(&self) -> bool {
        !self.has_errors()
    }

    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|issue| issue.severity == WorldValidationSeverity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &WorldValidationIssue> {
        self.issues
            .iter()
            .filter(|issue| issue.severity == WorldValidationSeverity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &WorldValidationIssue> {
        self.issues
            .iter()
            .filter(|issue| issue.severity == WorldValidationSeverity::Warning)
    }

    fn error(&mut self, message: impl Into<String>) {
        self.issues.push(WorldValidationIssue {
            severity: WorldValidationSeverity::Error,
            message: message.into(),
        });
    }

    fn warning(&mut self, message: impl Into<String>) {
        self.issues.push(WorldValidationIssue {
            severity: WorldValidationSeverity::Warning,
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldValidationIssue {
    pub severity: WorldValidationSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, thiserror::Error)]
pub enum GameError {
    #[error(transparent)]
    World(#[from] WorldError),

    #[error("invalid game state: {0}")]
    InvalidState(String),
}

#[derive(Debug, Clone)]
pub struct Game {
    world: World,
    state: GameState,
    undo_stack: Vec<GameSnapshot>,
    last_command: Option<String>,
    last_referenced_thing: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GameSnapshot {
    state: GameState,
    last_referenced_thing: Option<String>,
}

impl Game {
    const UNDO_LIMIT: usize = 20;

    pub fn new(world: World) -> Result<Self, GameError> {
        let state = world.initial_state()?;
        Ok(Self {
            world,
            state,
            undo_stack: Vec::new(),
            last_command: None,
            last_referenced_thing: None,
        })
    }

    pub fn state(&self) -> &GameState {
        &self.state
    }

    pub fn replace_state(&mut self, mut state: GameState) -> Result<(), GameError> {
        state.visited_rooms.insert(state.current_room.clone());
        self.validate_state(&state)?;
        self.state = state;
        self.clear_history();
        self.last_referenced_thing = None;
        Ok(())
    }

    pub fn snapshot(&self) -> GameSnapshot {
        GameSnapshot {
            state: self.state.clone(),
            last_referenced_thing: self.last_referenced_thing.clone(),
        }
    }

    pub fn restore_snapshot(&mut self, mut snapshot: GameSnapshot) -> Result<(), GameError> {
        snapshot
            .state
            .visited_rooms
            .insert(snapshot.state.current_room.clone());
        self.validate_state(&snapshot.state)?;
        self.state = snapshot.state;
        self.last_referenced_thing = snapshot.last_referenced_thing;
        Ok(())
    }

    pub fn last_command(&self) -> Option<&str> {
        self.last_command.as_deref()
    }

    pub fn remember_command(&mut self, command: impl Into<String>) {
        self.last_command = Some(command.into());
    }

    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.last_command = None;
        self.last_referenced_thing = None;
    }

    fn push_undo_state(&mut self, snapshot: GameSnapshot) {
        self.undo_stack.push(snapshot);

        if self.undo_stack.len() > Self::UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    fn validate_state(&self, state: &GameState) -> Result<(), GameError> {
        if !self.world.rooms.contains_key(&state.current_room) {
            return Err(GameError::InvalidState(format!(
                "current room '{}' does not exist",
                state.current_room
            )));
        }

        for room_id in &state.visited_rooms {
            if !self.world.rooms.contains_key(room_id) {
                return Err(GameError::InvalidState(format!(
                    "visited room '{room_id}' does not exist"
                )));
            }
        }

        for thing_id in &state.inventory {
            if !self.world.things.contains_key(thing_id) {
                return Err(GameError::InvalidState(format!(
                    "inventory contains unknown thing '{thing_id}'"
                )));
            }

            match state.thing_locations.get(thing_id) {
                Some(location) if location == "inventory" => {}
                Some(location) => {
                    return Err(GameError::InvalidState(format!(
                        "inventory thing '{thing_id}' has location '{location}', expected 'inventory'"
                    )));
                }
                None => {
                    return Err(GameError::InvalidState(format!(
                        "inventory thing '{thing_id}' has no saved location"
                    )));
                }
            }
        }

        for thing_id in &state.worn {
            let Some(thing) = self.world.things.get(thing_id) else {
                return Err(GameError::InvalidState(format!(
                    "worn set contains unknown thing '{thing_id}'"
                )));
            };

            if !state.inventory.contains(thing_id) {
                return Err(GameError::InvalidState(format!(
                    "worn thing '{thing_id}' is not in inventory"
                )));
            }

            if !thing.wearable {
                return Err(GameError::InvalidState(format!(
                    "worn thing '{thing_id}' is not wearable"
                )));
            }
        }

        for (thing_id, location) in &state.thing_locations {
            if !self.world.things.contains_key(thing_id) {
                return Err(GameError::InvalidState(format!(
                    "saved location references unknown thing '{thing_id}'"
                )));
            }

            if location == thing_id {
                return Err(GameError::InvalidState(format!(
                    "thing '{thing_id}' cannot contain itself"
                )));
            }

            if location != "inventory"
                && !self.world.rooms.contains_key(location)
                && !self.world.things.contains_key(location)
            {
                return Err(GameError::InvalidState(format!(
                    "thing '{thing_id}' is in unknown location '{location}'"
                )));
            }
        }

        for thing_id in &state.open_things {
            let Some(thing) = self.world.things.get(thing_id) else {
                return Err(GameError::InvalidState(format!(
                    "open set contains unknown thing '{thing_id}'"
                )));
            };

            if !thing.openable {
                return Err(GameError::InvalidState(format!(
                    "open thing '{thing_id}' is not openable"
                )));
            }
        }

        for thing_id in &state.locked_things {
            let Some(thing) = self.world.things.get(thing_id) else {
                return Err(GameError::InvalidState(format!(
                    "locked set contains unknown thing '{thing_id}'"
                )));
            };

            if !thing.lockable {
                return Err(GameError::InvalidState(format!(
                    "locked thing '{thing_id}' is not lockable"
                )));
            }

            if state.open_things.contains(thing_id) {
                return Err(GameError::InvalidState(format!(
                    "thing '{thing_id}' cannot be both open and locked"
                )));
            }
        }

        for thing_id in &state.hidden_things {
            if !self.world.things.contains_key(thing_id) {
                return Err(GameError::InvalidState(format!(
                    "hidden set contains unknown thing '{thing_id}'"
                )));
            }
        }

        for (actor_id, memory) in &state.actor_memory {
            let Some(actor) = self.world.things.get(actor_id) else {
                return Err(GameError::InvalidState(format!(
                    "actor memory references unknown actor '{actor_id}'"
                )));
            };

            if !actor.actor {
                return Err(GameError::InvalidState(format!(
                    "actor memory references non-actor thing '{actor_id}'"
                )));
            }

            for key in memory.keys() {
                if key.trim().is_empty() {
                    return Err(GameError::InvalidState(format!(
                        "actor memory for '{actor_id}' has an empty key"
                    )));
                }
            }
        }

        for timer in &state.timers {
            if timer.name.trim().is_empty() {
                return Err(GameError::InvalidState(
                    "scheduled event name cannot be empty".to_string(),
                ));
            }

            if timer.scene.as_deref().is_some_and(str::is_empty) {
                return Err(GameError::InvalidState(
                    "scheduled event scene cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }

    pub fn welcome(&self) -> Result<String, GameError> {
        self.look()
    }

    pub fn current_room_id(&self) -> &str {
        &self.state.current_room
    }

    pub fn current_scene(&self) -> Option<&str> {
        self.state.current_scene.as_deref()
    }

    pub fn current_chapter(&self) -> Option<&str> {
        self.state.current_chapter.as_deref()
    }

    pub fn start_scene(&mut self, name: impl Into<String>) {
        if let Some(current_scene) = &self.state.current_scene {
            self.state
                .timers
                .retain(|timer| timer.scene.as_ref() != Some(current_scene));
        }

        self.state.current_scene = Some(name.into());
    }

    pub fn end_scene(&mut self, name: Option<&str>) -> Option<String> {
        let current_scene = self.state.current_scene.as_deref()?;

        if name.is_some_and(|name| name != current_scene) {
            return None;
        }

        let ended = self.state.current_scene.take()?;
        self.state
            .timers
            .retain(|timer| timer.scene.as_ref() != Some(&ended));
        Some(ended)
    }

    pub fn set_chapter(&mut self, name: impl Into<String>) {
        self.state.current_chapter = Some(name.into());
    }

    pub fn flag(&mut self, name: impl Into<String>) {
        self.state.flags.insert(name.into());
    }

    pub fn clear_flag(&mut self, name: &str) {
        self.state.flags.remove(name);
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.state.flags.contains(name)
    }

    pub fn counter(&self, name: &str) -> i64 {
        self.state.counters.get(name).copied().unwrap_or(0)
    }

    pub fn set_counter(&mut self, name: impl Into<String>, value: i64) {
        self.state.counters.insert(name.into(), value);
    }

    pub fn inc_counter(&mut self, name: impl Into<String>, amount: i64) -> i64 {
        let counter = self.state.counters.entry(name.into()).or_default();
        *counter += amount;
        *counter
    }

    pub fn actor_memory(&self, actor_id: &str, key: &str) -> i64 {
        self.state
            .actor_memory
            .get(actor_id)
            .and_then(|memory| memory.get(key))
            .copied()
            .unwrap_or(0)
    }

    pub fn set_actor_memory(
        &mut self,
        actor_id: impl Into<String>,
        key: impl Into<String>,
        value: i64,
    ) {
        self.state
            .actor_memory
            .entry(actor_id.into())
            .or_default()
            .insert(key.into(), value);
    }

    pub fn inc_actor_memory(
        &mut self,
        actor_id: impl Into<String>,
        key: impl Into<String>,
        amount: i64,
    ) -> i64 {
        let memory = self.state.actor_memory.entry(actor_id.into()).or_default();
        let value = memory.entry(key.into()).or_default();
        *value += amount;
        *value
    }

    pub fn move_thing(&mut self, thing_id: &str, location_id: impl Into<String>) -> bool {
        if !self.world.things.contains_key(thing_id) {
            return false;
        }

        let location_id = location_id.into();
        if location_id == "inventory" {
            self.state.inventory.insert(thing_id.to_string());
        } else {
            self.state.inventory.remove(thing_id);
            self.state.worn.remove(thing_id);
        }

        self.state
            .thing_locations
            .insert(thing_id.to_string(), location_id);
        true
    }

    pub fn goto(&mut self, room_id: &str) -> Result<(), GameError> {
        if !self.world.rooms.contains_key(room_id) {
            return Err(WorldError::MissingRoom(room_id.to_string()).into());
        }

        self.state.current_room = room_id.to_string();
        self.state.visited_rooms.insert(room_id.to_string());
        Ok(())
    }

    pub fn has(&self, thing_id: &str) -> bool {
        self.state.inventory.contains(thing_id)
    }

    pub fn visible(&self, thing_id: &str) -> bool {
        self.world.things.contains_key(thing_id) && !self.state.hidden_things.contains(thing_id)
    }

    pub fn hide_thing(&mut self, thing_id: &str) -> bool {
        if !self.world.things.contains_key(thing_id) {
            return false;
        }

        self.state.hidden_things.insert(thing_id.to_string());
        true
    }

    pub fn reveal_thing(&mut self, thing_id: &str) -> bool {
        if !self.world.things.contains_key(thing_id) {
            return false;
        }

        self.state.hidden_things.remove(thing_id);
        true
    }

    pub fn visited(&self, room_id: &str) -> bool {
        self.state.visited_rooms.contains(room_id)
    }

    pub fn schedule_event(&mut self, turns: u64, name: impl Into<String>) {
        let name = name.into();
        self.cancel_event(&name);
        self.state.timers.push(ScheduledEvent {
            name,
            due_turn: self.state.turn.saturating_add(turns),
            scene: None,
        });
    }

    pub fn schedule_scene_event(
        &mut self,
        turns: u64,
        name: impl Into<String>,
        scene: impl Into<String>,
    ) {
        let name = name.into();
        let scene = scene.into();
        self.cancel_event(&name);
        self.state.timers.push(ScheduledEvent {
            name,
            due_turn: self.state.turn.saturating_add(turns),
            scene: Some(scene),
        });
    }

    pub fn cancel_event(&mut self, name: &str) {
        self.state.timers.retain(|timer| timer.name != name);
    }

    pub fn set_random_state(&mut self, random_state: u64) {
        self.state.random_state = random_state;
    }

    pub fn set_random_seed(&mut self, random_seed: u64) {
        self.state.random_seed = random_seed;
        self.state.random_state = random_seed;
    }

    pub fn handle_command(&mut self, input: &str) -> Result<CommandResult, GameError> {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return Ok(CommandResult::Continue(CommandOutcome::new("")));
        }

        let normalized = trimmed.to_lowercase();
        let normalized = normalize_command_text(&normalized);

        if matches!(normalized.as_str(), "again" | "g") {
            let Some(command) = self.last_command.clone() else {
                return Ok(CommandResult::Continue(CommandOutcome::new(
                    "Nothing to do again.",
                )));
            };

            return self.handle_command(&command);
        }

        if normalized == "undo" {
            let Some(snapshot) = self.undo_stack.pop() else {
                return Ok(CommandResult::Continue(CommandOutcome::new(
                    "Nothing to undo.",
                )));
            };

            self.restore_snapshot(snapshot)?;
            return Ok(CommandResult::Continue(CommandOutcome::new("Undone.")));
        }

        let verb = normalized.split_whitespace().next().unwrap_or_default();
        let advances_turn = command_advances_turn(verb);
        let pre_command_snapshot = self.snapshot();
        let result = match self.execute_normalized_command(&normalized) {
            Ok(result) => result,
            Err(error) => {
                self.restore_snapshot(pre_command_snapshot)?;
                return Err(error);
            }
        };

        let CommandResult::Continue(mut outcome) = result else {
            return Ok(result);
        };

        if advances_turn {
            self.push_undo_state(pre_command_snapshot);
            self.remember_command(normalized);
            outcome.events.extend(self.advance_turn());
        }

        Ok(CommandResult::Continue(outcome))
    }

    pub fn execute_normalized_command(
        &mut self,
        normalized: &str,
    ) -> Result<CommandResult, GameError> {
        let mut parts = normalized.split_whitespace();
        let verb = parts.next().unwrap_or_default();
        let rest = parts.collect::<Vec<_>>().join(" ");

        let outcome = match verb {
            "look" | "l" if rest.is_empty() => CommandOutcome {
                output: self.look()?,
                events: vec![GameEvent::Look {
                    room_id: self.state.current_room.clone(),
                }],
            },
            "look" | "l" => CommandOutcome::new(self.look_target(&rest)?),
            "inventory" | "inv" | "i" => CommandOutcome::new(self.inventory()),
            "examine" | "x" => CommandOutcome::new(self.examine(&rest)),
            "take" | "get" => self.take(&rest),
            "drop" => self.drop(&rest),
            "put" => self.put(&rest),
            "use" => self.use_thing(&rest),
            "read" => self.read(&rest),
            "open" => self.open(&rest),
            "close" | "shut" => self.close(&rest),
            "lock" => self.lock(&rest),
            "unlock" => self.unlock(&rest),
            "wear" | "don" => self.wear(&rest),
            "remove" | "doff" => self.remove(&rest),
            "talk" => self.talk(&rest),
            "ask" => self.ask(&rest),
            "tell" => self.tell(&rest),
            "show" => self.show(&rest),
            "give" => self.give(&rest),
            "go" => self.go_or_enter(&rest)?,
            "north" | "n" => self.go("north")?,
            "south" | "s" => self.go("south")?,
            "east" | "e" => self.go("east")?,
            "west" | "w" => self.go("west")?,
            "up" | "u" => self.go("up")?,
            "down" | "d" => self.go("down")?,
            "quit" | "exit" => return Ok(CommandResult::Quit("Goodbye.".to_string())),
            _ => self.fallback_command(verb, &rest)?,
        };

        self.remember_referenced_things(&outcome.events);

        Ok(CommandResult::Continue(outcome))
    }

    pub fn advance_turn(&mut self) -> Vec<GameEvent> {
        self.state.turn = self.state.turn.saturating_add(1);
        self.due_timer_events()
    }

    pub fn look(&self) -> Result<String, GameError> {
        let room = self.current_room()?;
        let mut output = format!("{}\n\n{}", room.name, room.desc);
        self.append_room_details(&mut output, room);

        Ok(output)
    }

    pub fn room_view(&self, desc: impl AsRef<str>) -> Result<String, GameError> {
        let room = self.current_room()?;
        let mut output = format!("{}\n\n{}", room.name, desc.as_ref());
        self.append_room_details(&mut output, room);

        Ok(output)
    }

    fn append_room_details(&self, output: &mut String, room: &Room) {
        let visible = self.visible_things();

        if !visible.is_empty() {
            output.push_str("\n\nYou can see ");
            output.push_str(&join_names(&visible));
            output.push('.');
        }

        if self.world.settings.exits.show {
            output.push_str("\n\n");
            output.push_str(&self.exit_list(room));
        }
    }

    fn exit_list(&self, room: &Room) -> String {
        let exits = if room.exits.is_empty() {
            "none".to_string()
        } else {
            room.exits.keys().cloned().collect::<Vec<_>>().join(", ")
        };

        format!("{}: {}.", self.world.settings.exits.label, exits)
    }

    fn inventory(&self) -> String {
        let things = self
            .state
            .inventory
            .iter()
            .filter(|id| self.visible(id))
            .filter_map(|id| self.world.things.get(id))
            .map(|thing| {
                if self.state.worn.contains(&thing.id) {
                    format!("a {} (worn)", thing.name)
                } else {
                    format!("a {}", thing.name)
                }
            })
            .collect::<Vec<_>>();

        if things.is_empty() {
            return "You are carrying nothing.".to_string();
        }

        format!("You are carrying {}.", join_phrases(&things))
    }

    fn examine(&self, query: &str) -> String {
        if query.is_empty() {
            return "Examine what?".to_string();
        }

        let Some(thing) = self.find_accessible_thing(query) else {
            return "You don't see that here.".to_string();
        };

        let mut output = thing
            .desc
            .clone()
            .unwrap_or_else(|| format!("You see nothing special about the {}.", thing.name));

        if thing.kind != ThingKind::Object {
            output.push_str("\n\n");
            if self.is_closed_container(&thing.id) {
                output.push_str(&format!("The {} is closed.", thing.name));
            } else {
                let contents = self.contents_of(&thing.id);
                output.push_str(&describe_contents(thing, &contents));
            }
        }

        output
    }

    fn take(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Take what?");
        }

        if query == "all" {
            return self.take_all();
        }

        let (query, source) = split_from_target(query);

        let source_id = if let Some(source) = source {
            let Some(source_id) = self.find_accessible_thing_id(source) else {
                return CommandOutcome::new("You don't see that here.");
            };

            if self.is_closed_container(&source_id) {
                let source = self
                    .world
                    .things
                    .get(&source_id)
                    .expect("source id came from world");
                return CommandOutcome::new(format!("The {} is closed.", source.name));
            }

            Some(source_id)
        } else {
            None
        };

        let Some(id) = self.find_reachable_thing_id(query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        if let Some(source_id) = source_id
            && self
                .state
                .thing_locations
                .get(&id)
                .is_none_or(|location| location != &source_id)
        {
            return CommandOutcome::new("That isn't there.");
        }

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");
        let thing_name = thing.name.clone();

        if !thing.portable {
            return CommandOutcome::new(format!("You can't take the {}.", thing.name));
        }

        self.state.inventory.insert(id.clone());
        self.state
            .thing_locations
            .insert(id.clone(), "inventory".to_string());

        CommandOutcome {
            output: format!("You take the {}.", thing_name),
            events: vec![GameEvent::Take { thing_id: id }],
        }
    }

    fn take_all(&mut self) -> CommandOutcome {
        let ids = self
            .world
            .things
            .iter()
            .filter(|(id, thing)| {
                thing.portable
                    && !self.state.inventory.contains(*id)
                    && self.visible(id)
                    && self.thing_is_reachable(id)
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();

        if ids.is_empty() {
            return CommandOutcome::new("There is nothing to take.");
        }

        let mut output = Vec::new();
        let mut events = Vec::new();

        for id in ids {
            let thing_name = self
                .world
                .things
                .get(&id)
                .expect("thing id came from world")
                .name
                .clone();
            self.state.inventory.insert(id.clone());
            self.state
                .thing_locations
                .insert(id.clone(), "inventory".to_string());
            output.push(format!("You take the {}.", thing_name));
            events.push(GameEvent::Take { thing_id: id });
        }

        CommandOutcome {
            output: output.join("\n"),
            events,
        }
    }

    fn put(&mut self, query: &str) -> CommandOutcome {
        let Some((item_query, preposition, target_query)) = split_put_target(query) else {
            return CommandOutcome::new("Put what where?");
        };

        let Some(item_id) = self.find_inventory_thing_id(item_query) else {
            return CommandOutcome::new("You aren't carrying that.");
        };

        let Some(target_id) = self.find_accessible_thing_id(target_query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        if item_id == target_id {
            return CommandOutcome::new("You can't put something inside itself.");
        }

        let target = self
            .world
            .things
            .get(&target_id)
            .expect("target id came from world");

        match (&target.kind, preposition) {
            (ThingKind::Container, "in") | (ThingKind::Supporter, "on") => {}
            (ThingKind::Container, _) => {
                return CommandOutcome::new(format!(
                    "You can only put things in the {}.",
                    target.name
                ));
            }
            (ThingKind::Supporter, _) => {
                return CommandOutcome::new(format!(
                    "You can only put things on the {}.",
                    target.name
                ));
            }
            (ThingKind::Object, _) => {
                return CommandOutcome::new(format!(
                    "You can't put things {} the {}.",
                    preposition, target.name
                ));
            }
        }

        if matches!(target.kind, ThingKind::Container) && self.is_closed_container(&target_id) {
            return CommandOutcome::new(format!("The {} is closed.", target.name));
        }

        let item = self
            .world
            .things
            .get(&item_id)
            .expect("item id came from inventory");
        let item_name = item.name.clone();
        let target_name = target.name.clone();

        if self.state.worn.contains(&item_id) {
            return CommandOutcome::new(format!("You need to remove the {} first.", item_name));
        }

        self.state.inventory.remove(&item_id);
        self.state.thing_locations.insert(item_id, target_id);

        CommandOutcome::new(format!(
            "You put the {} {} the {}.",
            item_name, preposition, target_name
        ))
    }

    fn drop(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Drop what?");
        }

        if query == "all" {
            return self.drop_all();
        }

        let Some(id) = self.find_inventory_thing_id(query) else {
            return CommandOutcome::new("You aren't carrying that.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");
        let thing_name = thing.name.clone();

        if self.state.worn.contains(&id) {
            return CommandOutcome::new(format!("You need to remove the {} first.", thing_name));
        }

        self.state.inventory.remove(&id);
        self.state
            .thing_locations
            .insert(id.clone(), self.state.current_room.clone());

        CommandOutcome {
            output: format!("You drop the {}.", thing_name),
            events: vec![GameEvent::Drop { thing_id: id }],
        }
    }

    fn drop_all(&mut self) -> CommandOutcome {
        let ids = self
            .state
            .inventory
            .iter()
            .filter(|id| !self.state.worn.contains(*id))
            .cloned()
            .collect::<Vec<_>>();

        if ids.is_empty() {
            return CommandOutcome::new("You have nothing to drop.");
        }

        let mut output = Vec::new();
        let mut events = Vec::new();

        for id in ids {
            let thing_name = self
                .world
                .things
                .get(&id)
                .expect("thing id came from inventory")
                .name
                .clone();
            self.state.inventory.remove(&id);
            self.state
                .thing_locations
                .insert(id.clone(), self.state.current_room.clone());
            output.push(format!("You drop the {}.", thing_name));
            events.push(GameEvent::Drop { thing_id: id });
        }

        CommandOutcome {
            output: output.join("\n"),
            events,
        }
    }

    fn use_thing(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Use what?");
        }

        if let Some((item_query, target_query)) = split_use_target(query) {
            let Some(item_id) = self.find_accessible_thing_id(item_query) else {
                return CommandOutcome::new("You don't see that here.");
            };

            let Some(target_id) = self.find_accessible_thing_id(target_query) else {
                return CommandOutcome::new("You don't see that here.");
            };

            if item_id == target_id {
                return CommandOutcome::new("You can't use something on itself.");
            }

            return CommandOutcome {
                output: "Nothing happens.".to_string(),
                events: vec![GameEvent::UseWith { item_id, target_id }],
            };
        }

        let Some(id) = self.find_accessible_thing_id(query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");

        CommandOutcome {
            output: format!("You find no use for the {}.", thing.name),
            events: vec![GameEvent::Use { thing_id: id }],
        }
    }

    fn read(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Read what?");
        }

        let Some(id) = self.find_accessible_thing_id(query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");

        CommandOutcome {
            output: thing
                .read
                .clone()
                .unwrap_or_else(|| format!("There is nothing to read on the {}.", thing.name)),
            events: vec![GameEvent::Read { thing_id: id }],
        }
    }

    fn open(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Open what?");
        }

        let Some(id) = self.find_accessible_thing_id(query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");
        let thing_name = thing.name.clone();

        if !thing.openable {
            return CommandOutcome::new(format!("You can't open the {}.", thing_name));
        }

        if self.is_thing_open(&id) {
            return CommandOutcome::new(format!("The {} is already open.", thing_name));
        }

        if self.is_thing_locked(&id) {
            return CommandOutcome::new(format!("The {} is locked.", thing_name));
        }

        self.state.open_things.insert(id.clone());

        CommandOutcome {
            output: format!("You open the {}.", thing_name),
            events: vec![GameEvent::Open { thing_id: id }],
        }
    }

    fn close(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Close what?");
        }

        let Some(id) = self.find_accessible_thing_id(query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");
        let thing_name = thing.name.clone();

        if !thing.openable {
            return CommandOutcome::new(format!("You can't close the {}.", thing_name));
        }

        if !self.is_thing_open(&id) {
            return CommandOutcome::new(format!("The {} is already closed.", thing_name));
        }

        self.state.open_things.remove(&id);

        CommandOutcome {
            output: format!("You close the {}.", thing_name),
            events: vec![GameEvent::Close { thing_id: id }],
        }
    }

    fn lock(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Lock what?");
        }

        let (target_query, key_query) = split_with_target(query);

        let Some(id) = self.find_accessible_thing_id(target_query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");
        let thing_name = thing.name.clone();
        let required_key = thing.key.clone();

        if !thing.lockable {
            return CommandOutcome::new(format!("You can't lock the {}.", thing_name));
        }

        if self.is_thing_locked(&id) {
            return CommandOutcome::new(format!("The {} is already locked.", thing_name));
        }

        if self.is_thing_open(&id) {
            return CommandOutcome::new(format!("You need to close the {} first.", thing_name));
        }

        if let Err(output) = self.check_lock_key(&thing_name, required_key.as_deref(), key_query) {
            return CommandOutcome::new(output);
        }

        self.state.locked_things.insert(id.clone());

        CommandOutcome {
            output: format!("You lock the {}.", thing_name),
            events: vec![GameEvent::Lock { thing_id: id }],
        }
    }

    fn unlock(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Unlock what?");
        }

        let (target_query, key_query) = split_with_target(query);

        let Some(id) = self.find_accessible_thing_id(target_query) else {
            return CommandOutcome::new("You don't see that here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");
        let thing_name = thing.name.clone();
        let required_key = thing.key.clone();

        if !thing.lockable {
            return CommandOutcome::new(format!("You can't unlock the {}.", thing_name));
        }

        if !self.is_thing_locked(&id) {
            return CommandOutcome::new(format!("The {} is already unlocked.", thing_name));
        }

        if let Err(output) = self.check_lock_key(&thing_name, required_key.as_deref(), key_query) {
            return CommandOutcome::new(output);
        }

        self.state.locked_things.remove(&id);

        CommandOutcome {
            output: format!("You unlock the {}.", thing_name),
            events: vec![GameEvent::Unlock { thing_id: id }],
        }
    }

    fn wear(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Wear what?");
        }

        let Some(id) = self.find_inventory_thing_id(query) else {
            return CommandOutcome::new("You aren't carrying that.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from inventory");

        if !thing.wearable {
            return CommandOutcome::new(format!("You can't wear the {}.", thing.name));
        }

        if !self.state.worn.insert(id) {
            return CommandOutcome::new(format!("You are already wearing the {}.", thing.name));
        }

        CommandOutcome::new(format!("You put on the {}.", thing.name))
    }

    fn remove(&mut self, query: &str) -> CommandOutcome {
        if query.is_empty() {
            return CommandOutcome::new("Remove what?");
        }

        let Some(id) = self.find_inventory_thing_id(query) else {
            return CommandOutcome::new("You aren't carrying that.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from inventory");

        if !self.state.worn.remove(&id) {
            return CommandOutcome::new(format!("You aren't wearing the {}.", thing.name));
        }

        CommandOutcome::new(format!("You remove the {}.", thing.name))
    }

    fn talk(&mut self, query: &str) -> CommandOutcome {
        let Some(query) = split_talk_target(query) else {
            return CommandOutcome::new("Talk to whom?");
        };

        let Some(id) = self.find_reachable_thing_id(query) else {
            return CommandOutcome::new("You don't see them here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");

        if !thing.actor {
            return CommandOutcome::new(format!("The {} does not respond.", thing.name));
        }

        CommandOutcome {
            output: format!("The {} has nothing to say.", thing.name),
            events: vec![GameEvent::Talk { thing_id: id }],
        }
    }

    fn ask(&mut self, query: &str) -> CommandOutcome {
        let Some((actor_query, topic)) = split_ask_target(query) else {
            return CommandOutcome::new("Ask whom about what?");
        };

        let Some((id, thing)) = self.find_actor(actor_query) else {
            return self.actor_not_found_output(actor_query);
        };
        let actor_name = thing.name.clone();
        let topic = self.resolve_actor_topic(&id, topic);

        if !self.actor_topic_available(&id, &topic) {
            return CommandOutcome::new(format!(
                "The {} has nothing to say about {}.",
                actor_name, topic
            ));
        }

        self.inc_actor_memory(id.clone(), format!("asked:{topic}"), 1);
        CommandOutcome {
            output: format!("The {} has nothing to say about {}.", actor_name, topic),
            events: vec![GameEvent::Ask {
                thing_id: id,
                topic,
            }],
        }
    }

    fn tell(&mut self, query: &str) -> CommandOutcome {
        let Some((actor_query, topic)) = split_tell_target(query) else {
            return CommandOutcome::new("Tell whom about what?");
        };

        let Some((id, thing)) = self.find_actor(actor_query) else {
            return self.actor_not_found_output(actor_query);
        };
        let actor_name = thing.name.clone();
        let topic = self.resolve_actor_topic(&id, topic);

        if !self.actor_topic_available(&id, &topic) {
            return CommandOutcome::new(format!("The {} does not respond to that.", actor_name));
        }

        self.inc_actor_memory(id.clone(), format!("told:{topic}"), 1);
        CommandOutcome {
            output: format!("The {} does not respond to that.", actor_name),
            events: vec![GameEvent::Tell {
                thing_id: id,
                topic,
            }],
        }
    }

    fn show(&mut self, query: &str) -> CommandOutcome {
        let Some((item_query, actor_query)) = split_item_actor_target(query) else {
            return CommandOutcome::new("Show what to whom?");
        };

        let Some(item_id) = self.find_inventory_thing_id(item_query) else {
            return CommandOutcome::new("You aren't carrying that.");
        };

        let Some((actor_id, actor)) = self.find_actor(actor_query) else {
            return self.actor_not_found_output(actor_query);
        };

        let actor_name = actor.name.clone();
        let item = self
            .world
            .things
            .get(&item_id)
            .expect("thing id came from inventory")
            .name
            .clone();
        self.inc_actor_memory(actor_id.clone(), format!("shown:{item_id}"), 1);

        CommandOutcome {
            output: format!("The {} shows no interest in the {}.", actor_name, item),
            events: vec![GameEvent::Show {
                thing_id: actor_id,
                item_id,
            }],
        }
    }

    fn give(&mut self, query: &str) -> CommandOutcome {
        let Some((item_query, actor_query)) = split_item_actor_target(query) else {
            return CommandOutcome::new("Give what to whom?");
        };

        let Some(item_id) = self.find_inventory_thing_id(item_query) else {
            return CommandOutcome::new("You aren't carrying that.");
        };

        let Some((actor_id, actor)) = self.find_actor(actor_query) else {
            return self.actor_not_found_output(actor_query);
        };

        let actor_name = actor.name.clone();
        let item = self
            .world
            .things
            .get(&item_id)
            .expect("thing id came from inventory")
            .name
            .clone();
        self.inc_actor_memory(actor_id.clone(), format!("given:{item_id}"), 1);

        CommandOutcome {
            output: format!("The {} does not want the {}.", actor_name, item),
            events: vec![GameEvent::Give {
                thing_id: actor_id,
                item_id,
            }],
        }
    }

    fn go_or_enter(&mut self, query: &str) -> Result<CommandOutcome, GameError> {
        if let Some(target) = query.strip_prefix("through ").map(str::trim)
            && !target.is_empty()
        {
            return self.enter(target);
        }

        self.go(query)
    }

    fn go(&mut self, direction: &str) -> Result<CommandOutcome, GameError> {
        let direction = normalize_direction(direction).unwrap_or(direction);

        if direction.is_empty() {
            return Ok(CommandOutcome::new("Go where?"));
        }

        let room = self.current_room()?;
        let Some(exit) = room.exits.get(direction) else {
            return Ok(CommandOutcome::new("You can't go that way."));
        };
        let exit = exit.clone();

        if let Some(required_item) = &exit.requires
            && !self.has(required_item)
        {
            return Ok(CommandOutcome::new(
                exit.locked_msg
                    .unwrap_or_else(|| "You can't go that way yet.".to_string()),
            ));
        }

        if !self.world.rooms.contains_key(&exit.to) {
            return Err(WorldError::MissingRoom(exit.to).into());
        }

        self.state.current_room = exit.to;
        self.state
            .visited_rooms
            .insert(self.state.current_room.clone());
        Ok(CommandOutcome {
            output: self.look()?,
            events: vec![GameEvent::EnterRoom {
                room_id: self.state.current_room.clone(),
            }],
        })
    }

    fn enter(&mut self, query: &str) -> Result<CommandOutcome, GameError> {
        if query.is_empty() {
            return Ok(CommandOutcome::new("Enter what?"));
        }

        if let Some(direction) = normalize_direction(query) {
            return self.go(direction);
        }

        let room = self.current_room()?;
        let matching_exits = room
            .exits
            .iter()
            .filter_map(|(direction, exit)| {
                self.world
                    .rooms
                    .get(&exit.to)
                    .filter(|target| {
                        normalize_noun_phrase(&target.id) == normalize_noun_phrase(query)
                            || normalize_noun_phrase(&target.name) == normalize_noun_phrase(query)
                    })
                    .map(|_| direction.clone())
            })
            .collect::<Vec<_>>();

        match matching_exits.as_slice() {
            [direction] => self.go(direction),
            [] => {
                if self.find_accessible_thing_id(query).is_some() {
                    Ok(self.use_thing(query))
                } else {
                    Ok(CommandOutcome::new("You can't enter that."))
                }
            }
            _ => Ok(CommandOutcome::new("Which way do you mean?")),
        }
    }

    fn check_lock_key(
        &self,
        thing_name: &str,
        required_key: Option<&str>,
        key_query: Option<&str>,
    ) -> Result<(), String> {
        let Some(required_key) = required_key else {
            if let Some(key_query) = key_query
                && self.find_inventory_thing_id(key_query).is_none()
            {
                return Err("You aren't carrying that.".to_string());
            }

            return Ok(());
        };

        let key_name = self
            .world
            .things
            .get(required_key)
            .map(|thing| thing.name.as_str())
            .unwrap_or("right key");

        if let Some(key_query) = key_query {
            let Some(key_id) = self.find_inventory_thing_id(key_query) else {
                return Err("You aren't carrying that.".to_string());
            };

            if key_id != required_key {
                return Err(format!("That doesn't fit the {}.", thing_name));
            }

            return Ok(());
        }

        if self.has(required_key) {
            Ok(())
        } else {
            Err(format!("You need the {}.", key_name))
        }
    }

    fn is_thing_open(&self, thing_id: &str) -> bool {
        self.state.open_things.contains(thing_id)
    }

    fn is_thing_locked(&self, thing_id: &str) -> bool {
        self.state.locked_things.contains(thing_id)
    }

    fn is_thing_hidden(&self, thing_id: &str) -> bool {
        self.state.hidden_things.contains(thing_id)
    }

    fn is_closed_container(&self, thing_id: &str) -> bool {
        self.world.things.get(thing_id).is_some_and(|thing| {
            thing.kind == ThingKind::Container && thing.openable && !self.is_thing_open(thing_id)
        })
    }

    fn current_room(&self) -> Result<&Room, GameError> {
        self.world
            .rooms
            .get(&self.state.current_room)
            .ok_or_else(|| WorldError::MissingRoom(self.state.current_room.clone()).into())
    }

    fn due_timer_events(&mut self) -> Vec<GameEvent> {
        let mut due = Vec::new();
        let mut pending = Vec::new();

        for timer in self.state.timers.drain(..) {
            if timer.due_turn <= self.state.turn {
                if timer
                    .scene
                    .as_ref()
                    .is_none_or(|scene| self.state.current_scene.as_ref() == Some(scene))
                {
                    due.push(GameEvent::Timer {
                        event_name: timer.name,
                    });
                }
            } else {
                pending.push(timer);
            }
        }

        self.state.timers = pending;
        due
    }

    fn visible_things(&self) -> Vec<&Thing> {
        self.world
            .things
            .iter()
            .filter(|(id, _)| {
                !self.is_thing_hidden(id)
                    && self
                        .state
                        .thing_locations
                        .get(*id)
                        .is_some_and(|location| location == &self.state.current_room)
            })
            .map(|(_, thing)| thing)
            .collect()
    }

    fn find_accessible_thing(&self, query: &str) -> Option<&Thing> {
        self.find_accessible_thing_id(query)
            .and_then(|id| self.world.things.get(&id))
    }

    fn find_accessible_thing_id(&self, query: &str) -> Option<String> {
        if let Some(id) = self.resolve_pronoun_thing_id(query)
            && (self.thing_is_reachable(&id) || self.state.inventory.contains(&id))
        {
            return Some(id);
        }

        self.find_reachable_thing_id(query).or_else(|| {
            self.state
                .inventory
                .iter()
                .filter(|id| self.visible(id))
                .find(|id| {
                    self.world
                        .things
                        .get(*id)
                        .is_some_and(|thing| thing.matches(query))
                })
                .cloned()
        })
    }

    fn find_reachable_thing_id(&self, query: &str) -> Option<String> {
        if let Some(id) = self.resolve_pronoun_thing_id(query)
            && self.thing_is_reachable(&id)
        {
            return Some(id);
        }

        self.world
            .things
            .iter()
            .find(|(id, thing)| {
                !self.is_thing_hidden(id) && thing.matches(query) && self.thing_is_reachable(id)
            })
            .map(|(id, _)| id.clone())
    }

    fn find_inventory_thing_id(&self, query: &str) -> Option<String> {
        if let Some(id) = self.resolve_pronoun_thing_id(query)
            && self.state.inventory.contains(&id)
        {
            return Some(id);
        }

        self.state
            .inventory
            .iter()
            .filter(|id| self.visible(id))
            .find(|id| {
                self.world
                    .things
                    .get(*id)
                    .is_some_and(|thing| thing.matches(query))
            })
            .cloned()
    }

    fn resolve_pronoun_thing_id(&self, query: &str) -> Option<String> {
        matches!(normalize_noun_phrase(query).as_str(), "it" | "them")
            .then(|| self.last_referenced_thing.clone())
            .flatten()
            .filter(|id| self.world.things.contains_key(id) && self.visible(id))
    }

    fn remember_referenced_things(&mut self, events: &[GameEvent]) {
        for event in events {
            let thing_id = match event {
                GameEvent::Take { thing_id }
                | GameEvent::Drop { thing_id }
                | GameEvent::Use { thing_id }
                | GameEvent::Read { thing_id }
                | GameEvent::Open { thing_id }
                | GameEvent::Close { thing_id }
                | GameEvent::Lock { thing_id }
                | GameEvent::Unlock { thing_id }
                | GameEvent::Talk { thing_id }
                | GameEvent::Ask { thing_id, .. }
                | GameEvent::Tell { thing_id, .. }
                | GameEvent::Show { thing_id, .. }
                | GameEvent::Give { thing_id, .. } => Some(thing_id),
                GameEvent::UseWith { item_id, .. } => Some(item_id),
                GameEvent::Look { .. }
                | GameEvent::EnterRoom { .. }
                | GameEvent::CustomVerb { .. }
                | GameEvent::Timer { .. } => None,
            };

            if let Some(thing_id) = thing_id {
                self.last_referenced_thing = Some(thing_id.clone());
            }
        }
    }

    fn find_actor(&self, query: &str) -> Option<(String, &Thing)> {
        let id = self.find_reachable_thing_id(query)?;
        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");

        thing.actor.then_some((id, thing))
    }

    fn actor_not_found_output(&self, query: &str) -> CommandOutcome {
        let Some(id) = self.find_reachable_thing_id(query) else {
            return CommandOutcome::new("You don't see them here.");
        };

        let thing = self
            .world
            .things
            .get(&id)
            .expect("thing id came from world");
        CommandOutcome::new(format!("The {} does not respond.", thing.name))
    }

    fn resolve_actor_topic(&self, actor_id: &str, query: &str) -> String {
        let normalized = normalize_noun_phrase(query);

        self.world
            .actor_topics
            .get(actor_id)
            .and_then(|topics| {
                topics
                    .values()
                    .find(|topic| {
                        normalize_noun_phrase(&topic.id) == normalized
                            || topic
                                .aliases
                                .iter()
                                .any(|alias| normalize_noun_phrase(alias) == normalized)
                    })
                    .map(|topic| topic.id.clone())
            })
            .unwrap_or(normalized)
    }

    fn actor_topic_available(&self, actor_id: &str, topic_id: &str) -> bool {
        let Some(topic) = self
            .world
            .actor_topics
            .get(actor_id)
            .and_then(|topics| topics.get(topic_id))
        else {
            return true;
        };

        topic
            .requires
            .as_ref()
            .is_none_or(|flag| self.has_flag(flag))
    }

    fn look_inside(&self, query: &str) -> String {
        let Some((preposition, target_query)) = split_look_target(query) else {
            return self.examine(query);
        };

        let Some(target) = self.find_accessible_thing(target_query) else {
            return "You don't see that here.".to_string();
        };

        match (&target.kind, preposition) {
            (ThingKind::Container, "in") | (ThingKind::Supporter, "on") => {
                if self.is_closed_container(&target.id) {
                    return format!("The {} is closed.", target.name);
                }

                describe_contents(target, &self.contents_of(&target.id))
            }
            (ThingKind::Container, _) => format!("Try looking in the {}.", target.name),
            (ThingKind::Supporter, _) => format!("Try looking on the {}.", target.name),
            (ThingKind::Object, _) => {
                format!("There's nothing {} the {}.", preposition, target.name)
            }
        }
    }

    fn look_target(&self, query: &str) -> Result<String, GameError> {
        if let Some(target) = query.strip_prefix("at ").map(str::trim)
            && !target.is_empty()
        {
            return Ok(self.examine(target));
        }

        if matches!(query, "exit" | "exits") {
            return Ok(self.exit_list(self.current_room()?));
        }

        if let Some(direction) = normalize_direction(query) {
            return self.look_exit(direction);
        }

        Ok(self.look_inside(query))
    }

    fn look_exit(&self, direction: &str) -> Result<String, GameError> {
        let room = self.current_room()?;
        let Some(exit) = room.exits.get(direction) else {
            return Ok("You can't see a way in that direction.".to_string());
        };

        if let Some(required_item) = &exit.requires
            && !self.has(required_item)
        {
            return Ok(exit
                .locked_msg
                .clone()
                .unwrap_or_else(|| "You can't see much that way yet.".to_string()));
        }

        let target = self
            .world
            .rooms
            .get(&exit.to)
            .ok_or_else(|| WorldError::MissingRoom(exit.to.clone()))?;

        if target.desc.is_empty() {
            Ok(format!("To the {} is {}.", direction, target.name))
        } else {
            Ok(format!(
                "To the {} is {}.\n\n{}",
                direction, target.name, target.desc
            ))
        }
    }

    fn contents_of(&self, thing_id: &str) -> Vec<&Thing> {
        self.world
            .things
            .iter()
            .filter(|(id, _)| {
                !self.is_thing_hidden(id)
                    && self
                        .state
                        .thing_locations
                        .get(*id)
                        .is_some_and(|location| location == thing_id)
            })
            .map(|(_, thing)| thing)
            .collect()
    }

    fn thing_is_reachable(&self, thing_id: &str) -> bool {
        let mut seen = BTreeSet::new();
        self.thing_is_reachable_inner(thing_id, &mut seen)
    }

    fn thing_is_reachable_inner(&self, thing_id: &str, seen: &mut BTreeSet<String>) -> bool {
        if self.is_thing_hidden(thing_id) {
            return false;
        }

        if !seen.insert(thing_id.to_string()) {
            return false;
        }

        let Some(location) = self.state.thing_locations.get(thing_id) else {
            return false;
        };

        if location == &self.state.current_room {
            return true;
        }

        let Some(parent) = self.world.things.get(location) else {
            return false;
        };

        if parent.kind == ThingKind::Object {
            return false;
        }

        if parent.kind == ThingKind::Container && parent.openable && !self.is_thing_open(&parent.id)
        {
            return false;
        }

        self.thing_is_reachable_inner(&parent.id, seen)
    }

    fn find_custom_verb(&self, query: &str) -> Option<String> {
        self.world
            .verbs
            .values()
            .find(|verb| verb.id == query || verb.aliases.iter().any(|alias| alias == query))
            .map(|verb| verb.id.clone())
    }

    fn fallback_command(&mut self, verb: &str, rest: &str) -> Result<CommandOutcome, GameError> {
        if let Some(verb_id) = self.find_custom_verb(verb) {
            return Ok(CommandOutcome {
                output: String::new(),
                events: vec![GameEvent::CustomVerb {
                    verb_id,
                    input: rest.to_string(),
                }],
            });
        }

        match verb {
            "enter" => self.enter(rest),
            "wait" | "z" => Ok(CommandOutcome::new("Time passes.")),
            "listen" => Ok(CommandOutcome::new("You hear nothing unusual.")),
            "smell" => Ok(CommandOutcome::new("You smell nothing unusual.")),
            "search" if rest.is_empty() => Ok(CommandOutcome::new("You find nothing new.")),
            "search" => Ok(CommandOutcome::new(self.examine(rest))),
            "touch" if rest.is_empty() => Ok(CommandOutcome::new("Touch what?")),
            "touch" => Ok(CommandOutcome::new(self.touch(rest))),
            _ => Ok(CommandOutcome::new("I don't understand that.")),
        }
    }

    fn touch(&self, query: &str) -> String {
        let Some(thing) = self.find_accessible_thing(query) else {
            return "You don't see that here.".to_string();
        };

        format!(
            "You touch the {} and notice nothing unexpected.",
            thing.name
        )
    }
}

impl Thing {
    fn matches(&self, query: &str) -> bool {
        let query = normalize_noun_phrase(query);

        normalize_noun_phrase(&self.id) == query
            || normalize_noun_phrase(&self.name) == query
            || self
                .aliases
                .iter()
                .any(|alias| normalize_noun_phrase(alias) == query)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    Continue(CommandOutcome),
    Quit(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutcome {
    pub output: String,
    pub events: Vec<GameEvent>,
}

impl CommandOutcome {
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            events: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameEvent {
    Look { room_id: String },
    EnterRoom { room_id: String },
    Take { thing_id: String },
    Drop { thing_id: String },
    Use { thing_id: String },
    UseWith { item_id: String, target_id: String },
    Read { thing_id: String },
    Open { thing_id: String },
    Close { thing_id: String },
    Lock { thing_id: String },
    Unlock { thing_id: String },
    Talk { thing_id: String },
    Ask { thing_id: String, topic: String },
    Tell { thing_id: String, topic: String },
    Show { thing_id: String, item_id: String },
    Give { thing_id: String, item_id: String },
    CustomVerb { verb_id: String, input: String },
    Timer { event_name: String },
}

pub fn next_random_state(state: u64) -> u64 {
    state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

pub fn random_bounded(state: u64, upper_exclusive: u64) -> (u64, u64) {
    assert!(upper_exclusive > 0, "random upper bound must be positive");
    let next = next_random_state(state);
    (next, next % upper_exclusive)
}

fn join_names(things: &[&Thing]) -> String {
    match things {
        [] => String::new(),
        [thing] => format!("a {}", thing.name),
        [first, second] => format!("a {} and a {}", first.name, second.name),
        many => {
            let mut names = many
                .iter()
                .map(|thing| format!("a {}", thing.name))
                .collect::<Vec<_>>();
            let last = names.pop().expect("many has at least one item");
            format!("{}, and {}", names.join(", "), last)
        }
    }
}

fn join_phrases(phrases: &[String]) -> String {
    match phrases {
        [] => String::new(),
        [phrase] => phrase.clone(),
        [first, second] => format!("{first} and {second}"),
        many => {
            let mut phrases = many.to_vec();
            let last = phrases.pop().expect("many has at least one item");
            format!("{}, and {}", phrases.join(", "), last)
        }
    }
}

fn describe_contents(holder: &Thing, contents: &[&Thing]) -> String {
    if contents.is_empty() {
        return format!(
            "There is nothing {} the {}.",
            holder.kind.preposition(),
            holder.name
        );
    }

    format!(
        "{} the {} {} {}.",
        capitalize(holder.kind.preposition()),
        holder.name,
        if contents.len() == 1 { "is" } else { "are" },
        join_names(contents)
    )
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();

    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn normalize_noun_phrase(input: &str) -> String {
    let normalized = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    strip_leading_article(&normalized).to_string()
}

fn normalize_command_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_direction(input: &str) -> Option<&'static str> {
    match input {
        "north" | "n" => Some("north"),
        "south" | "s" => Some("south"),
        "east" | "e" => Some("east"),
        "west" | "w" => Some("west"),
        "up" | "u" => Some("up"),
        "down" | "d" => Some("down"),
        _ => None,
    }
}

fn command_advances_turn(verb: &str) -> bool {
    !matches!(
        verb,
        "look"
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

fn strip_leading_article(input: &str) -> &str {
    input
        .strip_prefix("the ")
        .or_else(|| input.strip_prefix("a "))
        .or_else(|| input.strip_prefix("an "))
        .unwrap_or(input)
}

fn split_look_target(query: &str) -> Option<(&str, &str)> {
    query
        .strip_prefix("in ")
        .map(|target| ("in", target.trim()))
        .or_else(|| {
            query
                .strip_prefix("on ")
                .map(|target| ("on", target.trim()))
        })
        .filter(|(_, target)| !target.is_empty())
}

fn split_use_target(query: &str) -> Option<(&str, &str)> {
    query
        .split_once(" on ")
        .or_else(|| query.split_once(" with "))
        .map(|(item, target)| (item.trim(), target.trim()))
        .filter(|(item, target)| !item.is_empty() && !target.is_empty())
}

fn split_from_target(query: &str) -> (&str, Option<&str>) {
    if let Some((item, source)) = query.split_once(" from ") {
        return (item.trim(), Some(source.trim()));
    }

    (query.trim(), None)
}

fn split_with_target(query: &str) -> (&str, Option<&str>) {
    if let Some((target, tool)) = query.split_once(" with ") {
        return (target.trim(), Some(tool.trim()));
    }

    (query.trim(), None)
}

fn split_put_target(query: &str) -> Option<(&str, &str, &str)> {
    query
        .split_once(" in ")
        .map(|(item, target)| (item.trim(), "in", target.trim()))
        .or_else(|| {
            query
                .split_once(" on ")
                .map(|(item, target)| (item.trim(), "on", target.trim()))
        })
        .filter(|(item, _, target)| !item.is_empty() && !target.is_empty())
}

fn split_talk_target(query: &str) -> Option<&str> {
    let query = query.trim();

    if query.is_empty() {
        return None;
    }

    query
        .strip_prefix("to ")
        .map(str::trim)
        .or(Some(query))
        .filter(|target| !target.is_empty())
}

fn split_ask_target(query: &str) -> Option<(&str, &str)> {
    let query = query.trim();
    let query = query.strip_prefix("to ").unwrap_or(query);
    let (actor, topic) = query.split_once(" about ")?;
    let actor = actor.trim();
    let topic = topic.trim();

    (!actor.is_empty() && !topic.is_empty()).then_some((actor, topic))
}

fn split_tell_target(query: &str) -> Option<(&str, &str)> {
    let query = query.trim();
    let query = query.strip_prefix("to ").unwrap_or(query);
    let (actor, topic) = query.split_once(" about ")?;
    let actor = actor.trim();
    let topic = topic.trim();

    (!actor.is_empty() && !topic.is_empty()).then_some((actor, topic))
}

fn split_item_actor_target(query: &str) -> Option<(&str, &str)> {
    let (item, actor) = query.trim().split_once(" to ")?;
    let item = item.trim();
    let actor = actor.trim();

    (!item.is_empty() && !actor.is_empty()).then_some((item, actor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_takes_drops_and_lists_inventory() {
        let mut game = Game::new(test_world()).expect("valid world");

        assert!(game.welcome().expect("look").contains("Foyer"));
        assert!(game.visited("foyer"));
        assert!(!game.visited("hall"));

        let CommandResult::Continue(outcome) =
            game.handle_command("take key").expect("take command")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "You take the brass key.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Take {
                thing_id: "brass_key".to_string()
            }]
        );

        let CommandResult::Continue(outcome) = game.handle_command("i").expect("inventory") else {
            panic!("inventory should continue");
        };
        assert_eq!(outcome.output, "You are carrying a brass key.");

        let CommandResult::Continue(outcome) = game.handle_command("north").expect("move north")
        else {
            panic!("movement should continue");
        };
        assert!(outcome.output.contains("Hall"));
        assert!(game.visited("hall"));

        let CommandResult::Continue(outcome) = game.handle_command("drop key").expect("drop")
        else {
            panic!("drop should continue");
        };
        assert_eq!(outcome.output, "You drop the brass key.");
    }

    #[test]
    fn looks_toward_exits_without_moving() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) = game.handle_command("l north").expect("look north")
        else {
            panic!("look north should continue");
        };
        assert_eq!(outcome.output, "To the north is Hall.\n\nA narrow hall.");
        assert_eq!(game.current_room_id(), "foyer");
        assert_eq!(game.state().turn, 0);

        let CommandResult::Continue(outcome) = game.handle_command("look e").expect("look east")
        else {
            panic!("look east should continue");
        };
        assert_eq!(outcome.output, "The study door is locked.");
        assert_eq!(game.current_room_id(), "foyer");
        assert_eq!(game.state().turn, 0);

        let CommandResult::Continue(outcome) =
            game.handle_command("look exits").expect("look exits")
        else {
            panic!("look exits should continue");
        };
        assert_eq!(outcome.output, "Available exits: east, north.");
    }

    #[test]
    fn supports_common_parser_aliases_and_defaults() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) = game.handle_command("look at key").expect("look at")
        else {
            panic!("look at should continue");
        };
        assert_eq!(outcome.output, "A key.");
        assert_eq!(game.state().turn, 0);

        let CommandResult::Continue(outcome) = game.handle_command("go n").expect("go n") else {
            panic!("go n should continue");
        };
        assert!(outcome.output.contains("Hall"));
        assert_eq!(game.current_room_id(), "hall");

        let CommandResult::Continue(outcome) = game.handle_command("listen").expect("listen")
        else {
            panic!("listen should continue");
        };
        assert_eq!(outcome.output, "You hear nothing unusual.");

        let CommandResult::Continue(outcome) = game.handle_command("smell").expect("smell") else {
            panic!("smell should continue");
        };
        assert_eq!(outcome.output, "You smell nothing unusual.");

        let CommandResult::Continue(outcome) = game.handle_command("search").expect("search")
        else {
            panic!("search should continue");
        };
        assert_eq!(outcome.output, "You find nothing new.");

        let CommandResult::Continue(outcome) = game.handle_command("wait").expect("wait") else {
            panic!("wait should continue");
        };
        assert_eq!(outcome.output, "Time passes.");
    }

    #[test]
    fn enters_directions_and_uses_entered_things() {
        let mut game = Game::new(test_world()).expect("valid world");
        game.handle_command("north").expect("move north");

        let CommandResult::Continue(outcome) = game.handle_command("enter south").expect("enter")
        else {
            panic!("enter direction should continue");
        };
        assert!(outcome.output.contains("Foyer"));
        assert_eq!(game.current_room_id(), "foyer");

        let CommandResult::Continue(outcome) = game
            .handle_command("enter hall")
            .expect("enter room by exit target")
        else {
            panic!("enter room should continue");
        };
        assert!(outcome.output.contains("Hall"));
        assert_eq!(game.current_room_id(), "hall");

        let CommandResult::Continue(outcome) =
            game.handle_command("go through foyer").expect("go through")
        else {
            panic!("go through should continue");
        };
        assert!(outcome.output.contains("Foyer"));
        assert_eq!(game.current_room_id(), "foyer");
    }

    #[test]
    fn parses_two_object_use_commands() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) = game
            .handle_command("use key on table")
            .expect("use with target")
        else {
            panic!("use with target should continue");
        };
        assert_eq!(outcome.output, "Nothing happens.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::UseWith {
                item_id: "brass_key".to_string(),
                target_id: "table".to_string()
            }]
        );
    }

    #[test]
    fn takes_and_drops_all_available_items() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) = game.handle_command("take all").expect("take all")
        else {
            panic!("take all should continue");
        };
        assert_eq!(
            outcome.output,
            "You take the brass key.\nYou take the wool cloak."
        );
        assert!(game.has("brass_key"));
        assert!(game.has("wool_cloak"));

        game.handle_command("wear cloak").expect("wear cloak");
        let CommandResult::Continue(outcome) = game.handle_command("drop all").expect("drop all")
        else {
            panic!("drop all should continue");
        };
        assert_eq!(outcome.output, "You drop the brass key.");
        assert!(!game.has("brass_key"));
        assert!(game.has("wool_cloak"));
    }

    #[test]
    fn resolves_recent_object_pronouns() {
        let mut game = Game::new(test_world()).expect("valid world");

        game.handle_command("take key").expect("take key");
        let CommandResult::Continue(outcome) = game.handle_command("x it").expect("examine it")
        else {
            panic!("examine it should continue");
        };
        assert_eq!(outcome.output, "A key.");

        let CommandResult::Continue(outcome) = game.handle_command("drop it").expect("drop it")
        else {
            panic!("drop it should continue");
        };
        assert_eq!(outcome.output, "You drop the brass key.");
        assert!(!game.has("brass_key"));
    }

    #[test]
    fn repeats_last_advancing_command_and_undoes_state() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) = game.handle_command("again").expect("again command")
        else {
            panic!("again should continue");
        };
        assert_eq!(outcome.output, "Nothing to do again.");

        game.handle_command("take key").expect("take key");
        assert!(game.has("brass_key"));
        assert_eq!(game.state().turn, 1);

        let CommandResult::Continue(outcome) = game.handle_command("undo").expect("undo command")
        else {
            panic!("undo should continue");
        };
        assert_eq!(outcome.output, "Undone.");
        assert!(!game.has("brass_key"));
        assert_eq!(game.state().turn, 0);

        let CommandResult::Continue(outcome) = game.handle_command("again").expect("again command")
        else {
            panic!("again should continue");
        };
        assert_eq!(outcome.output, "You take the brass key.");
        assert!(game.has("brass_key"));
        assert_eq!(game.state().turn, 1);
    }

    #[test]
    fn resolves_objects_with_optional_articles() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) = game
            .handle_command("take the key")
            .expect("take article command")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "You take the brass key.");

        let CommandResult::Continue(outcome) = game.handle_command("drop a key").expect("drop")
        else {
            panic!("drop should continue");
        };
        assert_eq!(outcome.output, "You drop the brass key.");

        game.handle_command("take the brass key").expect("take key");
        let CommandResult::Continue(outcome) =
            game.handle_command("use the brass key").expect("use")
        else {
            panic!("use should continue");
        };
        assert_eq!(outcome.output, "You find no use for the brass key.");

        game.handle_command("drop the key").expect("drop key");
        let CommandResult::Continue(outcome) = game
            .handle_command("put the brass key in the wooden box")
            .expect("put in")
        else {
            panic!("put should continue");
        };
        assert_eq!(outcome.output, "You aren't carrying that.");

        game.handle_command("take the key").expect("take key");
        let CommandResult::Continue(outcome) = game
            .handle_command("put the brass key in the wooden box")
            .expect("put in")
        else {
            panic!("put should continue");
        };
        assert_eq!(outcome.output, "You put the brass key in the wooden box.");

        let CommandResult::Continue(outcome) = game
            .handle_command("look in the wooden box")
            .expect("look in")
        else {
            panic!("look should continue");
        };
        assert_eq!(outcome.output, "In the wooden box is a brass key.");

        let CommandResult::Continue(outcome) = game
            .handle_command("take the key from the box")
            .expect("take from")
        else {
            panic!("take from should continue");
        };
        assert_eq!(outcome.output, "You take the brass key.");

        let CommandResult::Continue(outcome) = game
            .handle_command("talk to the caretaker")
            .expect("talk article command")
        else {
            panic!("talk should continue");
        };
        assert_eq!(outcome.output, "The caretaker has nothing to say.");
    }

    #[test]
    fn validates_loaded_state_against_world() {
        let mut game = Game::new(test_world()).expect("valid world");
        let mut state = game.state().clone();
        state.current_room = "missing".to_string();

        let err = game.replace_state(state).expect_err("state should fail");
        assert!(err.to_string().contains("current room 'missing'"));

        let mut state = game.state().clone();
        state.visited_rooms.insert("missing".to_string());

        let err = game.replace_state(state).expect_err("state should fail");
        assert!(err.to_string().contains("visited room 'missing'"));
    }

    #[test]
    fn validates_world_graph_before_play() {
        let mut world = test_world();
        world.metadata.as_mut().expect("metadata").start = "missing_start".to_string();
        world
            .rooms
            .get_mut("foyer")
            .expect("foyer")
            .exits
            .insert("trapdoor".to_string(), Exit::open("cellar"));
        world.rooms.get_mut("foyer").expect("foyer").exits.insert(
            "locked".to_string(),
            Exit {
                to: "hall".to_string(),
                requires: Some("missing_key".to_string()),
                locked_msg: None,
            },
        );
        world.things.get_mut("brass_key").expect("key").location = "missing_room".to_string();
        world
            .things
            .get_mut("wool_cloak")
            .expect("cloak")
            .aliases
            .push("key".to_string());

        let report = world.validate();
        let errors = report
            .errors()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();
        let warnings = report
            .warnings()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();

        assert!(!report.is_success());
        assert!(
            errors
                .iter()
                .any(|message| message.contains("start room 'missing_start'"))
        );
        assert!(
            errors
                .iter()
                .any(|message| message.contains("exit 'trapdoor' targets missing room 'cellar'"))
        );
        assert!(
            errors
                .iter()
                .any(|message| message
                    .contains("exit 'locked' requires missing thing 'missing_key'"))
        );
        assert!(
            errors
                .iter()
                .any(|message| message.contains("thing 'brass_key' starts in missing location"))
        );
        assert!(
            warnings
                .iter()
                .any(|message| message.contains("object name or alias 'key' is shared"))
        );
    }

    #[test]
    fn validates_recursive_thing_locations() {
        let mut world = test_world();
        world.things.get_mut("wooden_box").expect("box").location = "table".to_string();
        world.things.get_mut("table").expect("table").location = "wooden_box".to_string();

        let report = world.validate();

        assert!(
            report
                .errors()
                .any(|issue| issue.message.contains("recursive containment cycle"))
        );
    }

    #[test]
    fn validates_thing_open_and_lock_metadata() {
        let mut world = test_world();
        world.things.get_mut("wool_cloak").expect("cloak").open = true;
        world.things.get_mut("caretaker").expect("caretaker").locked = true;
        world.things.get_mut("wooden_box").expect("box").key = Some("missing_key".to_string());

        let report = world.validate();
        let errors = report
            .errors()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();

        assert!(
            errors
                .iter()
                .any(|message| message.contains("thing 'wool_cloak' is open but not openable"))
        );
        assert!(
            errors
                .iter()
                .any(|message| message.contains("thing 'caretaker' is locked but not lockable"))
        );
        assert!(
            errors
                .iter()
                .any(|message| message.contains("references missing key 'missing_key'"))
        );
    }

    #[test]
    fn exit_display_is_controlled_by_world_settings() {
        let game = Game::new(test_world()).expect("valid world");
        assert!(!game.look().expect("look").contains("Available exits"));

        let mut world = test_world();
        world.settings.exits.show = true;
        let game = Game::new(world).expect("valid world");
        let output = game.look().expect("look");

        assert!(output.contains("Available exits: east, north."));
    }

    #[test]
    fn guarded_exit_requires_inventory_item() {
        let mut game = Game::new(test_world()).expect("valid world");
        let CommandResult::Continue(outcome) = game.handle_command("east").expect("east command")
        else {
            panic!("east should continue");
        };
        assert_eq!(outcome.output, "The study door is locked.");

        game.handle_command("take key").expect("take key");
        let CommandResult::Continue(outcome) = game.handle_command("east").expect("east command")
        else {
            panic!("east should continue");
        };
        assert!(outcome.output.contains("Study"));
    }

    #[test]
    fn puts_and_takes_items_from_supporters_and_containers() {
        let mut game = Game::new(test_world()).expect("valid world");
        game.handle_command("take key").expect("take key");

        let CommandResult::Continue(outcome) =
            game.handle_command("put key in box").expect("put command")
        else {
            panic!("put should continue");
        };
        assert_eq!(outcome.output, "You put the brass key in the wooden box.");

        let CommandResult::Continue(outcome) =
            game.handle_command("look in box").expect("look in command")
        else {
            panic!("look should continue");
        };
        assert_eq!(outcome.output, "In the wooden box is a brass key.");

        let CommandResult::Continue(outcome) = game
            .handle_command("take key from box")
            .expect("take from command")
        else {
            panic!("take from should continue");
        };
        assert_eq!(outcome.output, "You take the brass key.");

        let CommandResult::Continue(outcome) = game
            .handle_command("put key on table")
            .expect("put on command")
        else {
            panic!("put on should continue");
        };
        assert_eq!(outcome.output, "You put the brass key on the table.");

        let CommandResult::Continue(outcome) = game
            .handle_command("look on table")
            .expect("look on command")
        else {
            panic!("look on should continue");
        };
        assert_eq!(outcome.output, "On the table is a brass key.");
    }

    #[test]
    fn hidden_things_are_unavailable_until_revealed() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) =
            game.handle_command("look in box").expect("look in command")
        else {
            panic!("look should continue");
        };
        assert_eq!(outcome.output, "There is nothing in the wooden box.");

        let CommandResult::Continue(outcome) =
            game.handle_command("take note").expect("take command")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "You don't see that here.");
        assert!(!game.visible("hidden_note"));

        assert!(game.reveal_thing("hidden_note"));
        assert!(game.visible("hidden_note"));

        let CommandResult::Continue(outcome) =
            game.handle_command("look in box").expect("look in command")
        else {
            panic!("look should continue");
        };
        assert_eq!(outcome.output, "In the wooden box is a hidden note.");

        let CommandResult::Continue(outcome) = game
            .handle_command("take note from box")
            .expect("take command")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "You take the hidden note.");
    }

    #[test]
    fn opens_closes_locks_and_unlocks_things() {
        let mut game = Game::new(test_world()).expect("valid world");
        game.handle_command("take key").expect("take key");
        game.handle_command("put key in box").expect("put key");

        let CommandResult::Continue(outcome) =
            game.handle_command("close box").expect("close command")
        else {
            panic!("close should continue");
        };
        assert_eq!(outcome.output, "You close the wooden box.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Close {
                thing_id: "wooden_box".to_string()
            }]
        );

        let CommandResult::Continue(outcome) =
            game.handle_command("look in box").expect("look command")
        else {
            panic!("look should continue");
        };
        assert_eq!(outcome.output, "The wooden box is closed.");

        let CommandResult::Continue(outcome) = game
            .handle_command("take key from box")
            .expect("take from command")
        else {
            panic!("take should continue");
        };
        assert_eq!(outcome.output, "The wooden box is closed.");

        let CommandResult::Continue(outcome) =
            game.handle_command("open box").expect("open command")
        else {
            panic!("open should continue");
        };
        assert_eq!(outcome.output, "You open the wooden box.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Open {
                thing_id: "wooden_box".to_string()
            }]
        );

        game.handle_command("take key from box").expect("take key");
        game.handle_command("close box").expect("close box");

        let CommandResult::Continue(outcome) = game
            .handle_command("lock box with key")
            .expect("lock command")
        else {
            panic!("lock should continue");
        };
        assert_eq!(outcome.output, "You lock the wooden box.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Lock {
                thing_id: "wooden_box".to_string()
            }]
        );

        let CommandResult::Continue(outcome) =
            game.handle_command("open box").expect("open command")
        else {
            panic!("open should continue");
        };
        assert_eq!(outcome.output, "The wooden box is locked.");

        let CommandResult::Continue(outcome) = game
            .handle_command("unlock box with cloak")
            .expect("unlock command")
        else {
            panic!("unlock should continue");
        };
        assert_eq!(outcome.output, "You aren't carrying that.");

        let CommandResult::Continue(outcome) = game
            .handle_command("unlock box with key")
            .expect("unlock command")
        else {
            panic!("unlock should continue");
        };
        assert_eq!(outcome.output, "You unlock the wooden box.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Unlock {
                thing_id: "wooden_box".to_string()
            }]
        );
    }

    #[test]
    fn wears_and_removes_wearable_items() {
        let mut game = Game::new(test_world()).expect("valid world");
        game.handle_command("take cloak").expect("take cloak");

        let CommandResult::Continue(outcome) =
            game.handle_command("wear cloak").expect("wear command")
        else {
            panic!("wear should continue");
        };
        assert_eq!(outcome.output, "You put on the wool cloak.");

        let CommandResult::Continue(outcome) = game.handle_command("i").expect("inventory") else {
            panic!("inventory should continue");
        };
        assert_eq!(outcome.output, "You are carrying a wool cloak (worn).");

        let CommandResult::Continue(outcome) = game.handle_command("drop cloak").expect("drop")
        else {
            panic!("drop should continue");
        };
        assert_eq!(outcome.output, "You need to remove the wool cloak first.");

        let CommandResult::Continue(outcome) =
            game.handle_command("remove cloak").expect("remove command")
        else {
            panic!("remove should continue");
        };
        assert_eq!(outcome.output, "You remove the wool cloak.");
    }

    #[test]
    fn emits_due_timer_events_after_turns_advance() {
        let mut game = Game::new(test_world()).expect("valid world");
        game.schedule_event(2, "bell");

        let CommandResult::Continue(outcome) = game.handle_command("take key").expect("turn one")
        else {
            panic!("take should continue");
        };
        assert!(!outcome.events.contains(&GameEvent::Timer {
            event_name: "bell".to_string()
        }));

        let CommandResult::Continue(outcome) = game.handle_command("drop key").expect("turn two")
        else {
            panic!("drop should continue");
        };
        assert!(outcome.events.contains(&GameEvent::Timer {
            event_name: "bell".to_string()
        }));
    }

    #[test]
    fn tracks_scenes_chapters_and_scene_scoped_timers() {
        let mut game = Game::new(test_world()).expect("valid world");

        assert_eq!(game.current_scene(), None);
        assert_eq!(game.current_chapter(), None);

        game.set_chapter("arrival");
        game.start_scene("foyer_puzzle");
        game.schedule_scene_event(2, "scene_bell", "foyer_puzzle");
        assert_eq!(game.current_scene(), Some("foyer_puzzle"));
        assert_eq!(game.current_chapter(), Some("arrival"));

        game.handle_command("take key").expect("turn one");
        let CommandResult::Continue(outcome) = game.handle_command("drop key").expect("turn two")
        else {
            panic!("drop should continue");
        };
        assert!(outcome.events.contains(&GameEvent::Timer {
            event_name: "scene_bell".to_string()
        }));

        game.schedule_scene_event(1, "cancelled_bell", "foyer_puzzle");
        assert_eq!(
            game.end_scene(Some("foyer_puzzle")),
            Some("foyer_puzzle".to_string())
        );
        assert_eq!(game.current_scene(), None);
        let CommandResult::Continue(outcome) = game.handle_command("take key").expect("turn three")
        else {
            panic!("take should continue");
        };
        assert!(!outcome.events.contains(&GameEvent::Timer {
            event_name: "cancelled_bell".to_string()
        }));
    }

    #[test]
    fn random_state_advances_deterministically() {
        let state = GameState::DEFAULT_RANDOM_SEED;
        let first = random_bounded(state, 10);
        let second = random_bounded(first.0, 10);

        assert_eq!(first, random_bounded(state, 10));
        assert_eq!(second, random_bounded(first.0, 10));
        assert_ne!(first.0, state);
        assert_ne!(second.0, first.0);
    }

    #[test]
    fn uses_accessible_things() {
        let mut game = Game::new(test_world()).expect("valid world");
        let CommandResult::Continue(outcome) = game.handle_command("use key").expect("use command")
        else {
            panic!("use should continue");
        };

        assert_eq!(outcome.output, "You find no use for the brass key.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Use {
                thing_id: "brass_key".to_string()
            }]
        );
    }

    #[test]
    fn reads_accessible_things() {
        let mut game = Game::new(test_world()).expect("valid world");

        let CommandResult::Continue(outcome) =
            game.handle_command("read key").expect("read command")
        else {
            panic!("read should continue");
        };

        assert_eq!(outcome.output, "The key is stamped STUDY.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Read {
                thing_id: "brass_key".to_string()
            }]
        );

        let CommandResult::Continue(outcome) =
            game.handle_command("read cloak").expect("read command")
        else {
            panic!("read should continue");
        };

        assert_eq!(
            outcome.output,
            "There is nothing to read on the wool cloak."
        );
    }

    #[test]
    fn talks_to_actor_things() {
        let mut game = Game::new(test_world()).expect("valid world");
        let CommandResult::Continue(outcome) = game
            .handle_command("talk to caretaker")
            .expect("talk command")
        else {
            panic!("talk should continue");
        };

        assert_eq!(outcome.output, "The caretaker has nothing to say.");
        assert_eq!(
            outcome.events,
            vec![GameEvent::Talk {
                thing_id: "caretaker".to_string()
            }]
        );

        let CommandResult::Continue(outcome) =
            game.handle_command("talk to table").expect("talk command")
        else {
            panic!("talk should continue");
        };
        assert_eq!(outcome.output, "The table does not respond.");
    }

    #[test]
    fn asks_actor_things_about_topics() {
        let mut game = Game::new(test_world()).expect("valid world");
        let CommandResult::Continue(outcome) = game
            .handle_command("ask caretaker about key")
            .expect("ask command")
        else {
            panic!("ask should continue");
        };

        assert_eq!(
            outcome.output,
            "The caretaker has nothing to say about key."
        );
        assert_eq!(
            outcome.events,
            vec![GameEvent::Ask {
                thing_id: "caretaker".to_string(),
                topic: "key".to_string()
            }]
        );

        let CommandResult::Continue(outcome) = game
            .handle_command("ask table about key")
            .expect("ask command")
        else {
            panic!("ask should continue");
        };
        assert_eq!(outcome.output, "The table does not respond.");
    }

    #[test]
    fn supports_topic_aliases_tell_show_give_and_actor_memory() {
        let mut game = Game::new(test_world()).expect("valid world");
        game.handle_command("take key").expect("take key");

        let CommandResult::Continue(outcome) = game
            .handle_command("ask caretaker about brass key")
            .expect("ask alias command")
        else {
            panic!("ask should continue");
        };
        assert_eq!(
            outcome.events,
            vec![GameEvent::Ask {
                thing_id: "caretaker".to_string(),
                topic: "key".to_string()
            }]
        );
        assert_eq!(game.actor_memory("caretaker", "asked:key"), 1);

        let CommandResult::Continue(outcome) = game
            .handle_command("ask caretaker about house")
            .expect("ask gated command")
        else {
            panic!("ask should continue");
        };
        assert_eq!(
            outcome.output,
            "The caretaker has nothing to say about house."
        );
        assert!(outcome.events.is_empty());

        game.flag("knows_house");
        let CommandResult::Continue(outcome) = game
            .handle_command("tell caretaker about glass house")
            .expect("tell command")
        else {
            panic!("tell should continue");
        };
        assert_eq!(
            outcome.events,
            vec![GameEvent::Tell {
                thing_id: "caretaker".to_string(),
                topic: "house".to_string()
            }]
        );
        assert_eq!(game.actor_memory("caretaker", "told:house"), 1);

        let CommandResult::Continue(outcome) = game
            .handle_command("show key to caretaker")
            .expect("show command")
        else {
            panic!("show should continue");
        };
        assert_eq!(
            outcome.events,
            vec![GameEvent::Show {
                thing_id: "caretaker".to_string(),
                item_id: "brass_key".to_string()
            }]
        );
        assert_eq!(game.actor_memory("caretaker", "shown:brass_key"), 1);

        let CommandResult::Continue(outcome) = game
            .handle_command("give key to caretaker")
            .expect("give command")
        else {
            panic!("give should continue");
        };
        assert_eq!(
            outcome.events,
            vec![GameEvent::Give {
                thing_id: "caretaker".to_string(),
                item_id: "brass_key".to_string()
            }]
        );
        assert_eq!(game.actor_memory("caretaker", "given:brass_key"), 1);
        assert!(game.has("brass_key"));
    }

    fn test_world() -> World {
        World {
            metadata: Some(GameMetadata {
                title: "Test".to_string(),
                author: None,
                start: "foyer".to_string(),
                id: Some("test".to_string()),
                version: Some("1.0.0".to_string()),
            }),
            settings: GameSettings::default(),
            rooms: BTreeMap::from([
                (
                    "foyer".to_string(),
                    Room {
                        id: "foyer".to_string(),
                        name: "Foyer".to_string(),
                        desc: "A small foyer.".to_string(),
                        exits: BTreeMap::from([
                            ("north".to_string(), Exit::open("hall")),
                            (
                                "east".to_string(),
                                Exit {
                                    to: "study".to_string(),
                                    requires: Some("brass_key".to_string()),
                                    locked_msg: Some("The study door is locked.".to_string()),
                                },
                            ),
                        ]),
                    },
                ),
                (
                    "hall".to_string(),
                    Room {
                        id: "hall".to_string(),
                        name: "Hall".to_string(),
                        desc: "A narrow hall.".to_string(),
                        exits: BTreeMap::from([("south".to_string(), Exit::open("foyer"))]),
                    },
                ),
                (
                    "study".to_string(),
                    Room {
                        id: "study".to_string(),
                        name: "Study".to_string(),
                        desc: "A quiet study.".to_string(),
                        exits: BTreeMap::from([("west".to_string(), Exit::open("foyer"))]),
                    },
                ),
            ]),
            things: BTreeMap::from([
                (
                    "brass_key".to_string(),
                    Thing {
                        id: "brass_key".to_string(),
                        name: "brass key".to_string(),
                        aliases: vec!["key".to_string(), "brass key".to_string()],
                        location: "foyer".to_string(),
                        portable: true,
                        wearable: false,
                        actor: false,
                        hidden: false,
                        desc: Some("A key.".to_string()),
                        read: Some("The key is stamped STUDY.".to_string()),
                        openable: false,
                        open: false,
                        lockable: false,
                        locked: false,
                        key: None,
                        kind: ThingKind::Object,
                    },
                ),
                (
                    "wool_cloak".to_string(),
                    Thing {
                        id: "wool_cloak".to_string(),
                        name: "wool cloak".to_string(),
                        aliases: vec!["cloak".to_string(), "wool cloak".to_string()],
                        location: "foyer".to_string(),
                        portable: true,
                        wearable: true,
                        actor: false,
                        hidden: false,
                        desc: Some("A cloak of heavy grey wool.".to_string()),
                        read: None,
                        openable: false,
                        open: false,
                        lockable: false,
                        locked: false,
                        key: None,
                        kind: ThingKind::Object,
                    },
                ),
                (
                    "caretaker".to_string(),
                    Thing {
                        id: "caretaker".to_string(),
                        name: "caretaker".to_string(),
                        aliases: vec!["caretaker".to_string()],
                        location: "foyer".to_string(),
                        portable: false,
                        wearable: false,
                        actor: true,
                        hidden: false,
                        desc: Some("The caretaker waits with folded hands.".to_string()),
                        read: None,
                        openable: false,
                        open: false,
                        lockable: false,
                        locked: false,
                        key: None,
                        kind: ThingKind::Object,
                    },
                ),
                (
                    "wooden_box".to_string(),
                    Thing {
                        id: "wooden_box".to_string(),
                        name: "wooden box".to_string(),
                        aliases: vec!["box".to_string(), "wooden box".to_string()],
                        location: "foyer".to_string(),
                        portable: false,
                        wearable: false,
                        actor: false,
                        hidden: false,
                        desc: Some("A small wooden box.".to_string()),
                        read: None,
                        openable: true,
                        open: true,
                        lockable: true,
                        locked: false,
                        key: Some("brass_key".to_string()),
                        kind: ThingKind::Container,
                    },
                ),
                (
                    "table".to_string(),
                    Thing {
                        id: "table".to_string(),
                        name: "table".to_string(),
                        aliases: vec!["table".to_string()],
                        location: "foyer".to_string(),
                        portable: false,
                        wearable: false,
                        actor: false,
                        hidden: false,
                        desc: Some("A narrow table.".to_string()),
                        read: None,
                        openable: false,
                        open: false,
                        lockable: false,
                        locked: false,
                        key: None,
                        kind: ThingKind::Supporter,
                    },
                ),
                (
                    "hidden_note".to_string(),
                    Thing {
                        id: "hidden_note".to_string(),
                        name: "hidden note".to_string(),
                        aliases: vec!["note".to_string(), "hidden note".to_string()],
                        location: "wooden_box".to_string(),
                        portable: true,
                        wearable: false,
                        actor: false,
                        hidden: true,
                        desc: Some("A note tucked into a false seam.".to_string()),
                        read: Some("The note says LOOK CLOSER.".to_string()),
                        openable: false,
                        open: false,
                        lockable: false,
                        locked: false,
                        key: None,
                        kind: ThingKind::Object,
                    },
                ),
            ]),
            verbs: BTreeMap::new(),
            actor_topics: BTreeMap::from([(
                "caretaker".to_string(),
                BTreeMap::from([
                    (
                        "key".to_string(),
                        ActorTopic {
                            id: "key".to_string(),
                            aliases: vec!["brass key".to_string()],
                            requires: None,
                        },
                    ),
                    (
                        "house".to_string(),
                        ActorTopic {
                            id: "house".to_string(),
                            aliases: vec!["glass house".to_string()],
                            requires: Some("knows_house".to_string()),
                        },
                    ),
                ]),
            )]),
        }
    }
}
