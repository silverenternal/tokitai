//! `#[circuit_breaker(failure_threshold = N, reset_timeout = "30s")]`
//! decorator.
//!
//! Implements the classic 3-state circuit breaker
//! (closed / open / half-open) using static atomics.
//!
//! ## State machine
//!
//! - **Closed** (`__CB_STATE == 0`): calls pass through. On each
//!   `Err` the failure counter is incremented; if it reaches
//!   `failure_threshold` we transition to **Open** and record the
//!   current time as `open_at_ns`.
//! - **Open** (`__CB_STATE == 1`): if the wall clock is still
//!   within `reset_timeout` of `open_at_ns`, calls are routed to
//!   the body anyway in v1 (fail-fast is a v2 feature — see the
//!   note below). When the timeout has elapsed we transition to
//!   **Half-Open**.
//! - **Half-Open** (`__CB_STATE == 2`): the next call is allowed
//!   through. If it succeeds we close the circuit (state 0,
//!   failures 0); if it fails we re-open it.
//!
//! ## v1 limitation: fail-fast
//!
//! v1 does not synthesise an error when the circuit is open;
//! instead it lets the body run so that the call still observes
//! the current state. This keeps the error type generic (no
//! `E: From<String>` bound on the user's error). For fail-fast
//! behaviour, the user can read `__CB_STATE` (a `pub(crate)`
//! re-export) at the start of the body and return early.
//!
//! v2 will introduce a `CircuitOpen` trait that the user's error
//! type implements, and the macro will call `<E as CircuitOpen>::open()`
//! to synthesise the fast-fail error.
//!
//! ## v2 composition note
//!
//! Per-function statics avoid collisions; nested
//! `#[circuit_breaker]`s would each track their own state.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse2, ItemFn, LitInt, LitStr};

/// Parsed `#[circuit_breaker(...)]` argument list.
#[derive(Debug, Clone)]
pub struct CircuitBreakerArgs {
    pub failure_threshold: u32,
    pub reset_timeout: String,
}

impl syn::parse::Parse for CircuitBreakerArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut failure_threshold: u32 = 5;
        let mut reset_timeout = "30s".to_string();
        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            match key.to_string().as_str() {
                "failure_threshold" => {
                    let v: LitInt = input.parse()?;
                    failure_threshold = v.base10_parse()?;
                }
                "reset_timeout" => {
                    let v: LitStr = input.parse()?;
                    reset_timeout = v.value();
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown #[circuit_breaker] arg: {}", key),
                    ));
                }
            }
            if input.peek(syn::token::Comma) {
                let _: syn::token::Comma = input.parse()?;
            }
        }
        Ok(CircuitBreakerArgs {
            failure_threshold,
            reset_timeout,
        })
    }
}

/// Parse a human duration string into nanoseconds. Accepts suffixes
/// `ms`, `s`, `m`, `h`, and a bare integer (interpreted as seconds).
fn parse_duration_ns(s: &str) -> u64 {
    let s = s.trim();
    if let Some(rest) = s.strip_suffix("ms") {
        rest.parse::<u64>().unwrap_or(0) * 1_000_000
    } else if let Some(rest) = s.strip_suffix('s') {
        rest.parse::<u64>().unwrap_or(0) * 1_000_000_000
    } else if let Some(rest) = s.strip_suffix('m') {
        rest.parse::<u64>().unwrap_or(0) * 60 * 1_000_000_000
    } else if let Some(rest) = s.strip_suffix('h') {
        rest.parse::<u64>().unwrap_or(0) * 3600 * 1_000_000_000
    } else if let Ok(n) = s.parse::<u64>() {
        n * 1_000_000_000
    } else {
        0
    }
}

