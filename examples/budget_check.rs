//! T-014: per-impl-block token-cost budget example.
//!
//! This example defines a deliberately large `#[tool]` impl block
//! (200 tool methods) so a `TOKITAI_PROFILE_BUDGET=<N>` build
//! will emit a compile-time warning. The point of the example is
//! to make the warning visible from a single `cargo run` so CI
//! and humans can both confirm the budget path is wired up.
//!
//! # How to run
//!
//! ```bash
//! # Default build: no warning, no profiling.
//! cargo run --example budget_check
//!
//! # Profile mode: per-impl timing AND per-impl schema byte
//! # count, both via cargo:warning= lines.
//! TOKITAI_PROFILE=1 cargo run --example budget_check
//!
//! # Budget mode: any impl whose combined schema exceeds 8192
//! # bytes (about 2 000 tokens of LLM system-prompt budget) is
//! # reported. The BigTools impl below is well above 8 KB so
//! # this is the path CI exercises.
//! TOKITAI_PROFILE_BUDGET=8192 cargo run --example budget_check
//! ```
//!
//! # What the warning looks like
//!
//! When `TOKITAI_PROFILE=1` is set, the macro emits:
//!
//! ```text
//! cargo:warning=impl BigTools -> 200 tools, schema_bytes=NNNNN, est_tokens=NNNN
//! ```
//!
//! When `TOKITAI_PROFILE_BUDGET=8192` is set AND the impl's
//! `schema_bytes` exceeds 8192, the macro additionally emits:
//!
//! ```text
//! cargo:warning=impl BigTools -> 200 tools, schema_bytes=NNNNN exceeds budget=8192;
//!     consider splitting the impl or using #[wrap] to curate the exposed set
//! ```
//!
//! The build still succeeds — the budget is a hint, not a hard
//! error. Split the impl, narrow the exposed set with
//! `#[wrap(methods = [...])]`, or relax the budget.

use tokitai::tool;

// ---------------------------------------------------------------------------
// Deliberately large impl block so the budget path triggers.
//
// 200 methods × ~30 bytes of description per method ≈ 6 000
// bytes just for descriptions. With the macro's
// 4×-description proxy for the schema body, the total byte
// count for the impl lands well above the 8 192-byte default
// budget. The CI job (`.github/workflows/ci.yml`
// `budget-check`) builds this example with
// `TOKITAI_PROFILE_BUDGET=8192` and asserts the warning fires.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Default)]
pub struct BigTools;

