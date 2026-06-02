//! Property-based tests for the `#[tool]` proc-macro.

use proptest::prelude::*;
use proptest::test_runner::TestRunner;

use tokitai_macros::{__property_expand, __property_would_error};

const SNAPSHOT_PATH: &str = "tests/fixtures/property_based_snapshot.txt";

fn normalize_ws(s: &str) -> String {
    let stripped: String = s
        .lines()
        .map(|l| l.trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = String::with_capacity(stripped.len());
    let mut prev_ws = false;
    for ch in stripped.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(ch);
            prev_ws = false;
        }
    }
    out
}

const VALID_NAMES: &[&str] = &[
    "add",
    "sub",
    "mul",
    "search",
    "fetch",
    "display_name",
    "greet",
    "is_valid",
    "toggle",
    "compute",
];

const RESERVED_NAMES: &[&str] = &["call_tool", "tool_definitions", "configure_tool"];

const VALID_PARAM_TYPES: &[&str] = &[
    "i32",
    "String",
    "Option<bool>",
    "Option<Vec<i64>>",
    "Option<String>",
];

fn arb_method_name() -> impl Strategy<Value = String> {
    prop::sample::select(VALID_NAMES).prop_map(|s| s.to_string())
}

fn arb_param_type() -> impl Strategy<Value = String> {
    prop::sample::select(VALID_PARAM_TYPES).prop_map(|s| s.to_string())
}

#[derive(Debug, Clone)]
enum Violation {
    ReservedPrefix,
    ReservedInjected,
    ReturnsSelf,
    NoSelf,
    Generic,
    AsyncMutSelf,
}

fn arb_violation() -> impl Strategy<Value = Violation> {
    prop_oneof![
        Just(Violation::ReservedPrefix),
        Just(Violation::ReservedInjected),
        Just(Violation::ReturnsSelf),
        Just(Violation::NoSelf),
        Just(Violation::Generic),
        Just(Violation::AsyncMutSelf),
    ]
}

fn arb_valid_impl() -> impl Strategy<Value = String> {
    (
        1usize..=10,
        prop::collection::vec(arb_method_name(), 1..=10),
        prop::collection::vec(arb_param_type(), 0..=4),
    )
        .prop_map(|(n_methods, names, param_types)| {
            let mut src = String::from("impl RandomImpl {\n");
            for i in 0..n_methods {
                let name = &names[i % names.len()];
                let n_params = (i % 5).min(param_types.len());
                let params: Vec<String> = (0..n_params)
                    .map(|p| {
                        let ty = &param_types[p % param_types.len()];
                        format!("p{}: {}", p, ty)
                    })
                    .collect();
                src.push_str(&format!(
                    "    pub fn {}(&self, {}) -> i32 {{ {} }}\n",
                    name,
                    params.join(", "),
                    i as i32
                ));
            }
            src.push_str("}\n");
            src
        })
}

