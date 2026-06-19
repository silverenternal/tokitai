// T-016 negative case: the example's result type does not match the
// real method's signature. The method returns `i32` but the example
// writes `"not an i32"` (a `&str`). The host compiler must report a
// type error anchored at the `call!(...)` literal.

use tokitai::tool;

#[derive(Default, Debug)]
pub struct Negative;

#[tool]
impl Negative {
    #[tool(example = call!(self.bad_result(1) => "not an i32"))]
    pub fn bad_result(&self, n: i32) -> i32 {
        n
    }
}

fn main() {}