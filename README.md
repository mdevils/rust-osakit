# osakit

`osakit` aims to provide direct access to `OSAKit Framework` of macOS. Is uses ObjC-bindings
to access OSAKit and run both `AppleScript` and `JavaScript`.

`osakit` is built using [`serde`](https://crates.io/crates/serde) for input-output
serialization/deserialization.
Allows passing data to `JavaScript`/`AppleScript` functions and returns back the results.
Input and output data are represented using `Value` from
[`serde_json`](https://crates.io/crates/serde_json).

Comes with `declare_script!` macro (unstable) to simplify working with `OSAKit Framework`.

[Source code on GitHub](https://github.com/mdevils/rust-osakit)

## Installation

Add `osakit` to the dependencies. Specify `"full"` feature if you want to use `declare_script`
macro or `"stable"` feature to only include stable API.

```toml
[dependencies]
osakit = { version = "0.2", features = ["full"] }
```

## Example using `declare_script`

```rust
use serde::{Deserialize, Serialize};
use osakit::declare_script;
use std::error::Error;

declare_script! {
    #[language(JavaScript)]
    #[source("
        function concat(x, y) {
            return x + y;
        }
                                                                                                       
        function multiply(a, b) {
            return a * b;
        }
                                                                                                       
        function current_user() {
            return {
                id: 21,
                name: \"root\"
            };
        }
    ")]
    pub MyJsScript {
        pub fn concat(x: &str, y: &str) -> String;
        pub fn multiply(a: i32, b: i32) -> i32;
        pub fn current_user() -> User;
    }
}
                                                                                                       
#[derive(Deserialize, Serialize, Debug, PartialEq, Eq)]
struct User {
    id: u16,
    name: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let script = MyJsScript::new()?;
    assert_eq!(
        script.multiply(3, 2)?,
        6
    );
    assert_eq!(
        script.concat("Hello, ", "World")?,
        "Hello, World"
    );
    assert_eq!(
        script.current_user()?,
        User {
            id: 21,
            name: "root".into()
        }
    );
    Ok(())
}
```

## Example using `Script`

```rust
use osakit::{Language, Map, Script, Value, Number};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut script = Script::new_from_source(Language::AppleScript, "
        on is_app_running()
            tell application \"Hopefully Non-Existing Application\" to running
        end is_app_running
        on concat(x, y)
            return x & y
        end concat
        return {id: 21, name: \"root\"}
    ");
    script.compile()?;
    assert_eq!(
        script.execute()?,
        Value::Object(Map::from_iter(vec![
            ("id".into(), Value::Number(Number::from(21))),
            ("name".into(), Value::String("root".into()))
        ]))
    );
    assert_eq!(
        script.execute_function("concat", vec![
            Value::String("Hello, ".into()),
            Value::String("World!".into())
        ])?,
        Value::String("Hello, World!".into())
    );
    assert_eq!(
        script.execute_function("is_app_running", vec![])?,
        Value::Bool(false)
    );

    Ok(())
}
```

## Usage

See [Full Documentation](https://docs.rs/osakit/).

## Limitations

* Due to limitations on `OSAKit Framework`-side integer values returned from `JavaScript` code
  are limited to `i32` type.
* `OSAKit` calls must be made from the main thread, so, for example, the default `cargo test`s can fail,
  after stalling for 2 min, use a custom test harness like [libtest-mimic](https://github.com/LukasKalbertodt/libtest-mimic) with `--test-threads=1`.
  For convenience, there is a [libtest-mimic-collect](https://crates.io/crates/libtest-mimic-collect)
  crate that provides a procedural macro for collecting tests for `libtest-mimic` crate.

## Supported platforms

Due to the fact that OSAKit is Mac-specific, only `macOS` is supported.

## License

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
  ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