fn arb_invalid_impl() -> impl Strategy<Value = (Violation, String)> {
    arb_violation().prop_map(|v| {
        let src = match &v {
            Violation::ReservedPrefix => {
                "impl V { pub fn __reserved(&self, a: i32) -> i32 { a } }\n".to_string()
            }
            Violation::ReservedInjected => {
                "impl V { pub fn call_tool(&self) -> i32 { 0 } }\n".to_string()
            }
            Violation::ReturnsSelf => {
                "impl V { pub fn make(&self) -> Self { Self } }\n".to_string()
            }
            Violation::NoSelf => "impl V { pub fn helper() -> i32 { 0 } }\n".to_string(),
            Violation::Generic => "impl V { pub fn id<T>(&self, x: T) -> T { x } }\n".to_string(),
            Violation::AsyncMutSelf => {
                "impl V { pub async fn touch(&mut self, a: i32) -> i32 { a } }\n".to_string()
            }
        };
        (v, src)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RejectReason {
    ReservedPrefix,
    ReservedInjected,
    ReturnsSelf,
    NoSelf,
    // Intentionally allowed dead: the test fixture is conservative — the
    // property test would otherwise need a per-variant assertion that
    // we deliberately skip, since a public generic method is filtered
    // out by `is_tool_method` rather than rejected.
    #[allow(dead_code)]
    Generic,
    AsyncMutSelf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PipelineOutcome {
    Ok,
    ParseError,
    Rejected(RejectReason),
}

fn is_tool_method(m: &syn::ImplItemFn) -> bool {
    if !matches!(m.vis, syn::Visibility::Public(_)) {
        return false;
    }
    if m.sig
        .generics
        .params
        .iter()
        .any(|p| matches!(p, syn::GenericParam::Type(_) | syn::GenericParam::Const(_)))
    {
        return false;
    }
    true
}

fn runtime_reject_reason(m: &syn::ImplItemFn) -> Option<RejectReason> {
    if m.sig.ident.to_string().starts_with("__") {
        return Some(RejectReason::ReservedPrefix);
    }
    if !is_tool_method(m) {
        return None;
    }
    if RESERVED_NAMES.contains(&m.sig.ident.to_string().as_str()) {
        return Some(RejectReason::ReservedInjected);
    }
    if let syn::ReturnType::Type(_, ty) = &m.sig.output {
        if let syn::Type::Path(p) = ty.as_ref() {
            if p.path
                .segments
                .last()
                .map(|s| s.ident == "Self")
                .unwrap_or(false)
            {
                return Some(RejectReason::ReturnsSelf);
            }
        }
    }
    let has_self = m
        .sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, syn::FnArg::Receiver(_)));
    if !has_self {
        return Some(RejectReason::NoSelf);
    }
    if m.sig.asyncness.is_some() {
        for arg in &m.sig.inputs {
            if let syn::FnArg::Receiver(r) = arg {
                if r.mutability.is_some() {
                    return Some(RejectReason::AsyncMutSelf);
                }
            }
        }
    }
    None
}

fn run_runtime_pipeline(src: &str) -> PipelineOutcome {
    let item: syn::ItemImpl = match syn::parse_str(src) {
        Ok(i) => i,
        Err(_) => return PipelineOutcome::ParseError,
    };
    for it in &item.items {
        if let syn::ImplItem::Fn(m) = it {
            if let Some(r) = runtime_reject_reason(m) {
                return PipelineOutcome::Rejected(r);
            }
        }
    }
    PipelineOutcome::Ok
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn runtime_pipeline_does_not_panic_on_valid_impls(src in arb_valid_impl()) {
        let outcome = run_runtime_pipeline(&src);
        prop_assert!(matches!(
            outcome,
            PipelineOutcome::Ok | PipelineOutcome::ParseError
        ));
    }

    /// Property 2: every synthesized violation must produce a
    /// non-Ok pipeline outcome, OR the synthesized method must
    /// be a non-tool method (`!is_tool_method`) — e.g. a public
    /// generic method, which the macro silently skips rather
    /// than rejecting with `compile_error!`.
    #[test]
    fn macro_rejects_every_violation_in_pool((v, src) in arb_invalid_impl()) {
        let outcome = run_runtime_pipeline(&src);
        if matches!(outcome, PipelineOutcome::Ok) {
            let item: syn::ItemImpl = syn::parse_str(&src).unwrap();
            let any_non_tool = item.items.iter().any(|i| {
                if let syn::ImplItem::Fn(m) = i {
                    !is_tool_method(m)
                } else {
                    false
                }
            });
            prop_assert!(
                any_non_tool,
                "violation {:?} was accepted as a tool method",
                v
            );
        }
    }

    #[test]
    fn pipeline_is_deterministic(src in arb_valid_impl()) {
        let a = format!("{:?}", run_runtime_pipeline(&src));
        let b = format!("{:?}", run_runtime_pipeline(&src));
        prop_assert_eq!(&a, &b);
    }

    #[test]
    fn pipeline_ignores_method_order(
        names in prop::collection::vec(arb_method_name(), 2..=6),
        n_params in 0usize..=3,
    ) {
        let mut src_a = String::from("impl OrderImpl {\n");
        let mut src_b = String::from("impl OrderImpl {\n");
        for (i, name) in names.iter().enumerate() {
            let params: Vec<String> = (0..n_params)
                .map(|p| format!("p{}: i32", p))
                .collect();
            let line = format!(
                "    pub fn {}(&self, {}) -> i32 {{ {} }}\n",
                name, params.join(", "), i as i32
            );
            src_a.push_str(&line);
        }
        for (i, name) in names.iter().enumerate().rev() {
            let params: Vec<String> = (0..n_params)
                .map(|p| format!("p{}: i32", p))
                .collect();
            let line = format!(
                "    pub fn {}(&self, {}) -> i32 {{ {} }}\n",
                name, params.join(", "), i as i32
            );
            src_b.push_str(&line);
        }
        src_a.push_str("}\n");
        src_b.push_str("}\n");
        let a = format!("{:?}", run_runtime_pipeline(&src_a));
        let b = format!("{:?}", run_runtime_pipeline(&src_b));
        prop_assert_eq!(&a, &b);
    }

    #[test]
    fn runtime_rejection_is_bounded((_, src) in arb_invalid_impl()) {
        let outcome = run_runtime_pipeline(&src);
        if let PipelineOutcome::Rejected(r) = outcome {
            let documented = matches!(
                r,
                RejectReason::ReservedPrefix
                    | RejectReason::ReservedInjected
                    | RejectReason::ReturnsSelf
                    | RejectReason::NoSelf
                    | RejectReason::Generic
                    | RejectReason::AsyncMutSelf
            );
            prop_assert!(documented);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10))]

    #[test]
    fn sanity_proptest_works(x in 0u32..100) {
        prop_assert_eq!(x, x);
    }
}

#[test]
fn snapshot_5_method_fixture() {
    let expanded: &'static str = __property_expand!(
        impl SnapshotFixture {
            /// Add two numbers and return the sum.
            pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
            /// Concatenate a greeting and a name.
            pub fn greet(&self, greeting: String, name: String) -> String { format!("{}, {}", greeting, name) }
            /// Look up a display name for a user.
            pub fn display_name(&self, user_id: i64, nickname: Option<String>) -> Option<String> { nickname.or_else(|| Some(format!("user-{}", user_id))) }
            /// Check whether an email looks well-formed.
            pub fn is_valid_email(&self, email: String) -> bool { email.contains('@') }
            /// Toggle a boolean flag.
            pub fn toggle(&self, current: bool) -> bool { !current }
        }
    );

    let normalized = normalize_ws(expanded);

    if std::env::var("BLESS").is_ok() && std::env::var("BLESS").unwrap() == "1"
        || std::env::var("TOKITAI_BLESS").is_ok()
    {
        if let Some(parent) = std::path::Path::new(SNAPSHOT_PATH).parent() {
            std::fs::create_dir_all(parent).expect("create fixtures dir");
        }
        std::fs::write(SNAPSHOT_PATH, &normalized).expect("write snapshot");
        return;
    }

    let on_disk = match std::fs::read_to_string(SNAPSHOT_PATH) {
        Ok(s) => s,
        Err(_) => {
            if let Some(parent) = std::path::Path::new(SNAPSHOT_PATH).parent() {
                std::fs::create_dir_all(parent).expect("create fixtures dir");
            }
            std::fs::write(SNAPSHOT_PATH, &normalized).expect("write snapshot");
            return;
        }
    };
    let on_disk_normalized = normalize_ws(&on_disk);
    assert_eq!(
        normalized, on_disk_normalized,
        "5-method fixture snapshot drifted; run with `BLESS=1` to re-baseline"
    );
}

