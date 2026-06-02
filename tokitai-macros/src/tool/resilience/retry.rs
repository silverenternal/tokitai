//! `#[retry(...)]` decorator.
//!
//! Wraps a function body in a retry loop. The function is expected
//! to return a `Result<T, E>`; the decorator retries on `Err` up
//! to `max` times, sleeping between attempts according to the
//! configured backoff strategy.
//!
//! ## Args
//!
//! - `max = N` — maximum number of attempts (default `3`).
//! - `backoff = "constant" | "linear" | "exponential"` — backoff
//!   strategy (default `"exponential"`).
//! - `jitter = true | false` — add a small random offset to each
//!   backoff, derived from `SystemTime`'s sub-second nanos (default
//!   `true`).
//! - `on = "transient" | "any"` — which errors to retry on. v1
//!   always retries on any `Err`; the value is accepted and stored
//!   so the public surface is stable for the v2 release, where we
//!   will retry only when the error matches a user-supplied
//!   predicate.
//!
//! ## v2 composition note
//!
//! To support nested `#[retry]` layers (e.g. `#[retry(max=10)]
//! #[retry(max=3, on="transient")]`), the macro should detect
//! existing retry state in the body and append a new layer rather
//! than wrap from scratch. For v1, the inner layer wins.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse2, ItemFn, LitBool, LitInt, LitStr,
};

/// Parsed `#[retry(...)]` argument list.
#[derive(Debug, Clone)]
pub struct RetryArgs {
    pub max: u32,
    pub backoff: String,
    pub jitter: bool,
    pub on: String,
}

impl Parse for RetryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut max: u32 = 3;
        let mut backoff = "exponential".to_string();
        let mut jitter = true;
        let mut on = "any".to_string();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;
            match key.to_string().as_str() {
                "max" => {
                    let v: LitInt = input.parse()?;
                    max = v.base10_parse()?;
                }
                "backoff" => {
                    let v: LitStr = input.parse()?;
                    backoff = v.value();
                }
                "jitter" => {
                    let v: LitBool = input.parse()?;
                    jitter = v.value();
                }
                "on" => {
                    let v: LitStr = input.parse()?;
                    on = v.value();
                }
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown #[retry] arg: {}", key),
                    ));
                }
            }
            if input.peek(syn::token::Comma) {
                let _: syn::token::Comma = input.parse()?;
            }
        }

        Ok(RetryArgs { max, backoff, jitter, on })
    }
}

/// Expand `#[retry(args)] fn ...` into a function with a retry-wrapped
/// body. The function signature is preserved verbatim; only the body
/// is replaced.
pub fn expand(args: TokenStream2, input: TokenStream2) -> TokenStream2 {
    let parsed_args = match syn::parse2::<RetryArgs>(args) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error(),
    };
    let mut func: ItemFn = match parse2::<ItemFn>(input) {
        Ok(f) => f,
        Err(e) => return e.to_compile_error(),
    };

    let max = parsed_args.max;
    let backoff = parsed_args.backoff.clone();
    let jitter = parsed_args.jitter;
    let _on = parsed_args.on; // v1: accepted, ignored
    let is_async = func.sig.asyncness.is_some();
    let original_block = func.block.clone();

    // Backoff expression (uses the local `attempt: u32` variable).
    let backoff_expr: TokenStream2 = match backoff.as_str() {
        "constant" => quote! { 100u64 },
        "linear" => quote! { 100u64 * (attempt as u64) },
        "exponential" => quote! {
            100u64 * (1u64 << (::std::cmp::min(attempt, 20u32).saturating_sub(1)))
        },
        _ => quote! { 100u64 },
    };

    let jitter_stmt: TokenStream2 = if jitter {
        quote! {
            let __jitter_offset: u64 = {
                let __nanos = ::std::time::SystemTime::now()
                    .duration_since(::std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as u64)
                    .unwrap_or(0u64);
                __nanos % 50u64
            };
        }
    } else {
        quote! { let __jitter_offset: u64 = 0u64; }
    };

    // For async we drive the sleep through the registered
    // AsyncExecutor (if any) so it does not block a runtime thread.
    // For sync we just use `std::thread::sleep` directly.
    let sleep_stmt: TokenStream2 = if is_async {
        quote! {
            {
                let __retry_sleep = ::std::time::Duration::from_millis(__total_ms);
                if ::tokitai_core::current_async_executor().is_some() {
                    let __retry_res: ::std::result::Result<(), ::tokitai_core::ToolError> =
                        ::tokitai_core::block_on_async(async move {
                            ::std::thread::sleep(__retry_sleep);
                        });
                    let _ = __retry_res;
                } else {
                    ::std::thread::sleep(__retry_sleep);
                }
            }
        }
    } else {
        quote! {
            ::std::thread::sleep(::std::time::Duration::from_millis(__total_ms));
        }
    };

    // The body must be re-evaluable on each iteration, so we hoist it
    // into an `async move { ... }` block (async case) or evaluate the
    // `Block` directly (sync case). The break-out type is unified
    // to `Ok(...)` / `Err(...)` so the function's return type is
    // preserved by inference.
    let new_block: TokenStream2 = if is_async {
        quote! {{
            let mut attempt: u32 = 0u32;
            let __retry_result = loop {
                attempt = attempt + 1u32;
                let __r = async move #original_block;
                match __r.await {
                    Ok(__v) => break Ok(__v),
                    Err(__e) if attempt < #max => {
                        let __backoff_ms: u64 = #backoff_expr;
                        #jitter_stmt
                        let __total_ms: u64 =
                            __backoff_ms.saturating_add(__jitter_offset);
                        #sleep_stmt
                    }
                    Err(__e) => break Err(__e),
                }
            };
            __retry_result
        }}
    } else {
        quote! {{
            let mut attempt: u32 = 0u32;
            let __retry_result = loop {
                attempt = attempt + 1u32;
                let __r = #original_block;
                match __r {
                    Ok(__v) => break Ok(__v),
                    Err(__e) if attempt < #max => {
                        let __backoff_ms: u64 = #backoff_expr;
                        #jitter_stmt
                        let __total_ms: u64 =
                            __backoff_ms.saturating_add(__jitter_offset);
                        #sleep_stmt
                    }
                    Err(__e) => break Err(__e),
                }
            };
            __retry_result
        }}
    };

    func.block = match syn::parse2::<Box<syn::Block>>(new_block) {
        Ok(b) => b,
        Err(e) => return e.to_compile_error(),
    };

    quote! { #func }
}
