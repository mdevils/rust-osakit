mod export;

#[allow(unused_imports)]
pub use export::*;

#[macro_use]
extern crate libtest_mimic_collect;

pub fn main() {
    libtest_mimic_collect::TestCollection::run();
}
