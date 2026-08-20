use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Error, LitStr, Result};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// How many times a provider may record a decoration on a single node
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Cardinality {
    Many,
    One,
}

/// Builds the `Decoration` implementation for a derive input
///
/// The emitted implementation is an `unsafe impl`, because `Decoration`'s
/// key contract is a safety obligation: a key must name exactly one type
/// definition. The derive discharges it by building the key from the
/// defining module's path, the type's name, and [`definition_hash`], and
/// by rejecting generic types, whose single key would have to cover one
/// layout per instantiation.
///
/// # Errors
///
/// Returns an error if the type is generic, if the `decoration` attribute is
/// missing, names an option other than `cardinality`, is given a cardinality
/// other than `one` or `many`, or declares a cardinality more than once.
pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    if let Some(parameter) = input.generics.params.first() {
        return Err(Error::new_spanned(
            parameter,
            "a decoration cannot be generic, because its key must name exactly one type",
        ));
    }

    let cardinality = cardinality(input)?;
    let name = &input.ident;
    let hash = definition_hash(input);
    let where_clause = &input.generics.where_clause;

    let (reference, lookup) = match cardinality {
        Cardinality::Many => (
            quote!(::std::vec::Vec<&'decoration Self>),
            quote!(node.decorations_of_type::<Self>()),
        ),
        Cardinality::One => (
            quote!(::core::option::Option<&'decoration Self>),
            quote!(node.decoration::<Self>()),
        ),
    };

    Ok(quote! {
        #[automatically_derived]
        unsafe impl ::whisker_types::Decoration for #name #where_clause {
            const KEY: ::whisker_types::DecorationKey = ::whisker_types::DecorationKey::new(
                ::core::concat!(
                    ::core::module_path!(),
                    "::",
                    ::core::stringify!(#name),
                    "#",
                    #hash,
                ),
            );

            type Ref<'decoration> = #reference;

            fn lookup<'decoration>(
                node: &::whisker_types::DecoratedNode<'decoration>,
            ) -> Self::Ref<'decoration> {
                #lookup
            }
        }
    })
}

/// Hashes a type's definition into sixteen hexadecimal digits
///
/// [`module_path!`] stops at module boundaries, so two types of one name
/// in two function bodies of the same module share a path and a name.
/// Their definitions still differ, and folding those into the key keeps
/// the two apart. The hash covers the item as written, so both images
/// compiling one source derive the same digits, which is what lets a key
/// travel across the plugin boundary at all.
///
/// Two byte-identical definitions in two scopes of one module remain
/// indistinguishable. They describe the same layout, so reading one as
/// the other cannot misread a field, and the plugin handshake already
/// rests on one compiler laying identical definitions out identically.
///
/// [`module_path!`]: std::module_path
fn definition_hash(input: &DeriveInput) -> String {
    let mut hash = FNV_OFFSET_BASIS;

    for byte in quote!(#input).to_string().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    format!("{hash:016x}")
}

/// Reads the cardinality declared by the type's `decoration` attribute
fn cardinality(input: &DeriveInput) -> Result<Cardinality> {
    let mut declared: Option<Cardinality> = None;

    for attr in &input.attrs {
        if !attr.path().is_ident("decoration") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("cardinality") {
                return Err(meta.error("expected `cardinality`"));
            }

            let value: LitStr = meta.value()?.parse()?;
            let parsed = match value.value().as_str() {
                "many" => Cardinality::Many,
                "one" => Cardinality::One,
                other => {
                    return Err(Error::new(
                        value.span(),
                        format!("expected `one` or `many`, found `{other}`"),
                    ));
                }
            };

            if declared.replace(parsed).is_some() {
                return Err(Error::new(value.span(), "cardinality declared twice"));
            }

            Ok(())
        })?;
    }

    declared.ok_or_else(|| {
        Error::new_spanned(
            &input.ident,
            "missing `#[decoration(cardinality = \"one\")]` or \
             `#[decoration(cardinality = \"many\")]`",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> DeriveInput {
        syn::parse_str(source).expect("should parse as a derive input")
    }

    #[test]
    fn expand_builds_key_from_module_path_name_and_definition() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains("const KEY : :: whisker_types :: DecorationKey"));
        assert!(code.contains(":: core :: module_path ! ()"));
        assert!(code.contains(":: core :: stringify ! (Flag)"));
        assert!(code.contains(&format!("{:?}", definition_hash(&input))));
    }

    #[test]
    fn expand_emits_an_unsafe_impl() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains("unsafe impl :: whisker_types :: Decoration for Flag"));
    }

    #[test]
    fn expand_gives_one_definition_the_same_key_twice() {
        let source = r#"#[decoration(cardinality = "one")] struct Flag(u32);"#;

        let first = expand(&parse(source)).expect("should expand").to_string();
        let second = expand(&parse(source)).expect("should expand").to_string();

        assert_eq!(first, second);
    }

    #[test]
    fn expand_gives_same_named_types_of_different_shape_different_keys() {
        let counted = parse(r#"#[decoration(cardinality = "one")] struct Flag(u32);"#);
        let named = parse(r#"#[decoration(cardinality = "one")] struct Flag(String);"#);

        let counted = definition_hash(&counted);
        let named = definition_hash(&named);

        assert_ne!(counted, named);
    }

    #[test]
    fn expand_with_generic_type_returns_error() {
        let input =
            parse(r#"#[decoration(cardinality = "one")] struct Wrapper<T>(T) where T: Clone;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("cannot be generic"));
    }

    #[test]
    fn expand_with_lifetime_parameter_returns_error() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Borrowed<'a>(&'a str);"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("cannot be generic"));
    }

    #[test]
    fn expand_with_many_cardinality_returns_a_vec() {
        let input = parse(r#"#[decoration(cardinality = "many")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains(":: std :: vec :: Vec < & 'decoration Self >"));
        assert!(code.contains("decorations_of_type :: < Self > ()"));
    }

    #[test]
    fn expand_with_one_cardinality_returns_an_option() {
        let input = parse(r#"#[decoration(cardinality = "one")] struct Flag;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains(":: core :: option :: Option < & 'decoration Self >"));
        assert!(code.contains("decoration :: < Self > ()"));
    }

    #[test]
    fn expand_with_repeated_cardinality_returns_error() {
        let input =
            parse(r#"#[decoration(cardinality = "one", cardinality = "many")] struct Flag;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("declared twice"));
    }

    #[test]
    fn expand_with_unknown_cardinality_returns_error() {
        let input = parse(r#"#[decoration(cardinality = "several")] struct Flag;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("expected `one` or `many`"));
    }

    #[test]
    fn expand_with_unknown_option_returns_error() {
        let input = parse(r#"#[decoration(shape = "one")] struct Flag;"#);

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("expected `cardinality`"));
    }

    #[test]
    fn expand_without_attribute_returns_error() {
        let input = parse("struct Flag;");

        let error = expand(&input).expect_err("should reject");

        assert!(error.to_string().contains("missing"));
    }
}
