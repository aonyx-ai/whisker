use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Error, LitStr, Result};

/// How many times a provider may record a decoration on a single node
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Cardinality {
    Many,
    One,
}

/// Builds the `Decoration` implementation for a derive input
///
/// # Errors
///
/// Returns an error if the `decoration` attribute is missing, names an option
/// other than `cardinality`, is given a cardinality other than `one` or
/// `many`, or declares a cardinality more than once.
pub(crate) fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let cardinality = cardinality(input)?;
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

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
        impl #impl_generics ::whisker_types::Decoration for #name #type_generics #where_clause {
            type Ref<'decoration> = #reference;

            fn lookup<'decoration>(
                node: &::whisker_types::DecoratedNode<'decoration>,
            ) -> Self::Ref<'decoration> {
                #lookup
            }
        }
    })
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
    fn expand_with_generic_type_carries_generics_onto_the_impl() {
        let input =
            parse(r#"#[decoration(cardinality = "one")] struct Wrapper<T>(T) where T: Clone;"#);

        let code = expand(&input).expect("should expand").to_string();

        assert!(code.contains("impl < T > :: whisker_types :: Decoration for Wrapper < T >"));
        assert!(code.contains("where T : Clone"));
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
