//! This module contains the core logic for the
//! Iterated Prisoner's Dilemma Tournament

// use boa_engine::{
//     js_string,
//     object::{builtins::JsArray, ObjectInitializer},
//     property::Attribute,
//     Context, JsValue, Source,
// };
#![deny(clippy::unwrap_used, clippy::expect_used)]
use mlua::{Function, Lua, LuaSerdeExt};
use rustpython::InterpreterConfig;
use rustpython_vm::{
    builtins::PyTuple, convert::IntoObject, function::FuncArgs, Interpreter, PyObjectRef, Settings,
};

use crate::error::ExecutionError;

thread_local! {
    pub static PYTHON: Interpreter = {
        // let mut settings = Settings::default();
        // settings.utf8_mode = 0;
        // settings.import_site = false;
        InterpreterConfig::new()
            .settings(Settings::default())
            .init_stdlib()
            // .settings(settings)
            // .init_hook(Box::new(|vm| {
            //     vm.add_native_modules(rustpython_stdlib::get_module_inits().filter(
            //         |(name, _)| {
            //             name == "builtins"  // Surely we don't need builtins
            //                 || name == "json"
            //                 || name == "math"
            //                 || name == "random"
            //                 || name == "re"
            //         },
            //     ));
            // }))
            .interpreter()
    };
}
/// This enum represents the different player-agents
/// that can play in the tournament
/// The `history` parameter will always be a list of tuples
/// where each tuple contains 2 boolean values representing
/// the actions of (you, them) in the previous rounds
/// The `storage` parameter will be a string that can be used
/// to store any information between rounds
/// The return value of the function should be a tuple of 2 values
/// where the first value is a boolean representing the action of the player
/// and the second value is a string representing the storage value
#[derive(Clone, Debug)]
pub enum Executor {
    /// Lua programs must only contain ae
    /// anonymous function definition that takes
    /// 2 arguments: `history` and `storage`
    /// and returns a tuple of 2 values: `action` and `storage`
    Lua(String),
    /// Python programs must only contain a single
    /// function definition named `main` that takes
    /// 2 arguments: `history` and `storage`
    /// and returns a tuple of 2 values: `action` and `storage`
    Python(String),
    JavaScript(String),
    WASM(String),
}
impl Executor {
    // (you, them)
    // true = cooperate, false = defect
    pub fn run(
        &self,
        history: &[(bool, bool)],
        storage: String,
    ) -> Result<(bool, String), ExecutionError> {
        match self {
            Executor::Lua(program) => {
                let lua = Lua::new();
                lua.sandbox(true).map_err(|_| {
                    ExecutionError::InitializationError("Sandboxing failed".to_string())
                })?;
                // Set memory to 1GB
                lua.set_memory_limit(1024 * 1024 * 1024).map_err(|_| {
                    ExecutionError::InitializationError("Memory limit failed".to_string())
                })?;
                let function: Function = lua
                    .load(program)
                    .eval()
                    .map_err(|_| ExecutionError::SyntaxError)?;

                function
                    .call::<(bool, String)>((
                        #[allow(clippy::expect_used)]
                        lua.to_value(&history).expect("Could not serialize history"),
                        #[allow(clippy::expect_used)]
                        lua.to_value(&storage).expect("Could not serialize storage"),
                    ))
                    .map_err(|err| ExecutionError::DeserializationError(err.to_string()))
            }
            Executor::Python(program) => {
                let output: Result<(bool, String), ExecutionError> = PYTHON.with(|interpreter| {
                    interpreter.enter(|vm| {
                        let scope = vm.new_scope_with_builtins();
                        vm.run_block_expr(scope.clone(), program)
                            .map_err(|_| ExecutionError::SyntaxError)?;
                        let function = scope.globals.get_item("main", vm).map_err(|_| {
                            ExecutionError::InitializationError(
                                "Main function not found".to_string(),
                            )
                        })?;

                        let args_history: PyObjectRef = vm
                            .ctx
                            .new_list(
                                history
                                    .into_iter()
                                    .map(|(you, them)| {
                                        vm.ctx
                                            .new_tuple(vec![
                                                vm.ctx.new_bool(*you).into(),
                                                vm.ctx.new_bool(*them).into(),
                                            ])
                                            .into()
                                    })
                                    .collect::<Vec<PyObjectRef>>(),
                            )
                            .into();
                        let result = function
                            .to_callable()
                            .ok_or(ExecutionError::InitializationError(
                                "Expected a function".to_string(),
                            ))
                            .map(|callable| {
                                callable.invoke(
                                    FuncArgs::from(vec![
                                        args_history,
                                        vm.ctx.new_str(storage).into(),
                                    ]),
                                    vm,
                                )
                            })?
                            .map_err(|_| ExecutionError::RuntimeError("Wtf".to_string()))?;
                        let result_tuple = result
                            .downcast::<PyTuple>()
                            .map_err(|_| {
                                ExecutionError::DeserializationError(
                                    "Could not deserialize output into a tuple".to_string(),
                                )
                            })?
                            .into_object();
                        let result_tuple = result_tuple.to_sequence();
                        let action: bool = result_tuple
                            .get_item(0, vm)
                            .map_err(|_| {
                                ExecutionError::DeserializationError("Could not des".to_string())
                            })?
                            .try_into_value(vm)
                            .map_err(|_| {
                                ExecutionError::DeserializationError("Could not des".to_string())
                            })?;
                        let new_storage: String = result_tuple
                            .get_item(1, vm)
                            .map_err(|_| {
                                ExecutionError::DeserializationError("Could not des".to_string())
                            })?
                            .try_into_value(vm)
                            .map_err(|_| {
                                ExecutionError::DeserializationError("Could not des".to_string())
                            })?;
                        Ok((action, new_storage))
                    })
                });
                output
            }
            Executor::JavaScript(_program) => {
                // let mut context = Context::default();
                // // Evaluate function definition
                // context.eval(Source::from_bytes(program)).unwrap();
                // // Create an object that can be used in eval calls.
                // let history = {
                //     history.into_iter().map(|x| {
                //         Into::<JsValue>::into(JsArray::from_iter::<Vec<JsValue>>(
                //             vec![JsValue::Boolean(x.0), JsValue::Boolean(x.1)],
                //             &mut context,
                //         ))
                //     })
                // };
                // // let args_history = ;
                // let arg = ObjectInitializer::new(&mut context)
                //     .property(
                //         js_string!("history"),
                //         JsArray::from_iter(history.into_iter(), &mut context),
                //         Attribute::READONLY,
                //     )
                //     .property(
                //         js_string!("storage"),
                //         js_string!(storage),
                //         Attribute::READONLY,
                //     )
                //     .build();
                // context
                //     .register_global_property(js_string!("input"), arg, Attribute::all())
                //     .expect("property shouldn't exist");

                // let value = context
                //     .eval(Source::from_bytes("main(input.history, input.storage)"))
                //     .unwrap();
                todo!()
            }
            Executor::WASM(_) => {
                todo!()
            }
        }
    }
}
#[derive(Clone, Debug)]
pub struct Player {
    pub name: String,
    pub executor: Executor,
    storage: String,
}
impl Player {
    pub fn new(name: String, executor: Executor) -> Player {
        Player {
            name,
            executor,
            storage: String::new(),
        }
    }
    pub fn with_storage(mut self, storage: String) -> Player {
        self.storage = storage;
        self
    }
    pub fn run(&mut self, history: &[(bool, bool)]) -> bool {
        let Ok((action, storage)) = self.executor.run(history, self.storage.clone()) else {
            todo!()
        };
        self.storage = storage;
        action
    }
    pub fn reset_storage(&mut self) {
        self.storage = String::new();
    }
}
pub struct TournamentConfig {
    pub players: Vec<Player>,
    pub rounds: i32,
    pub mutual_win_score: i32,
    pub mutual_loss_score: i32,
    pub win_score: i32,
    pub loss_score: i32,
}
impl TournamentConfig {
    pub fn new() -> TournamentConfig {
        TournamentConfig::default()
    }
    pub fn with_players(mut self, players: Vec<Player>) -> TournamentConfig {
        self.players = players;
        self
    }
    pub fn with_rounds(mut self, rounds: i32) -> TournamentConfig {
        self.rounds = rounds;
        self
    }
    pub fn with_mutual_win_score(mut self, score: i32) -> TournamentConfig {
        self.mutual_win_score = score;
        self
    }
    pub fn with_mutual_loss_score(mut self, score: i32) -> TournamentConfig {
        self.mutual_loss_score = score;
        self
    }
    pub fn with_win_score(mut self, score: i32) -> TournamentConfig {
        self.win_score = score;
        self
    }
    pub fn with_loss_score(mut self, score: i32) -> TournamentConfig {
        self.loss_score = score;
        self
    }
    /// Modeled after https://ncase.me/trust/
    /// (higher the better)
    pub fn with_nick_style_score(mut self) -> TournamentConfig {
        self.mutual_win_score = 2;
        self.mutual_loss_score = 0;
        self.win_score = 3;
        self.loss_score = -1;
        self
    }
    /// Modeled after the classic prisoner scenario scores
    /// (lower the better)
    pub fn with_classic_style_score(mut self) -> TournamentConfig {
        self.mutual_win_score = 1;
        self.mutual_loss_score = 2;
        self.win_score = 0;
        self.loss_score = 3;
        self
    }
}
impl Default for TournamentConfig {
    fn default() -> TournamentConfig {
        TournamentConfig {
            players: vec![],
            rounds: 100,
            // Default to Nick style
            mutual_win_score: 2,
            mutual_loss_score: 0,
            win_score: 3,
            loss_score: -1,
        }
    }
}

