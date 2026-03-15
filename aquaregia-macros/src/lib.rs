//! Procedural macros for the Aquaregia crate.
//!
//! This crate provides the `#[tool]` procedural macro for concise tool definitions
//! in Aquaregia agents.
//!
//! ## The `#[tool]` Macro
//!
//! The `#[tool]` macro generates a function that returns a [`aquaregia::Tool`] with
//! automatic JSON Schema derivation and typed argument handling.
//!
//! ### Usage
//!
//! ```rust,no_run
//! use aquaregia::tool;
//! use serde_json::{Value, json};
//!
//! /// Get weather information by city
//! #[tool(description = "Get weather by city")]
//! async fn get_weather(city: String) -> Result<Value, String> {
//!     Ok(json!({ "city": city, "temp_c": 23, "condition": "sunny" }))
//! }
//!
//! // The macro generates:
//! // - A struct `get_weather_args` with Deserialize and JsonSchema derives
//! // - A function `get_weather()` that returns a `Tool`
//! // - Automatic schema validation for arguments
//! ```
//!
//! ### Macro Requirements
//!
//! - Must be an `async fn`
//! - Must return `Result<Value, String>` or similar error type
//! - Parameters must be simple identifiers with types (no patterns)
//! - No generic parameters or where clauses (currently)
//! - No `self` receivers (must be free functions)
//!
//! ### Generated Code
//!
//! For a function like:
//!
//! ```rust,ignore
//! #[tool(description = "Example tool")]
//! async fn example(x: String, y: i32) -> Result<Value, String> {
//!     // body
//! }
//! ```
//!
//! The macro generates:
//!
//! ```rust,ignore
//! #[derive(Deserialize, JsonSchema)]
//! struct __AquaregiaToolArgs_example {
//!     x: String,
//!     y: i32,
//! }
//!
//! fn example() -> Tool {
//!     tool("example")
//!         .description("Example tool")
//!         .execute(|args: __AquaregiaToolArgs_example| async move {
//!             __aquaregia_tool_handler_example(args.x, args.y)
//!                 .await
//!                 .map_err(|err| ToolExecError::Execution(err.to_string()))
//!         })
//! }
//!
//! async fn __aquaregia_tool_handler_example(x: String, y: i32) -> Result<Value, String> {
//!     // original body
//! }
//! ```

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::spanned::Spanned;
use syn::{
    Expr, FnArg, ItemFn, Lit, LitStr, Meta, Pat, Token, punctuated::Punctuated, parse_macro_input,
};

#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    let metas = match Punctuated::<Meta, Token![,]>::parse_terminated.parse(attr) {
        Ok(metas) => metas,
        Err(err) => return err.to_compile_error().into(),
    };

    let mut description: Option<LitStr> = None;
    for meta in metas {
        match meta {
            Meta::NameValue(name_value) if name_value.path.is_ident("description") => {
                if description.is_some() {
                    return syn::Error::new(
                        name_value.span(),
                        "duplicate `description` argument in #[tool(...)]",
                    )
                    .to_compile_error()
                    .into();
                }
                match name_value.value {
                    Expr::Lit(expr_lit) => match expr_lit.lit {
                        Lit::Str(lit) => description = Some(lit),
                        _ => {
                            return syn::Error::new(
                                expr_lit.span(),
                                "`description` must be a string literal",
                            )
                            .to_compile_error()
                            .into();
                        }
                    },
                    _ => {
                        return syn::Error::new(
                            name_value.value.span(),
                            "`description` must be a string literal",
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            }
            other => {
                return syn::Error::new(
                    other.span(),
                    "unsupported #[tool(...)] argument; expected `description = \"...\"`",
                )
                .to_compile_error()
                .into();
            }
        }
    }

    let input = parse_macro_input!(item as ItemFn);
    if input.sig.asyncness.is_none() {
        return syn::Error::new(
            input.sig.fn_token.span(),
            "#[tool] requires an `async fn` handler",
        )
        .to_compile_error()
        .into();
    }
    if !input.sig.generics.params.is_empty() || input.sig.generics.where_clause.is_some() {
        return syn::Error::new(
            input.sig.generics.span(),
            "#[tool] does not support generic parameters yet",
        )
        .to_compile_error()
        .into();
    }

    let vis = input.vis;
    let attrs = input.attrs;
    let fn_name = input.sig.ident;
    let output = input.sig.output;
    let body = input.block;

    let mut arg_idents = Vec::new();
    let mut arg_tys = Vec::new();
    for arg in input.sig.inputs {
        match arg {
            FnArg::Receiver(receiver) => {
                return syn::Error::new(
                    receiver.span(),
                    "#[tool] does not support methods with `self`",
                )
                .to_compile_error()
                .into();
            }
            FnArg::Typed(pat_type) => {
                let ident = match *pat_type.pat {
                    Pat::Ident(pat_ident)
                        if pat_ident.by_ref.is_none()
                            && pat_ident.mutability.is_none()
                            && pat_ident.subpat.is_none() =>
                    {
                        pat_ident.ident
                    }
                    other => {
                        return syn::Error::new(
                            other.span(),
                            "#[tool] parameters must be simple identifiers, e.g. `city: String`",
                        )
                        .to_compile_error()
                        .into();
                    }
                };
                arg_idents.push(ident);
                arg_tys.push(*pat_type.ty);
            }
        }
    }

    let description_lit =
        description.unwrap_or_else(|| LitStr::new("", proc_macro2::Span::call_site()));
    let args_ident = format_ident!("__AquaregiaToolArgs_{}", fn_name);
    let handler_ident = format_ident!("__aquaregia_tool_handler_{}", fn_name);
    let arg_extracts = arg_idents.iter().map(|ident| quote! { args.#ident });

    quote! {
        #(#attrs)*
        #vis fn #fn_name() -> ::aquaregia::Tool {
            #[allow(non_camel_case_types)]
            #[derive(::aquaregia::__aquaregia_serde::Deserialize, ::aquaregia::__aquaregia_schemars::JsonSchema)]
            struct #args_ident {
                #( #arg_idents: #arg_tys, )*
            }

            ::aquaregia::tool(stringify!(#fn_name))
                .description(#description_lit)
                .execute(|args: #args_ident| async move {
                    #handler_ident( #( #arg_extracts ),* )
                        .await
                        .map_err(|err| ::aquaregia::ToolExecError::Execution(err.to_string()))
                })
        }

        async fn #handler_ident( #( #arg_idents: #arg_tys ),* ) #output {
            #body
        }
    }
    .into()
}
