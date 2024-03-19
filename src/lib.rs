//! `osakit` aims to provide direct access to `OSAKit Framework` of macOS. Is uses ObjC-bindings
//! to access OSAKit and run both `AppleScript` and `JavaScript`.
//!
//! `osakit` is built using [`serde`](https://crates.io/crates/serde) for input-output
//! serialization/deserialization.
//! Allows passing data to `JavaScript`/`AppleScript` functions and returns back the results.
//! Input and output data are represented using `Value` from [`serde_json`].
//!
//! Comes with [`declare_script!`] macro (unstable) to simplify
//! working with `OSAKit Framework`.
//!
//! [Source code on GitHub](https://github.com/mdevils/rust-osakit)
//!
//! ## Installation
//!
//! Add `osakit` to the dependencies. Specify `"full"` feature if you want to use `declare_script`
//! macro or `"stable"` feature to only include stable API.
//!
//! ```toml
//! [dependencies]
//! osakit = { version = "0.1.0", features = ["full"] }
//! ```
//!
//! ## Example using `declare_script`
//!
//! ```
//! use serde::{Deserialize, Serialize};
//! use osakit::declare_script;
//!
//! declare_script! {
//!     #[language(JavaScript)]
//!     #[source("
//!         function concat(x, y) {
//!             return x + y;
//!         }
//!
//!         function multiply(a, b) {
//!             return a * b;
//!         }
//!
//!         function current_user() {
//!             return {
//!                 id: 21,
//!                 name: \"root\"
//!             };
//!         }
//!     ")]
//!     MyJsScript {
//!         fn concat(x: &str, y: &str) -> String;
//!         fn multiply(a: i32, b: i32) -> i32;
//!         fn current_user() -> User;
//!     }
//! }
//!
//! #[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
//! struct User {
//!     id: u16,
//!     name: String,
//! }
//!
//! let script = MyJsScript::new().unwrap();
//! assert_eq!(
//!     script.multiply(3, 2).unwrap(),
//!     6
//! );
//! assert_eq!(
//!     script.concat("Hello, ", "World").unwrap(),
//!     "Hello, World"
//! );
//! assert_eq!(
//!     script.current_user().unwrap(),
//!     User {
//!         id: 21,
//!         name: "root".into()
//!     }
//! );
//! ```
//!
//! ## Example using `Script`
//!
//! ```
//! use osakit::{Language, Map, Script, Value, Number};
//!
//! let mut script = Script::new_from_source(
//!     Language::AppleScript, "
//!     on is_app_running()
//!         tell application \"Hopefully Non-Existing Application\" to running
//!     end is_app_running
//!
//!     on concat(x, y)
//!         return x & y
//!     end concat
//!
//!     return {id: 21, name: \"root\"}
//! ");
//!
//! script.compile().unwrap();
//!
//! assert_eq!(
//!     script.execute().unwrap(),
//!     Value::Object(Map::from_iter(vec![
//!         ("id".into(), Value::Number(Number::from(21))),
//!         ("name".into(), Value::String("root".into()))
//!     ]))
//! );
//!
//! assert_eq!(
//!     script.execute_function("concat", &vec![
//!         Value::String("Hello, ".into()),
//!         Value::String("World!".into())
//!     ]).unwrap(),
//!     Value::String("Hello, World!".into())
//! );
//!
//! assert_eq!(
//!     script.execute_function("is_app_running", &vec![]).unwrap(),
//!     Value::Bool(false)
//! );
//! ```
//!
//! ## Supported platforms
//!
//! Due to the fact that OSAKit is Mac-specific, only `macOS` is supported.

mod script;
mod value;

pub use script::{Language, Script, ScriptCompilationError, ScriptExecutionError};
pub use serde_json::Error as JsonError;
pub use value::{from_value, to_value, Map, Number, Value};

#[cfg(feature = "declare-script")]
pub use macros::ScriptFunctionRunError;
/// [`declare_script!`] macro related types.
#[cfg(feature = "declare-script")]
pub mod macros;