pub struct Tournament {
    pub config: TournamentConfig,
}

impl Default for Tournament {
    fn default() -> Self {
        Self::new()
    }
}

impl Tournament {
    pub fn new() -> Tournament {
        Tournament {
            config: TournamentConfig::default(),
        }
    }
    pub fn with_config(config: TournamentConfig) -> Tournament {
        Tournament { config }
    }
    pub fn run(&mut self) -> Vec<i32> {
        let mut scores = vec![0; self.config.players.len()];
        for (i, player1) in self.config.players.iter().enumerate() {
            for (j, player2) in self.config.players.iter().enumerate() {
                // don't let a player play against themselves
                if i == j {
                    continue;
                }
                let mut player1 = player1.clone();
                let mut player2 = player2.clone();
                let mut player1_history = vec![];
                let mut player2_history = vec![];
                for _ in 0..self.config.rounds {
                    let player1_action = player1.run(&player1_history);
                    let player2_action = player2.run(&player2_history);
                    let player1_score = if player1_action && player2_action {
                        self.config.mutual_win_score
                    } else if !player1_action && !player2_action {
                        self.config.mutual_loss_score
                    } else if !player1_action && player2_action {
                        self.config.win_score
                    } else {
                        self.config.loss_score
                    };
                    let player2_score = if player2_action && player1_action {
                        self.config.mutual_win_score
                    } else if !player2_action && !player1_action {
                        self.config.mutual_loss_score
                    } else if !player2_action && player1_action {
                        self.config.win_score
                    } else {
                        self.config.loss_score
                    };
                    scores[i] += player1_score;
                    scores[j] += player2_score;
                    player1_history.push((player1_action, player2_action));
                    player2_history.push((player2_action, player1_action));
                }
            }
        }
        scores
    }
}
