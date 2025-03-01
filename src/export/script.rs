use crate::value::input::{values_vec_to_ns_array, ScriptInputConversionError};
use crate::value::output::{get_value_from_ns_apple_event_descriptor, ScriptOutputConversionError};
use crate::value::Value;
use objc2::{rc::Retained, runtime::AnyObject, AllocAnyThread};
use objc2_foundation::{NSAppleEventDescriptor, NSDictionary, NSString, NSValue};
use objc2_osa_kit::{
    OSALanguage, OSALanguageInstance, OSAScript, OSAScriptErrorMessageKey, OSAScriptErrorRangeKey,
    OSAStorageOptions,
};
use std::fmt::{Debug, Formatter};
use std::ops::Deref;
use thiserror::Error;

/// Languages supported by `OSAKit`.
pub enum Language {
    AppleScript,
    JavaScript,
}

fn check_main_thread() -> Result<(), ScriptExecutionError> {
    if std::thread::current().name() != Some("main") {
        return Err(ScriptExecutionError::MainThread);
    }
    Ok(())
}

/// Script instance, allowing to compile and execute `AppleScript`/`JavaScript` using `OSAKit`.
/// Uses `OSAScript` class from `OSAKit Framework` directly.
///
/// ## Example
///
/// ```
/// use osakit::{Language, Map, Script, Value, Number};
///
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// #
/// let mut script = Script::new_from_source(
///     Language::AppleScript,
///     "
///     on is_app_running()
///         tell application \"Hopefully Non-Existing Application\" to running
///     end is_app_running
///
///     on concat(x, y)
///         return x & y
///     end concat
///
///     return {id: 21, name: \"root\"}",
/// );
///
/// script.compile()?;
///
/// assert_eq!(
///     script.execute()?,
///     Value::Object(Map::from_iter(vec![
///         ("id".into(), Value::Number(Number::from(21))),
///         ("name".into(), Value::String("root".into()))
///     ]))
/// );
///
/// assert_eq!(
///     script.execute_function("concat", vec![
///         Value::String("Hello, ".into()),
///         Value::String("World!".into())
///     ])?,
///     Value::String("Hello, World!".into())
/// );
///
/// assert_eq!(
///     script.execute_function("is_app_running", vec![])?,
///     Value::Bool(false)
/// );
/// #
/// # Ok(())
/// # }
/// ```
pub struct Script {
    script: Retained<OSAScript>,
    compiled: bool,
}

impl Debug for Script {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Script {{ language: Language::{}, source: {:?}, compiled: {:?} }}",
            unsafe { self.script.language().name() }
                .map(|l| l.to_string())
                .unwrap_or_else(|| "?".to_string()),
            unsafe { self.script.source() }.to_string(),
            self.compiled
        )
    }
}

/// Error happening during compilation. Returned by [`Script::compile`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ScriptCompilationError {
    #[error("unknown compilation error")]
    Unknown,
    #[error("compilation error: {message}")]
    Failure {
        message: String,
        location: usize,
        length: usize,
    },
}

