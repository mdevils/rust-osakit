use crate::Value;
use icrate::objc2::rc::Id;
use icrate::objc2::ClassType;
use icrate::Foundation::{NSArray, NSDictionary, NSNull, NSNumber, NSObject, NSString};
use std::ops::Deref;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ScriptInputConversionError {
    #[error("number conversion error: `{0}`")]
    NumberConversionError(String),
}

fn value_to_nsobject(value: &Value) -> Result<Id<NSObject>, ScriptInputConversionError> {
    Ok(unsafe {
        match value {
            Value::String(s) => Id::cast(NSString::from_str(s)),
            Value::Bool(b) => Id::cast(NSNumber::initWithBool(NSNumber::alloc(), *b)),
            Value::Number(n) => Id::cast(if n.is_f64() {
                n.as_f64()
                    .map(|f| NSNumber::initWithDouble(NSNumber::alloc(), f))
                    .ok_or_else(|| {
                        ScriptInputConversionError::NumberConversionError(n.to_string())
                    })?
            } else if n.is_i64() {
                n.as_i64()
                    .map(|l| NSNumber::initWithLongLong(NSNumber::alloc(), l))
                    .ok_or_else(|| {
                        ScriptInputConversionError::NumberConversionError(n.to_string())
                    })?
            } else {
                n.as_u64()
                    .map(|l| NSNumber::initWithUnsignedLongLong(NSNumber::alloc(), l))
                    .ok_or_else(|| {
                        ScriptInputConversionError::NumberConversionError(n.to_string())
                    })?
            }),
            Value::Null => Id::cast(NSNull::null()),
            Value::Array(vec) => Id::cast(values_vec_to_ns_array(vec)?),
            Value::Object(obj) => {
                let mut keys: Vec<Id<NSString>> = Vec::new();
                let mut values: Vec<Id<NSObject>> = Vec::new();
                for (key, value) in obj.iter() {
                    keys.push(NSString::from_str(key));
                    values.push(value_to_nsobject(value)?)
                }
                let key_refs: Vec<&NSString> = keys.iter().map(|k| k.deref()).collect();
                Id::cast(NSDictionary::from_vec(&key_refs, values))
            }
        }
    })
}

pub(crate) fn values_vec_to_ns_array(
    values: &[Value],
) -> Result<Id<NSArray>, ScriptInputConversionError> {
    let mut vec: Vec<Id<NSObject>> = Vec::new();

    for item in values {
        vec.push(value_to_nsobject(item)?);
    }

    Ok(unsafe { Id::cast::<NSArray>(NSArray::from_vec(vec)) })
}
