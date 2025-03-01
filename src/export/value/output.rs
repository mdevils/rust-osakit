use super::{Map, Value};
use objc2::{msg_send, rc::Retained};
use objc2_foundation::{NSAppleEventDescriptor, NSInteger};
use serde_json::Number;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ScriptOutputConversionError {
    #[error("string expected, but none found")]
    StringExpectedButNoneFound,
    #[error("date expected, but none found")]
    DateExpectedButNoneFound,
    #[error("string expected, but {0} found")]
    StringExpectedButValueFound(String),
    #[error("unexpected typed value: `{0}`")]
    UnpexpectedTypedValue(String),
    #[error("unkndown descriptor type: `{0}`")]
    UnknownDescriptorType(String),
    #[error("descriptor not found at index: `{0}`")]
    DescriptorNotFoundAtIndex(isize),
    #[error("infinite float cannot be converted: `{0}`")]
    InfiniteFloat(String),
    #[error("url expected, but none found")]
    UrlExpectedButNoneFound,
}

type FourCharCode = u32;

#[inline]
fn get_descriptor_type(descriptor: &Retained<NSAppleEventDescriptor>) -> FourCharCode {
    unsafe { msg_send![descriptor, descriptorType] }
}

#[inline]
fn get_descriptor_for_keyword(
    descriptor: &Retained<NSAppleEventDescriptor>,
    keyword: FourCharCode,
) -> Option<Retained<NSAppleEventDescriptor>> {
    unsafe { msg_send![descriptor, descriptorForKeyword: keyword] }
}

#[inline]
fn add_special_key_to_map_if_defined(
    map: &mut Map<String, Value>,
    descriptor: &Retained<NSAppleEventDescriptor>,
    keyword: FourCharCode,
    key: &str,
) -> Result<(), ScriptOutputConversionError> {
    if let Some(val_descriptor) = get_descriptor_for_keyword(descriptor, keyword) {
        map.insert(
            key.into(),
            get_value_from_ns_apple_event_descriptor(val_descriptor)?,
        );
    }
    Ok(())
}