/// Error happening during execution. Returned by [`Script::execute`] and [`Script::execute_function`].
#[derive(Error, Debug, PartialEq, Eq)]
pub enum ScriptExecutionError {
    #[error("unknown execution error")]
    Unknown,
    /// Happens when an error is thrown during script execution.
    #[error("execution error: {message}")]
    Runtime {
        message: String,
        location: usize,
        length: usize,
    },
    /// Happens when trying to convert execution result (`NSAppleEventDescriptor`) to [`Value`].
    #[error("output value conversion error")]
    OutputConversion(#[from] ScriptOutputConversionError),
    /// Happens when trying to convert arguments to the format compatible with `OSAScript`.
    #[error("input value conversion error")]
    InputConversion(#[from] ScriptInputConversionError),
    #[error("osakit can only be used from the main thread")]
    MainThread,
}

fn extract_error_data(
    error_dict_opt: Option<Retained<NSDictionary<NSString, AnyObject>>>,
) -> Option<(String, (usize, usize))> {
    match error_dict_opt {
        None => None,
        Some(error_dict) => match unsafe { error_dict.valueForKey(OSAScriptErrorMessageKey) } {
            None => None,
            Some(message_obj) => {
                let error_message_ns_str: Retained<NSString> =
                    unsafe { Retained::cast_unchecked(message_obj) };
                Some((
                    error_message_ns_str.to_string(),
                    match unsafe { error_dict.valueForKey(OSAScriptErrorRangeKey) }
                        .map(|range| -> Retained<NSValue> {
                            unsafe { Retained::cast_unchecked(range) }
                        })
                        .map(|range| range.get_range())
                    {
                        Some(Some(range)) => (range.location, range.length),
                        _ => (0, 0),
                    },
                ))
            }
        },
    }
}

#[inline]
fn get_osa_language_instance(language: Language) -> Retained<OSALanguageInstance> {
    let language_name = match language {
        Language::AppleScript => "AppleScript",
        Language::JavaScript => "JavaScript",
    };
    let language =
        unsafe { OSALanguage::languageForName(&NSString::from_str(language_name)) }.unwrap();
    unsafe { OSALanguageInstance::languageInstanceWithLanguage(language.deref()) }
}

impl Script {
    /// Constructs Script instance using language and source code.
    pub fn new_from_source(language: Language, source: &str) -> Self {
        let script_ns_string = NSString::from_str(source);
        let script = OSAScript::alloc();
        let ns_language_instance = get_osa_language_instance(language);
        let script = unsafe {
            OSAScript::initWithSource_fromURL_languageInstance_usingStorageOptions(
                script,
                &script_ns_string,
                None,
                Some(ns_language_instance.deref()),
                OSAStorageOptions::Null,
            )
        };
        Self {
            script,
            compiled: false,
        }
    }

    /// Compiles previously specified source code and returns an error in case of compilation failure.
    pub fn compile(&mut self) -> Result<(), ScriptCompilationError> {
        if self.compiled {
            return Ok(());
        }

        let mut error_opt: Option<Retained<NSDictionary<NSString, AnyObject>>> = None;
        if unsafe { self.script.compileAndReturnError(Some(&mut error_opt)) } {
            self.compiled = true;
            return Ok(());
        }

        match extract_error_data(error_opt) {
            None => Err(ScriptCompilationError::Unknown),
            Some((message, (location, length))) => Err(ScriptCompilationError::Failure {
                message,
                location,
                length,
            }),
        }
    }

    /// Executes script and returns the output.
    /// In case of `AppleScript` output can be returned using `return` keyword. I.e. `return "test"`.
    /// In case of `JavaScript` output can be returned using `output` variable. I.e. `output = "test";`.
    pub fn execute(&self) -> Result<Value, ScriptExecutionError> {
        check_main_thread()?;
        let mut error_opt: Option<Retained<NSDictionary<NSString, AnyObject>>> = None;
        let result = unsafe { self.script.executeAndReturnError(Some(&mut error_opt)) };
        Self::process_execution_result(result, error_opt)
    }

    fn process_execution_result(
        result: Option<Retained<NSAppleEventDescriptor>>,
        error_opt: Option<Retained<NSDictionary<NSString, AnyObject>>>,
    ) -> Result<Value, ScriptExecutionError> {
        match error_opt {
            None => match result {
                Some(event_descriptor) => {
                    Ok(get_value_from_ns_apple_event_descriptor(event_descriptor)?)
                }
                None => Ok(Value::Null),
            },
            Some(error) => match extract_error_data(Some(error)) {
                None => Err(ScriptExecutionError::Unknown),
                Some((message, (location, length))) => Err(ScriptExecutionError::Runtime {
                    message,
                    location,
                    length,
                }),
            },
        }
    }

    /// Executes a function in case of `JavaScript` and a subroutine in case of `AppleScript`.
    /// Specified `arguments` are passed to the function and function return value is retuned as [`Value`].
    pub fn execute_function<I: IntoIterator<Item = Value>>(
        &self,
        function_name: &str,
        arguments: I,
    ) -> Result<Value, ScriptExecutionError> {
        check_main_thread()?;
        let mut error_opt: Option<Retained<NSDictionary<NSString, AnyObject>>> = None;
        let ns_handler_name = NSString::from_str(function_name);
        let ns_arguments = values_vec_to_ns_array(arguments)?;
        let result = unsafe {
            self.script.executeHandlerWithName_arguments_error(
                ns_handler_name.deref(),
                ns_arguments.deref(),
                Some(&mut error_opt),
            )
        };
        Self::process_execution_result(result, error_opt)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value::{Map, Number};

    macro_rules! str {
        ($str:literal) => {
            Value::String(String::from($str))
        };
    }

    macro_rules! rec {
        ($($key:ident: $value:expr,)*) => {
            {
                let mut map: Map<String, Value> = Map::new();
                $(map.insert(String::from((stringify!($key))), $value);)*
                Value::Object(map)
            }
        };
    }

    #[test]
    fn it_fails_in_case_of_invalid_syntax_in_apple_script() {
        let mut script = Script::new_from_source(Language::AppleScript, "hello world");
        assert_eq!(
            script.compile().unwrap_err(),
            ScriptCompilationError::Failure {
                message: String::from("A identifier can’t go after this identifier."),
                location: 0,
                length: 11
            }
        );
    }

    #[test]
    fn it_fails_in_case_of_invalid_syntax_in_java_script() {
        let mut script = Script::new_from_source(Language::JavaScript, "hello world");
        assert_eq!(
            script.compile().unwrap_err(),
            ScriptCompilationError::Failure {
                message: String::from(
                    "Error on line 1: SyntaxError: Unexpected identifier 'world'"
                ),
                location: 0,
                length: 11
            }
        );
    }

    #[test]
    fn it_compiles_correct_apple_script() {
        let mut script = Script::new_from_source(Language::AppleScript, "return 1");
        assert_eq!(script.compile(), Ok(()));
    }

    #[test]
    fn it_compiles_correct_java_script() {
        let mut script = Script::new_from_source(Language::JavaScript, "output = 1;");
        assert_eq!(script.compile(), Ok(()));
    }

    #[test]
    fn it_fails_in_case_of_runtime_error_in_apple_script() {
        let mut script = Script::new_from_source(
            Language::AppleScript,
            "tell application \"_NonExistingApplicationName_\" to launch",
        );
        script.compile().unwrap();
        assert_eq!(
            script.execute().unwrap_err(),
            ScriptExecutionError::Runtime {
                message: String::from("File _NonExistingApplicationName_ wasn’t found."),
                location: 51,
                length: 6
            }
        );
    }

    #[test]
    fn it_fails_in_case_of_runtime_error_in_java_script() {
        let mut script = Script::new_from_source(Language::JavaScript, "var x = y;");
        script.compile().unwrap();
        assert_eq!(
            script.execute().unwrap_err(),
            ScriptExecutionError::Runtime {
                message: String::from("Error: ReferenceError: Can't find variable: y"),
                location: 0,
                length: 0
            }
        );
    }

    #[test]
    fn it_returns_null_if_nothing_was_returned_in_apple_script() {
        let mut script = Script::new_from_source(Language::AppleScript, "");
        script.compile().unwrap();
        assert_eq!(script.execute().unwrap(), Value::Null);
    }

    #[test]
    fn it_returns_null_if_nothing_was_returned_in_java_script() {
        let mut script = Script::new_from_source(Language::JavaScript, "");
        script.compile().unwrap();
        assert_eq!(script.execute().unwrap(), Value::Null);
    }

    #[test]
    fn it_returns_calculated_string_value() {
        let mut script = Script::new_from_source(Language::AppleScript, "return \"Hello World\"");
        script.compile().unwrap();
        assert_eq!(script.execute().unwrap(), str!("Hello World"));
    }

    #[test]
    fn it_returns_complex_calculated_value() {
        let mut script = Script::new_from_source(
            Language::JavaScript,
            "output = {\
                string: \"Hello\",\
                small_int: 3,\
                neg_small_int: -3,\
                big_int: 12312312,\
                neg_big_int: -12312312,\
                double: 5.64,\
                bool_true: true,\
                bool_false: false,\
                list: [\"First\", \"Second\", \"épistèmê\"],\
                list_empty: [],\
                null: null,\
                undef: undefined,\
                nested: {\
                    field: 55\
                }\
            };",
        );
        script.compile().unwrap();
        assert_eq!(
            script.execute().unwrap(),
            rec! {
                big_int: Value::Number(Number::from(12312312)),
                bool_false: Value::Bool(false),
                bool_true: Value::Bool(true),
                double: Value::Number(Number::from_f64(5.64).unwrap()),
                list: Value::Array(vec![
                    str!("First"),
                    str!("Second"),
                    str!("épistèmê")
                ]),
                list_empty: Value::Array(vec![]),
                neg_small_int: Value::Number(Number::from(-3)),
                neg_big_int: Value::Number(Number::from(-12312312)),
                nested: rec! {
                    field: Value::Number(Number::from(55)),
                },
                null: Value::Null,
                small_int: Value::Number(Number::from(3)),
                string: str!("Hello"),
                undef: Value::Null,
            }
        );
    }

    #[test]
    fn it_returns_passed_arguments_in_java_script() {
        let mut script = Script::new_from_source(
            Language::JavaScript,
            "function test(x, y) {\
                return [x, y];\
            }",
        );
        script.compile().unwrap();
        assert_eq!(
            script
                .execute_function("test", vec![Value::Bool(true), Value::Null])
                .unwrap(),
            Value::Array(vec![Value::Bool(true), Value::Null])
        );
    }

    #[test]
    fn it_returns_passed_arguments_in_apple_script() {
        let mut script = Script::new_from_source(
            Language::AppleScript,
            "on test_handler(x, y)
                return {x, y}
            end test_handler",
        );
        script.compile().unwrap();
        assert_eq!(
            script
                .execute_function("test_handler", vec![Value::Bool(true), Value::Null])
                .unwrap(),
            Value::Array(vec![Value::Bool(true), Value::Null])
        );
    }

    #[test]
    fn it_supports_debug() {
        let script = Script::new_from_source(Language::AppleScript, "return 123");
        assert_eq!(
            format!("{:?}", script),
            "Script { language: Language::AppleScript, source: \"return 123\", compiled: false }"
        );
    }
}
