use super::script::{Script, ScriptExecutionError};
use super::value::Value;
use serde::de::DeserializeOwned;
use serde_json::from_value;
use thiserror::Error;

/// Error returned when calling a method of a script constructed by [`crate::declare_script!`]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScriptFunctionRunError {
    #[error("function execution failed: {0}")]
    Execution(ScriptExecutionError),
    #[error("could not serialize argument `{arg_name}`: {message}")]
    ArgumentSerialization { arg_name: String, message: String },
    #[error("could not deserialize function execution result: {message}")]
    ResultDeserialization { message: String },
}

#[doc(hidden)]
pub fn __arg_s_error<T>(
    arg_name: &str,
    error: ::serde_json::Error,
) -> Result<T, ScriptFunctionRunError> {
    Err(ScriptFunctionRunError::ArgumentSerialization {
        arg_name: String::from(arg_name),
        message: error.to_string(),
    })
}

#[doc(hidden)]
pub fn __exec_and_deserialize<T: DeserializeOwned, I: IntoIterator<Item = Value>>(
    script: &Script,
    fn_name: &str,
    arguments: I,
) -> Result<T, ScriptFunctionRunError> {
    match script.execute_function(fn_name, arguments) {
        Ok(output) => {
            let deserialized_value: Result<T, serde_json::Error> = from_value(output);
            match deserialized_value {
                Ok(result) => Ok(result),
                Err(err) => Err(ScriptFunctionRunError::ResultDeserialization {
                    message: err.to_string(),
                }),
            }
        }
        Err(err) => Err(ScriptFunctionRunError::Execution(err)),
    }
}

/// Macro to help construct scripts in a form of API.
///
/// ## Example:
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use osakit::declare_script;
///
/// declare_script! {
///     #[language(JavaScript)]
///     #[source("
///         function concat(x, y) {
///             return x + y;
///         }
///
///         function multiply(a, b) {
///             return a * b;
///         }
///
///         function current_user() {
///             return {
///                 id: 21,
///                 name: \"root\"
///             };
///         }
///     ")]
///     pub MyJsScript {
///         pub fn concat(x: &str, y: &str) -> String;
///         pub fn multiply(a: i32, b: i32) -> i32;
///         pub fn current_user() -> User;
///     }
/// }
///
/// #[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
/// struct User {
///     id: u16,
///     name: String,
/// }
///
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// #
/// let script = MyJsScript::new()?;
/// assert_eq!(
///     script.multiply(3, 2)?,
///     6
/// );
/// assert_eq!(
///     script.concat("Hello, ", "World")?,
///     "Hello, World"
/// );
/// assert_eq!(
///     script.current_user()?,
///     User {
///         id: 21,
///         name: "root".into()
///     }
/// );
/// #
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "declare-script")]
#[macro_export]
macro_rules! declare_script {
    (
        #[language($language:ident)]
        #[source($source:literal)]
        $(#[$struct_meta:meta])*
        $vis:vis $struct_name:ident {
            $(
                $(#[$fn_meta:meta])*
                $fn_vis:vis fn $fn_name:ident(
                    $($fn_arg_name:ident : $fn_arg_type:ty),*
                )$( -> $fn_res_type:ty)?;
            )*
        }
    ) => {
        $(#[$struct_meta])*
        $vis struct $struct_name {
            script: $crate::Script
        }

        impl $struct_name {
            $vis fn new() -> ::core::result::Result<$struct_name, $crate::ScriptCompilationError> {
                let mut script = $crate::Script::new_from_source(
                    $crate::Language::$language,
                    $source
                );
                script.compile()?;
                Ok($struct_name { script })
            }

            $(
                $crate::__script_fn!(
                    $(#[$fn_meta])*
                    $fn_vis fn $fn_name($($fn_arg_name : $fn_arg_type),*)$( -> $fn_res_type)?;
                );
            )*
        }
    };
}

#[cfg(feature = "declare-script")]
#[macro_export]
#[doc(hidden)]
macro_rules! __script_fn {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($($arg_name:ident : $arg_type:ty),*) -> $res_type:ty;
    ) => {
        $crate::__script_fn_impl!(
            meta = ($($meta)*)
            vis = ($vis)
            name = ($name)
            args = ($($arg_name : $arg_type),*)
            res = ($res_type)
        );
    };
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($($arg_name:ident : $arg_type:ty),*);
    ) => {
        $crate::__script_fn_impl!(
            meta = ($($meta)*)
            vis = ($vis)
            name = ($name)
            args = ($($arg_name : $arg_type),*)
            res = (())
        );
    };
}

#[cfg(feature = "declare-script")]
#[macro_export]
#[doc(hidden)]
macro_rules! __script_fn_impl {
    (
        meta = ($($meta:meta)*)
        vis = ($vis:vis)
        name = ($name:ident)
        args = ($($arg_name:ident : $arg_type:ty),*)
        res = ($res_type:ty)
    ) => {
        $(#[$meta])*
        $vis fn $name(&self $(, $arg_name : $arg_type)*) -> ::core::result::Result<$res_type, $crate::ScriptFunctionRunError> {
            let arguments: Vec<$crate::Value> = vec![$(
                $crate::to_value($arg_name).or_else(|e| $crate::macros::__arg_s_error(stringify!($arg_name), e))?,
            )*];
            $crate::macros::__exec_and_deserialize(
                &self.script,
                stringify!($name),
                arguments
            )
        }
    };
}

#[cfg(test)]
mod test {
    use super::super::script::ScriptExecutionError;
    use super::ScriptFunctionRunError;

    declare_script! {
        #[language(JavaScript)]
        #[source("
            function concat(x, y) {
                return x + y;
            }

            function no_args_no_result() {}

            function throws_an_error(message) {
                throw new Error(message);
            }
        ")]
        pub(crate) MacroTestScript {
            pub(crate) fn concat(x: &str, y: &str) -> String;
            pub(crate) fn no_args_no_result();
            pub(crate) fn throws_an_error(message: &str);
        }
    }

    #[test]
    fn it_runs_concat_function() {
        let script = MacroTestScript::new().unwrap();
        assert_eq!(script.concat("Hello, ", "World").unwrap(), "Hello, World");
    }

    #[test]
    fn it_runs_no_args_no_result() {
        let script = MacroTestScript::new().unwrap();
        assert_eq!(script.no_args_no_result().unwrap(), ());
    }

    #[test]
    fn it_throws_an_error() {
        let script = MacroTestScript::new().unwrap();
        assert_eq!(
            script.throws_an_error("Test Error").unwrap_err(),
            ScriptFunctionRunError::Execution(ScriptExecutionError::Runtime {
                message: "Error: Error: Test Error".into(),
                location: 0,
                length: 0
            })
        );
    }
}