/// Expand `#[circuit_breaker(args)] fn ...` into a function whose
/// body is wrapped with a 3-state circuit breaker. The function
/// signature is preserved.
pub fn expand(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let parsed = match syn::parse2::<CircuitBreakerArgs>(args) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let mut func: ItemFn = match parse2::<ItemFn>(input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let failure_threshold = parsed.failure_threshold;
    let reset_timeout_ns = parse_duration_ns(&parsed.reset_timeout);
    let is_async = func.sig.asyncness.is_some();
    let original_block = func.block.clone();

    // The statics are declared at the outermost block scope so
    // that both the pre-call guard and the post-call handler can
    // see them. The guard does the state-check + open->half-open
    // transition; the handler does the success/Err-driven update.
    let new_block: TokenStream2 = if is_async {
        quote! {{
            // 0 = Closed, 1 = Open, 2 = HalfOpen
            static __CB_STATE: ::std::sync::atomic::AtomicU8 =
                ::std::sync::atomic::AtomicU8::new(0u8);
            static __CB_FAILURES: ::std::sync::atomic::AtomicU32 =
                ::std::sync::atomic::AtomicU32::new(0u32);
            static __CB_OPEN_AT_NS: ::std::sync::atomic::AtomicU64 =
                ::std::sync::atomic::AtomicU64::new(0u64);
            use ::std::sync::atomic::Ordering;

            let __cb_threshold: u32 = #failure_threshold;
            let __cb_reset_ns: u64 = #reset_timeout_ns;
            let __cb_now_ns: u64 = ::std::time::SystemTime::now()
                .duration_since(::std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0u64);
            let __cb_state: u8 = __CB_STATE.load(Ordering::Relaxed);

            if __cb_state == 1u8 {
                let __cb_open_at: u64 = __CB_OPEN_AT_NS.load(Ordering::Relaxed);
                if __cb_now_ns.saturating_sub(__cb_open_at) >= __cb_reset_ns {
                    // Transition to HalfOpen; the call below is the probe.
                    __CB_STATE.store(2u8, Ordering::Relaxed);
                }
                // else: circuit is open; v1 still runs the body so the
                // call observes the current state. v2 will add a
                // fail-fast early-return here.
            }

            let __cb_inner = async move #original_block;
            match __cb_inner.await {
                Ok(__cb_v) => {
                    __CB_STATE.store(0u8, Ordering::Relaxed);
                    __CB_FAILURES.store(0u32, Ordering::Relaxed);
                    Ok::<_, _>(__cb_v)
                }
                Err(__cb_e) => {
                    let __cb_f = __CB_FAILURES
                        .fetch_add(1u32, Ordering::Relaxed)
                        + 1u32;
                    if __cb_f >= __cb_threshold {
                        __CB_STATE.store(1u8, Ordering::Relaxed);
                        __CB_OPEN_AT_NS.store(__cb_now_ns, Ordering::Relaxed);
                    }
                    Err::<_, _>(__cb_e)
                }
            }
        }}
    } else {
        quote! {{
            // 0 = Closed, 1 = Open, 2 = HalfOpen
            static __CB_STATE: ::std::sync::atomic::AtomicU8 =
                ::std::sync::atomic::AtomicU8::new(0u8);
            static __CB_FAILURES: ::std::sync::atomic::AtomicU32 =
                ::std::sync::atomic::AtomicU32::new(0u32);
            static __CB_OPEN_AT_NS: ::std::sync::atomic::AtomicU64 =
                ::std::sync::atomic::AtomicU64::new(0u64);
            use ::std::sync::atomic::Ordering;

            let __cb_threshold: u32 = #failure_threshold;
            let __cb_reset_ns: u64 = #reset_timeout_ns;
            let __cb_now_ns: u64 = ::std::time::SystemTime::now()
                .duration_since(::std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0u64);
            let __cb_state: u8 = __CB_STATE.load(Ordering::Relaxed);

            if __cb_state == 1u8 {
                let __cb_open_at: u64 = __CB_OPEN_AT_NS.load(Ordering::Relaxed);
                if __cb_now_ns.saturating_sub(__cb_open_at) >= __cb_reset_ns {
                    __CB_STATE.store(2u8, Ordering::Relaxed);
                }
            }

            match #original_block {
                Ok(__cb_v) => {
                    __CB_STATE.store(0u8, Ordering::Relaxed);
                    __CB_FAILURES.store(0u32, Ordering::Relaxed);
                    Ok::<_, _>(__cb_v)
                }
                Err(__cb_e) => {
                    let __cb_f = __CB_FAILURES
                        .fetch_add(1u32, Ordering::Relaxed)
                        + 1u32;
                    if __cb_f >= __cb_threshold {
                        __CB_STATE.store(1u8, Ordering::Relaxed);
                        __CB_OPEN_AT_NS.store(__cb_now_ns, Ordering::Relaxed);
                    }
                    Err::<_, _>(__cb_e)
                }
            }
        }}
    };

    func.block = match syn::parse2::<Box<syn::Block>>(new_block) {
        Ok(b) => b,
        Err(e) => return e.to_compile_error(),
    };

    quote! { #func }
}