macro_rules! four_char_codes {
    ($($cost_name:ident: $four_char_code:literal),*$(,)?) => {
        const fn four_char_code_from_string(source: &str) -> FourCharCode {
            let bytes = source.as_bytes();
            if bytes.len() != 4 {
                panic!("Invalid four char code length.");
            }
            return u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        $(const $cost_name: FourCharCode = four_char_code_from_string($four_char_code);)*
    };
}

four_char_codes! {
    DESC_TYPE_STRING: "utxt",
    DESC_TYPE_TRUE: "true",
    DESC_TYPE_FALSE: "fals",
    DESC_TYPE_RECORD: "reco",
    DESC_TYPE_LIST: "list",
    DESC_TYPE_LONG: "long",
    DESC_TYPE_DOUBLE: "doub",
    DESC_TYPE_TYPE: "type",
    DESC_TYPE_NULL: "null",
    DESC_TYPE_ENUM: "enum",
    DESC_TYPE_LDATE: "ldt ",
    DESC_TYPE_URL: "url ",
    OSTYPE_MISSING: "msng",
    OSTYPE_NULL: "null",
    OSTYPE_YES: "yes ",
    OSTYPE_NO: "no  ",
    AS_USER_RECORD_FIELDS: "usrf",
    AS_ID: "ID  ",
    AS_NAME: "pnam",
}

#[cold]
fn four_char_code_to_string(t: FourCharCode) -> String {
    t.to_be_bytes()
        .into_iter()
        .map(|b| if b.is_ascii() { char::from(b) } else { '?' })
        .collect::<String>()
}

pub(crate) fn get_value_from_ns_apple_event_descriptor(
    descriptor: Retained<NSAppleEventDescriptor>,
) -> Result<Value, ScriptOutputConversionError> {
    Ok(match get_descriptor_type(&descriptor) {
        DESC_TYPE_STRING => Value::String(
            unsafe { descriptor.stringValue() }
                .ok_or(ScriptOutputConversionError::StringExpectedButNoneFound)?
                .to_string(),
        ),
        DESC_TYPE_LONG => Value::Number(Number::from(unsafe { descriptor.int32Value() })),
        DESC_TYPE_LDATE => Value::Number(Number::from(unsafe {
            descriptor
                .dateValue()
                .ok_or(ScriptOutputConversionError::DateExpectedButNoneFound)?
                .timeIntervalSince1970()
                .trunc() as i64
        })),
        DESC_TYPE_DOUBLE => {
            let value = unsafe { descriptor.doubleValue() };
            Value::Number(
                Number::from_f64(value)
                    .ok_or_else(|| ScriptOutputConversionError::InfiniteFloat(value.to_string()))?,
            )
        }
        DESC_TYPE_TRUE => Value::Bool(true),
        DESC_TYPE_FALSE => Value::Bool(false),
        DESC_TYPE_ENUM => match unsafe { descriptor.typeCodeValue() } {
            OSTYPE_YES => Value::Bool(true),
            OSTYPE_NO => Value::Bool(false),
            type_code_value => {
                return Err(ScriptOutputConversionError::UnpexpectedTypedValue(
                    four_char_code_to_string(type_code_value),
                ))
            }
        },
        DESC_TYPE_TYPE => match unsafe { descriptor.typeCodeValue() } {
            OSTYPE_MISSING => Value::Null,
            OSTYPE_NULL => Value::Null,
            type_code_value => {
                return Err(ScriptOutputConversionError::UnpexpectedTypedValue(
                    four_char_code_to_string(type_code_value),
                ))
            }
        },
        DESC_TYPE_URL => match unsafe { descriptor.stringValue() } {
            Some(url) => Value::String(url.to_string()),
            None => return Err(ScriptOutputConversionError::UrlExpectedButNoneFound),
        },
        DESC_TYPE_NULL => Value::Null,
        DESC_TYPE_RECORD => {
            let mut result: Map<String, Value> = Map::new();
            add_special_key_to_map_if_defined(&mut result, &descriptor, AS_ID, "id")?;
            add_special_key_to_map_if_defined(&mut result, &descriptor, AS_NAME, "name")?;
            match get_descriptor_for_keyword(&descriptor, AS_USER_RECORD_FIELDS) {
                Some(descriptor) => {
                    for i in (1..unsafe { descriptor.numberOfItems() } + 1).step_by(2) {
                        let key = match get_nested_ns_apple_event_descriptor_value(&descriptor, i)?
                        {
                            Value::String(s) => s,
                            unexpected_value => {
                                return Err(
                                    ScriptOutputConversionError::StringExpectedButValueFound(
                                        unexpected_value.to_string(),
                                    ),
                                )
                            }
                        };
                        result.insert(
                            key,
                            get_nested_ns_apple_event_descriptor_value(&descriptor, i + 1)?,
                        );
                    }
                    Value::Object(result)
                }
                None => Value::Object(result),
            }
        }
        DESC_TYPE_LIST => {
            let mut result: Vec<Value> = Vec::new();
            for i in 1..unsafe { descriptor.numberOfItems() } + 1 {
                result.push(get_nested_ns_apple_event_descriptor_value(&descriptor, i)?);
            }
            Value::Array(result)
        }
        unknown => {
            return Err(ScriptOutputConversionError::UnknownDescriptorType(
                four_char_code_to_string(unknown),
            ))
        }
    })
}

#[inline]
fn get_nested_ns_apple_event_descriptor_value(
    descriptor: &Retained<NSAppleEventDescriptor>,
    index: NSInteger,
) -> Result<Value, ScriptOutputConversionError> {
    get_value_from_ns_apple_event_descriptor(unsafe { descriptor.descriptorAtIndex(index) }.ok_or(
        ScriptOutputConversionError::DescriptorNotFoundAtIndex(index),
    )?)
}

#[cfg(test)]
mod test {
    use super::super::super::script::{Language, Script};
    use super::super::super::value::output::ScriptOutputConversionError;
    use super::*;
    use objc2::AllocAnyThread;
    use objc2_foundation::NSAppleEventDescriptor;

    #[test]
    fn it_returns_null_for_empty_descriptor() {
        let descriptor = NSAppleEventDescriptor::alloc();
        let descriptor = unsafe { NSAppleEventDescriptor::init(descriptor) };
        assert_eq!(
            get_value_from_ns_apple_event_descriptor(descriptor).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn it_fails_when_called_with_incorrect_index() {
        let descriptor = NSAppleEventDescriptor::alloc();
        let descriptor = unsafe { NSAppleEventDescriptor::initListDescriptor(descriptor) };
        assert_eq!(
            get_nested_ns_apple_event_descriptor_value(&descriptor, 1),
            Err(ScriptOutputConversionError::DescriptorNotFoundAtIndex(1))
        );
    }

    mod java_script {
        use super::*;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn value_from_java_script(json: &str) -> Value {
            let mut script =
                Script::new_from_source(Language::JavaScript, &format!("output = ({});", json));
            script.compile().unwrap();
            script.execute().unwrap()
        }

        #[test]
        fn it_returns_string() {
            assert_eq!(
                value_from_java_script("\"Hello World\""),
                Value::String(String::from("Hello World"))
            );
        }

        #[test]
        fn it_returns_true() {
            assert_eq!(value_from_java_script("true"), Value::Bool(true));
        }

        #[test]
        fn it_returns_false() {
            assert_eq!(value_from_java_script("false"), Value::Bool(false));
        }

        #[test]
        fn it_returns_positive_long() {
            assert_eq!(
                value_from_java_script("2000000"),
                Value::Number(Number::from(2000000))
            );
        }

        #[test]
        fn it_returns_negative_long() {
            assert_eq!(
                value_from_java_script("-2000000"),
                Value::Number(Number::from(-2000000))
            );
        }

        #[test]
        fn it_returns_positive_double() {
            assert_eq!(
                value_from_java_script("1234.5678"),
                Value::Number(Number::from_f64(1234.5678).unwrap())
            );
        }

        #[test]
        fn it_returns_negative_double() {
            assert_eq!(
                value_from_java_script("-1234.5678"),
                Value::Number(Number::from_f64(-1234.5678).unwrap())
            );
        }

        #[test]
        fn it_returns_null() {
            assert_eq!(value_from_java_script("null"), Value::Null);
        }

        #[test]
        fn it_returns_null_in_case_of_undefined() {
            assert_eq!(value_from_java_script("undefined"), Value::Null);
        }

        #[test]
        fn it_returns_empty_array() {
            assert_eq!(value_from_java_script("[]"), Value::Array(vec![]));
        }

        #[test]
        fn it_returns_array_with_items() {
            assert_eq!(
                value_from_java_script("[1, true, \"Hello\"]"),
                Value::Array(vec![
                    Value::Number(Number::from(1)),
                    Value::Bool(true),
                    Value::String("Hello".into()),
                ])
            );
        }

        #[test]
        fn it_returns_empty_object() {
            assert_eq!(value_from_java_script("{}"), Value::Object(Map::new()));
        }

        #[test]
        fn it_returns_empty_object_for_functions() {
            assert_eq!(
                value_from_java_script("console.log"),
                Value::Object(Map::new())
            );
        }

        #[test]
        fn it_returns_date_as_long() {
            let epsilon = 100;
            assert!(match value_from_java_script("new Date()") {
                Value::Number(num) =>
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        - num.as_u64().unwrap()
                        < epsilon,
                _ => false,
            });
        }

        #[test]
        fn it_returns_object_with_items() {
            assert_eq!(
                value_from_java_script("{x: 1, b: true, s: \"Hello\"}"),
                Value::Object(Map::from_iter(vec![
                    ("x".into(), Value::Number(Number::from(1))),
                    ("b".into(), Value::Bool(true)),
                    ("s".into(), Value::String("Hello".into())),
                ]))
            );
        }
    }

    mod apple_script {
        use super::super::super::super::script::ScriptExecutionError;
        use super::*;
        use std::time::{SystemTime, UNIX_EPOCH};

        fn value_from_apple_script(value: &str) -> Value {
            let mut script =
                Script::new_from_source(Language::AppleScript, &format!("return {}", value));
            script.compile().unwrap();
            script.execute().unwrap()
        }

        fn error_from_apple_script(value: &str) -> ScriptExecutionError {
            let mut script =
                Script::new_from_source(Language::AppleScript, &format!("return {}", value));
            script.compile().unwrap();
            script.execute().unwrap_err()
        }

        #[test]
        fn it_returns_string() {
            assert_eq!(
                value_from_apple_script("\"Hello World\""),
                Value::String(String::from("Hello World"))
            );
        }

        #[test]
        fn it_returns_true() {
            assert_eq!(value_from_apple_script("true"), Value::Bool(true));
        }

        #[test]
        fn it_returns_false() {
            assert_eq!(value_from_apple_script("false"), Value::Bool(false));
        }

        #[test]
        fn it_returns_true_for_yes() {
            assert_eq!(value_from_apple_script("yes"), Value::Bool(true));
        }

        #[test]
        fn it_returns_false_for_no() {
            assert_eq!(value_from_apple_script("no"), Value::Bool(false));
        }

        #[test]
        fn it_returns_positive_long() {
            assert_eq!(
                value_from_apple_script("2000000"),
                Value::Number(Number::from(2000000))
            );
        }

        #[test]
        fn it_returns_positive_longlong_as_float() {
            assert_eq!(
                value_from_apple_script("9000000000"),
                Value::Number(Number::from_f64(9000000000.0).unwrap())
            );
        }

        #[test]
        fn it_returns_negative_long() {
            assert_eq!(
                value_from_apple_script("-2000000"),
                Value::Number(Number::from(-2000000))
            );
        }

        #[test]
        fn it_returns_date_as_long() {
            let epsilon = 100;
            assert!(match value_from_apple_script("(current date)") {
                Value::Number(num) =>
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                        - num.as_u64().unwrap()
                        < epsilon,
                _ => false,
            });
        }

        #[test]
        fn it_returns_positive_double() {
            assert_eq!(
                value_from_apple_script("1234.5678"),
                Value::Number(Number::from_f64(1234.5678).unwrap())
            );
        }

        #[test]
        fn it_returns_negative_double() {
            assert_eq!(
                value_from_apple_script("-1234.5678"),
                Value::Number(Number::from_f64(-1234.5678).unwrap())
            );
        }

        #[test]
        fn it_returns_url_as_string() {
            assert_eq!(
                value_from_apple_script("\"http://example.com\" as URL"),
                Value::String("http://example.com".into())
            );
        }

        #[test]
        fn it_returns_null() {
            assert_eq!(value_from_apple_script("null"), Value::Null);
        }

        #[test]
        fn it_returns_null_in_case_of_missing_value() {
            assert_eq!(value_from_apple_script("missing value"), Value::Null);
        }

        #[test]
        fn it_returns_empty_array() {
            assert_eq!(value_from_apple_script("{}"), Value::Array(vec![]));
        }

        #[test]
        fn it_returns_array_with_items() {
            assert_eq!(
                value_from_apple_script("{1, true, \"Hello\"}"),
                Value::Array(vec![
                    Value::Number(Number::from(1)),
                    Value::Bool(true),
                    Value::String("Hello".into()),
                ])
            );
        }

        #[test]
        fn it_returns_object_with_items() {
            assert_eq!(
                value_from_apple_script("{x: 1, b: true, s: \"Hello\"}"),
                Value::Object(Map::from_iter(vec![
                    ("x".into(), Value::Number(Number::from(1))),
                    ("b".into(), Value::Bool(true)),
                    ("s".into(), Value::String("Hello".into())),
                ]))
            );
        }

        #[test]
        fn it_returns_object_with_special_fields() {
            assert_eq!(
                value_from_apple_script("{x: 1, id: 123, name: \"test\"}"),
                Value::Object(Map::from_iter(vec![
                    ("x".into(), Value::Number(Number::from(1))),
                    ("id".into(), Value::Number(Number::from(123))),
                    ("name".into(), Value::String("test".into())),
                ]))
            );
        }

        #[test]
        fn it_fails_in_case_of_enumerations() {
            assert_eq!(
                error_from_apple_script("return key"),
                ScriptExecutionError::OutputConversion(
                    ScriptOutputConversionError::UnpexpectedTypedValue("ks$\0".into())
                )
            );
        }

        #[test]
        fn it_fails_in_case_of_classes() {
            assert_eq!(
                error_from_apple_script("November"),
                ScriptExecutionError::OutputConversion(
                    ScriptOutputConversionError::UnpexpectedTypedValue("nov ".into())
                )
            );
        }
    }
}
