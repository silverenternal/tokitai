// T-017: trybuild fixture for a compose chain with a type
// mismatch. The macro must refuse to compile this with a
// diagnostic anchored at the offending step (`step_b`).
//
// `step_a` returns `String`, but `step_b` expects `i32` as
// its first argument. The chain does not connect; the macro
// reports the mismatch.

use tokitai::{compose, tool};

pub struct BrokenChain;

#[compose(name = "broken", steps = [step_a, step_b])]
#[tool]
impl BrokenChain {
    pub fn step_a(&self, x: i32) -> String {
        format!("{}", x)
    }

    pub fn step_b(&self, count: i32) -> i32 {
        count
    }
}

fn main() {}
