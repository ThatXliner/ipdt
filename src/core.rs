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
use serde_json::Value;

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
    /// We use JSON and sys.argv to pass the history and storage
    /// to the Piston executor. Your program should output
    /// your next move and storage as a JSON object of
    /// {action: boolean, storage: string} If your language does not support
    /// JSON input built-in, well you're screwed
    /// (language, program)
    Piston(String, String),
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
                // let module_wat = r#"
                // (module
                //   (type $t0 (func (param i32) (result i32)))
                //   (func $add_one (export "add_one") (type $t0) (param $p0 i32) (result i32)
                //     local.get $p0
                //     i32.const 1
                //     i32.add))
                // "#;

                // let mut store = Store::default();
                // let module = Module::new(&store, &module_wat).map_err(|_| {
                //     ExecutionError::InitializationError("Could not initialize WASM module".into())
                // })?;
                // // The module doesn't import anything, so we create an empty import object.
                // let import_object = imports! {};
                // let instance =
                //     Instance::new(&mut store, &module, &import_object).map_err(|_| {
                //         ExecutionError::InitializationError("Could not initialize WASM".to_string())
                //     })?;

                // let main = instance.exports.get_function("main").map_err(|_| {
                //     ExecutionError::InitializationError("Could not get main function".to_string())
                // })?;
                // main.call(
                //     &mut store,
                //     &[WASMValue::FuncRef(Some(WASMFunction::new_typed(
                //         &mut store,
                //         |index: usize, which: bool| {
                //             if which {
                //                 history[index].0
                //             } else {
                //                 history[index].1
                //             }
                //         },
                //     )))],
                // );
                todo!()
            }
            Executor::Piston(language, program) => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(run_piston(language, program, history, storage)),
        }
    }
}
#[derive(serde::Deserialize)]
struct Output {
    action: bool,
    storage: String,
}
#[derive(serde::Deserialize)]
struct PistonResponse {
    language: String,
    version: String,
    compile: Option<PistonProcess>,
    run: PistonProcess,
}

#[derive(serde::Deserialize)]
struct PistonProcess {
    output: String,
    stdout: String,
}

async fn run_piston(
    language: &String,
    program: &String,
    history: &[(bool, bool)],
    storage: String,
) -> Result<(bool, String), ExecutionError> {
    run_piston_at(
        "https://emkc.org/api/v2/piston/execute",
        language,
        program,
        history,
        storage,
    )
    .await
}

async fn run_piston_at(
    endpoint: &str,
    language: &String,
    program: &String,
    history: &[(bool, bool)],
    storage: String,
) -> Result<(bool, String), ExecutionError> {
    let request = serde_json::json!({
        "language": language,
        "version": "*",
        "files": [{ "name": "", "content": program, "encoding": "utf8" }],
        "stdin": "",
        "compile_timeout": 10000,
        "run_timeout": 3000,
        "compile_memory_limit": -1,
        "run_memory_limit": -1,
        "args": [serde_json::to_string(history).unwrap(), serde_json::to_string(&storage).unwrap()],
    });
    let response = async {
        reqwest::Client::new()
            .post(endpoint)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<PistonResponse>()
            .await
    }
    .await;

    match response {
        Ok(response) => {
            println!("Language: {}", response.language);
            println!("Version: {}", response.version);

            if let Some(c) = response.compile {
                println!("Compilation: {}", c.output);
            }

            println!("Output: {}", response.run.output);
            let output: Output = serde_json::from_str(&response.run.stdout).unwrap();
            return Ok((output.action, output.storage));
        }
        Err(e) => {
            return Err(ExecutionError::RuntimeError(e.to_string()));
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
        // TODO: Multithread this
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

#[cfg(test)]
mod piston_tests {
    use std::io::{Read, Write};

    #[test]
    fn sends_piston_protocol_and_reads_action_and_storage() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/execute", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let body = loop {
                let mut chunk = [0; 4096];
                let count = stream.read(&mut chunk).unwrap();
                assert_ne!(count, 0);
                request.extend_from_slice(&chunk[..count]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    assert!(headers.starts_with("POST /execute HTTP/1.1"));
                    let length: usize = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse().unwrap())
                        })
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break serde_json::from_slice::<serde_json::Value>(
                            &request[end + 4..end + 4 + length],
                        )
                        .unwrap();
                    }
                }
            };
            assert_eq!(body["language"], "python");
            assert_eq!(body["files"][0]["content"], "print('action')");
            assert_eq!(
                body["args"],
                serde_json::json!(["[[true,false]]", "\"previous\""])
            );
            assert_eq!(body["run_timeout"], 3000);
            let response = serde_json::json!({
                "language":"python", "version":"3.12", "compile":null,
                "run":{"output":"done", "stdout":"{\"action\":true,\"storage\":\"next\"}"}
            })
            .to_string();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).unwrap();
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime
            .block_on(super::run_piston_at(
                &endpoint,
                &"python".into(),
                &"print('action')".into(),
                &[(true, false)],
                "previous".into(),
            ))
            .unwrap();
        server.join().unwrap();
        assert_eq!(result, (true, "next".into()));
    }
}
