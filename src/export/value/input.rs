use crate::Value;
use objc2::{rc::Retained, AllocAnyThread};
use objc2_foundation::{NSArray, NSDictionary, NSNull, NSNumber, NSObject, NSString};
use std::ops::Deref;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ScriptInputConversionError {
    #[error("number conversion error: `{0}`")]
    NumberConversionError(String),
}

fn value_to_nsobject(value: Value) -> Result<Retained<NSObject>, ScriptInputConversionError> {
    Ok(unsafe {
        match value {
            Value::String(s) => Retained::cast_unchecked(NSString::from_str(&s)),
            Value::Bool(b) => {
                Retained::cast_unchecked(NSNumber::initWithBool(NSNumber::alloc(), b))
            }
            Value::Number(n) => Retained::cast_unchecked(if n.is_f64() {
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
            Value::Null => Retained::cast_unchecked(NSNull::null()),
            Value::Array(vec) => Retained::cast_unchecked(values_vec_to_ns_array(vec)?),
            Value::Object(obj) => {
                let mut keys: Vec<Retained<NSString>> = Vec::new();
                let mut values: Vec<Retained<NSObject>> = Vec::new();
                for (key, value) in obj.into_iter() {
                    keys.push(NSString::from_str(&key));
                    values.push(value_to_nsobject(value)?)
                }
                let key_refs: Vec<&NSString> = keys.iter().map(|k| k.deref()).collect();
                Retained::cast_unchecked(NSDictionary::from_retained_objects(&key_refs, &values))
            }
        }
    })
}

pub(crate) fn values_vec_to_ns_array<I: IntoIterator<Item = Value>>(
    values: I,
) -> Result<Retained<NSArray>, ScriptInputConversionError> {
    let mut vec: Vec<Retained<NSObject>> = Vec::new();

    for item in values {
        vec.push(value_to_nsobject(item)?);
    }

    Ok(unsafe { Retained::cast_unchecked::<NSArray>(NSArray::from_retained_slice(&vec)) })
}