#[tool]
impl BigTools {
    /// Return the value of synthetic tool number one.
    pub fn tool_001(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number two.
    pub fn tool_002(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number three.
    pub fn tool_003(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number four.
    pub fn tool_004(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number five.
    pub fn tool_005(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number six.
    pub fn tool_006(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seven.
    pub fn tool_007(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eight.
    pub fn tool_008(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number nine.
    pub fn tool_009(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ten.
    pub fn tool_010(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eleven.
    pub fn tool_011(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twelve.
    pub fn tool_012(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirteen.
    pub fn tool_013(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fourteen.
    pub fn tool_014(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifteen.
    pub fn tool_015(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixteen.
    pub fn tool_016(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventeen.
    pub fn tool_017(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighteen.
    pub fn tool_018(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number nineteen.
    pub fn tool_019(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty.
    pub fn tool_020(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-one.
    pub fn tool_021(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-two.
    pub fn tool_022(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-three.
    pub fn tool_023(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-four.
    pub fn tool_024(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-five.
    pub fn tool_025(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-six.
    pub fn tool_026(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-seven.
    pub fn tool_027(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-eight.
    pub fn tool_028(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number twenty-nine.
    pub fn tool_029(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty.
    pub fn tool_030(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-one.
    pub fn tool_031(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-two.
    pub fn tool_032(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-three.
    pub fn tool_033(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-four.
    pub fn tool_034(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-five.
    pub fn tool_035(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-six.
    pub fn tool_036(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-seven.
    pub fn tool_037(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-eight.
    pub fn tool_038(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number thirty-nine.
    pub fn tool_039(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty.
    pub fn tool_040(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-one.
    pub fn tool_041(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-two.
    pub fn tool_042(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-three.
    pub fn tool_043(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-four.
    pub fn tool_044(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-five.
    pub fn tool_045(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-six.
    pub fn tool_046(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-seven.
    pub fn tool_047(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-eight.
    pub fn tool_048(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number forty-nine.
    pub fn tool_049(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty.
    pub fn tool_050(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-one.
    pub fn tool_051(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-two.
    pub fn tool_052(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-three.
    pub fn tool_053(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-four.
    pub fn tool_054(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-five.
    pub fn tool_055(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-six.
    pub fn tool_056(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-seven.
    pub fn tool_057(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-eight.
    pub fn tool_058(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number fifty-nine.
    pub fn tool_059(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty.
    pub fn tool_060(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-one.
    pub fn tool_061(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-two.
    pub fn tool_062(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-three.
    pub fn tool_063(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-four.
    pub fn tool_064(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-five.
    pub fn tool_065(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-six.
    pub fn tool_066(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-seven.
    pub fn tool_067(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-eight.
    pub fn tool_068(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number sixty-nine.
    pub fn tool_069(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy.
    pub fn tool_070(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-one.
    pub fn tool_071(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-two.
    pub fn tool_072(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-three.
    pub fn tool_073(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-four.
    pub fn tool_074(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-five.
    pub fn tool_075(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-six.
    pub fn tool_076(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-seven.
    pub fn tool_077(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-eight.
    pub fn tool_078(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number seventy-nine.
    pub fn tool_079(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty.
    pub fn tool_080(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-one.
    pub fn tool_081(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-two.
    pub fn tool_082(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-three.
    pub fn tool_083(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-four.
    pub fn tool_084(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-five.
    pub fn tool_085(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-six.
    pub fn tool_086(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-seven.
    pub fn tool_087(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-eight.
    pub fn tool_088(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number eighty-nine.
    pub fn tool_089(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety.
    pub fn tool_090(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-one.
    pub fn tool_091(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-two.
    pub fn tool_092(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-three.
    pub fn tool_093(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-four.
    pub fn tool_094(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-five.
    pub fn tool_095(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-six.
    pub fn tool_096(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-seven.
    pub fn tool_097(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-eight.
    pub fn tool_098(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number ninety-nine.
    pub fn tool_099(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred.
    pub fn tool_100(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred one.
    pub fn tool_101(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred two.
    pub fn tool_102(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred three.
    pub fn tool_103(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred four.
    pub fn tool_104(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred five.
    pub fn tool_105(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred six.
    pub fn tool_106(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seven.
    pub fn tool_107(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eight.
    pub fn tool_108(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred nine.
    pub fn tool_109(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ten.
    pub fn tool_110(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eleven.
    pub fn tool_111(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twelve.
    pub fn tool_112(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirteen.
    pub fn tool_113(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fourteen.
    pub fn tool_114(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifteen.
    pub fn tool_115(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixteen.
    pub fn tool_116(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventeen.
    pub fn tool_117(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighteen.
    pub fn tool_118(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred nineteen.
    pub fn tool_119(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty.
    pub fn tool_120(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-one.
    pub fn tool_121(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-two.
    pub fn tool_122(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-three.
    pub fn tool_123(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-four.
    pub fn tool_124(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-five.
    pub fn tool_125(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-six.
    pub fn tool_126(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-seven.
    pub fn tool_127(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-eight.
    pub fn tool_128(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred twenty-nine.
    pub fn tool_129(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty.
    pub fn tool_130(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-one.
    pub fn tool_131(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-two.
    pub fn tool_132(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-three.
    pub fn tool_133(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-four.
    pub fn tool_134(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-five.
    pub fn tool_135(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-six.
    pub fn tool_136(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-seven.
    pub fn tool_137(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-eight.
    pub fn tool_138(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred thirty-nine.
    pub fn tool_139(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty.
    pub fn tool_140(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-one.
    pub fn tool_141(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-two.
    pub fn tool_142(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-three.
    pub fn tool_143(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-four.
    pub fn tool_144(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-five.
    pub fn tool_145(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-six.
    pub fn tool_146(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-seven.
    pub fn tool_147(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-eight.
    pub fn tool_148(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred forty-nine.
    pub fn tool_149(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty.
    pub fn tool_150(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-one.
    pub fn tool_151(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-two.
    pub fn tool_152(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-three.
    pub fn tool_153(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-four.
    pub fn tool_154(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-five.
    pub fn tool_155(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-six.
    pub fn tool_156(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-seven.
    pub fn tool_157(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-eight.
    pub fn tool_158(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred fifty-nine.
    pub fn tool_159(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty.
    pub fn tool_160(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-one.
    pub fn tool_161(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-two.
    pub fn tool_162(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-three.
    pub fn tool_163(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-four.
    pub fn tool_164(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-five.
    pub fn tool_165(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-six.
    pub fn tool_166(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-seven.
    pub fn tool_167(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-eight.
    pub fn tool_168(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred sixty-nine.
    pub fn tool_169(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy.
    pub fn tool_170(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-one.
    pub fn tool_171(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-two.
    pub fn tool_172(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-three.
    pub fn tool_173(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-four.
    pub fn tool_174(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-five.
    pub fn tool_175(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-six.
    pub fn tool_176(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-seven.
    pub fn tool_177(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-eight.
    pub fn tool_178(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred seventy-nine.
    pub fn tool_179(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty.
    pub fn tool_180(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-one.
    pub fn tool_181(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-two.
    pub fn tool_182(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-three.
    pub fn tool_183(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-four.
    pub fn tool_184(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-five.
    pub fn tool_185(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-six.
    pub fn tool_186(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-seven.
    pub fn tool_187(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-eight.
    pub fn tool_188(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred eighty-nine.
    pub fn tool_189(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety.
    pub fn tool_190(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-one.
    pub fn tool_191(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-two.
    pub fn tool_192(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-three.
    pub fn tool_193(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-four.
    pub fn tool_194(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-five.
    pub fn tool_195(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-six.
    pub fn tool_196(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-seven.
    pub fn tool_197(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-eight.
    pub fn tool_198(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number one hundred ninety-nine.
    pub fn tool_199(&self, input: String) -> String {
        input
    }
    /// Return the value of synthetic tool number two hundred.
    pub fn tool_200(&self, input: String) -> String {
        input
    }
}

// ---------------------------------------------------------------------------
// A second, *small* impl block so the budget warning is emitted
// for the BigTools impl only. The macro emits a separate
// cargo:warning= line per `#[tool]` impl block, so a single
// warning for BigTools + a quiet run for SmallTools is the
// expected output. The CI job asserts the BigTools line is
// present and the SmallTools line is NOT (because
// 3 × small-tool description bytes is well below the 8 KB
// budget).
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Default)]
pub struct SmallTools;

#[tool]
impl SmallTools {
    /// First small tool.
    pub fn one(&self, input: String) -> String {
        input
    }
    /// Second small tool.
    pub fn two(&self, input: String) -> String {
        input
    }
    /// Third small tool.
    pub fn three(&self, input: String) -> String {
        input
    }
}

fn main() {
    use tokitai::ToolProvider;

    // Confirm the tool set at runtime so the example is also a
    // smoke test: when TOKITAI_PROFILE_BUDGET is set, the macro
    // still emits a runnable binary that exposes all 200 BigTools
    // + 3 SmallTools methods.
    let big = BigTools::tool_definitions().len();
    let small = SmallTools::tool_definitions().len();
    println!("BigTools exposed {} tool definitions", big);
    println!("SmallTools exposed {} tool definitions", small);

    // The CI job greps the `cargo:warning=impl BigTools ...`
    // line out of `cargo build` output. We do NOT print a copy
    // of it from `main` — the warning is emitted at compile
    // time by the macro itself.
}