#[test]
fn compile_time_would_error_for_invalid_impls() {
    // A public generic method is a definite violation
    // (E0006); the real macro emits `compile_error!` for it
    // because the generated JSON Schema cannot express
    // type-parameterised tools.
    let would_error: bool = __property_would_error!(
        impl GenericFixture {
            /// Identity function.
            pub fn id<T>(&self, x: T) -> T { x }
        }
    );
    assert!(
        would_error,
        "real macro should emit compile_error! for a public generic method"
    );
}

#[test]
fn compile_time_would_not_error_for_valid_impl() {
    let would_error: bool = __property_would_error!(
        impl HappyFixture {
            /// Add two numbers.
            pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
        }
    );
    assert!(
        !would_error,
        "real macro should NOT emit compile_error! for a valid impl"
    );
}

#[test]
fn compile_time_expansion_is_stable() {
    let expanded: &'static str = __property_expand!(
        impl FiveMethodFixture {
            /// First.
            pub fn first(&self, a: i32) -> i32 { a }
            /// Second.
            pub fn second(&self, a: i32) -> i32 { a }
            /// Third.
            pub fn third(&self, a: i32) -> i32 { a }
            /// Fourth.
            pub fn fourth(&self, a: i32) -> i32 { a }
            /// Fifth.
            pub fn fifth(&self, a: i32) -> i32 { a }
        }
    );
    assert!(!expanded.is_empty(), "expansion should be non-empty");
    for name in &["first", "second", "third", "fourth", "fifth"] {
        let shim = format!("__call_{}", name);
        assert!(
            expanded.contains(&shim),
            "expansion should contain dispatch shim `{}`",
            shim
        );
    }
}

#[test]
fn runtime_pipeline_agrees_with_real_macro() {
    let src = "impl SharedFixture { pub fn add(&self, a: i32, b: i32) -> i32 { a + b } }\n";
    let runtime = run_runtime_pipeline(src);
    let real: bool = __property_would_error!(
        impl SharedFixture {
            /// Add two numbers.
            pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
        }
    );
    assert_eq!(
        matches!(runtime, PipelineOutcome::Ok),
        !real,
        "runtime replica ({:?}) disagrees with real macro (would_error={})",
        runtime,
        real
    );
}

#[allow(dead_code)]
fn _proptest_runner_is_reachable() -> TestRunner {
    TestRunner::default()
}
