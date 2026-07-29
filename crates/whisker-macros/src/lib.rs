mod decoration;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Implements `Decoration` for a type, fixing how it reads back from a node
///
/// The cardinality attribute is required and decides the result of
/// `DecoratedNode::get` for this type: `one` yields `Option<&Self>` for a
/// decoration a provider records at most once per node, `many` yields
/// `Vec<&Self>` for one it may record repeatedly. Declaring it on the type
/// rather than at each call site means a rule cannot read a repeated
/// decoration as though it were singular.
///
/// # Examples
///
/// ```
/// use whisker_macros::Decoration;
///
/// #[derive(Decoration)]
/// #[decoration(cardinality = "one")]
/// pub struct ResolvedType {
///     display: String,
/// }
///
/// #[derive(Decoration)]
/// #[decoration(cardinality = "many")]
/// pub struct TraitImpl {
///     name: String,
/// }
/// ```
#[proc_macro_derive(Decoration, attributes(decoration))]
pub fn derive_decoration(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    decoration::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
