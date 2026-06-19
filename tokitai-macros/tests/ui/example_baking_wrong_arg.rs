// T-016 negative case: the example's input type does not match
// the real method's signature. `self.bad_example("not an int")` is
// a `&str` but `bad_example` takes an `i32`. The host compiler must
// report a type error anchored at the `call!(...)` literal.

use tokitai::tool;

#[derive(Default, Debug)]
pub struct Negative;

#[tool]
impl Negative {
    #[tool(example = call!(self.bad_example("not an int") => 42))]
    pub fn bad_example(&self, n: i32) -> i32 {
        n
    }
}

fn main() {}