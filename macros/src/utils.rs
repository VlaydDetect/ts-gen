use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{
    spanned::Spanned, Attribute, Error, Expr, ExprLit, GenericParam, Generics, Lit, Meta, Path,
    Result, Token, TypeParamBound,
};

use super::attr::Attr;
#[cfg(feature = "serde-compat")]
use super::attr::Serde;
use crate::deps::Dependencies;

macro_rules! syn_err {
    ($l:literal $(, $a:expr)*) => {
        syn_err!(proc_macro2::Span::call_site(); $l $(, $a)*)
    };
    ($s:expr; $l:literal $(, $a:expr)*) => {
        return Err(syn::Error::new($s, format!($l $(, $a)*)))
    };
}

macro_rules! syn_err_spanned {
    ($s:expr; $l:literal $(, $a:expr)*) => {
        return Err(syn::Error::new_spanned($s, format!($l $(, $a)*)))
    };
}

macro_rules! impl_parse {
    ($i:ident $(<$inner: ident>)? ($input:ident, $out:ident) { $($k:pat => $e:expr),* $(,)? }) => {
        impl std::convert::TryFrom<&syn::Attribute> for $i $(<$inner>)? {
            type Error = syn::Error;

            fn try_from(attr: &syn::Attribute) -> syn::Result<Self> { attr.parse_args() }
        }

        impl syn::parse::Parse for $i $(<$inner>)? {
            fn parse($input: syn::parse::ParseStream) -> syn::Result<Self> {
                let mut $out = Self::default();
                loop {
                    let span = $input.span();
                    let key: Ident = $input.call(syn::ext::IdentExt::parse_any)?;
                    match &*key.to_string() {
                        $($k => $e,)*
                        #[allow(unreachable_patterns)]
                        x => syn_err!(
                            span;
                            "Unknown attribute \"{x}\". Allowed attributes are: {}",
                            [$(stringify!($k),)*].join(", ")
                        )
                    }

                    match $input.is_empty() {
                        true => break,
                        false => {
                            $input.parse::<syn::Token![,]>()?;
                        }
                    }
                }

                Ok($out)
            }
        }
    };
}

/// Converts a rust identifier to a typescript identifier.
pub fn to_ts_ident(ident: &Ident) -> String {
    let ident = ident.to_string();
    if ident.starts_with("r#") {
        ident.trim_start_matches("r#").to_owned()
    } else {
        ident
    }
}

/// Convert an arbitrary name to a valid Typescript field name.
///
/// If the name contains special characters or if its first character
/// is a number it will be wrapped in quotes.
pub fn raw_name_to_ts_field(value: String) -> String {
    let valid_chars = value
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '$');

    let does_not_start_with_digit = value
        .chars()
        .next()
        .map_or(true, |first| !first.is_numeric());

    let valid = valid_chars && does_not_start_with_digit;

    if valid {
        value
    } else {
        format!(r#""{value}""#)
    }
}

/// Parse all `#[ts(..)]` attributes from the given slice.
pub fn parse_attrs<'a, A>(attrs: &'a [Attribute]) -> Result<A>
where
    A: TryFrom<&'a Attribute, Error = Error> + Attr,
{
    Ok(attrs
        .iter()
        .filter(|a| a.path().is_ident("ts"))
        .map(A::try_from)
        .collect::<Result<Vec<A>>>()?
        .into_iter()
        .fold(A::default(), |acc, cur| acc.merge(cur)))
}

/// Parse all `#[serde(..)]` attributes from the given slice.
#[cfg(feature = "serde-compat")]
#[allow(unused)]
pub fn parse_serde_attrs<'a, A>(attrs: &'a [Attribute]) -> Serde<A>
where
    A: Attr,
    Serde<A>: TryFrom<&'a Attribute, Error = Error>,
{
    use crate::attr::Serde;

    attrs
        .iter()
        .filter(|a| a.path().is_ident("serde"))
        .flat_map(|attr| match Serde::<A>::try_from(attr) {
            Ok(attr) => Some(attr),
            Err(_) => {
                use quote::ToTokens;

                warning::print_warning(
                    "failed to parse serde attribute",
                    format!("{}", attr.to_token_stream()),
                    "ts-gen failed to parse this attribute. It will be ignored.",
                )
                .unwrap();
                None
            }
        })
        .fold(Serde::<A>::default(), |acc, cur| acc.merge(cur))
}

/// Return doc comments parsed and formatted as JSDoc.
pub fn parse_docs(attrs: &[Attribute]) -> Result<String> {
    let lines = attrs
        .iter()
        .filter_map(|a| match a.meta {
            Meta::NameValue(ref x) if x.path.is_ident("doc") => Some(x),
            _ => None,
        })
        .map(|attr| match attr.value {
            Expr::Lit(ExprLit {
                lit: Lit::Str(ref str),
                ..
            }) => Ok(str.value()),
            _ => syn_err!(attr.span(); "doc attribute with non literal expression found"),
        })
        .map(|attr| {
            attr.map(|line| match line.trim() {
                "" => " *".to_owned(),
                _ => format!(" *{}", line.trim_end()),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(match lines.is_empty() {
        true => "".to_owned(),
        false => format!("/**\n{}\n */\n", lines.join("\n")),
    })
}

#[cfg(feature = "serde-compat")]
mod warning {
    use std::{fmt::Display, io::Write};

    use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};

    // Sadly, it is impossible to raise a warning in a proc macro.
    // This function prints a message which looks like a compiler warning.
    #[allow(unused)]
    pub fn print_warning(
        title: impl Display,
        content: impl Display,
        note: impl Display,
    ) -> std::io::Result<()> {
        let make_color = |color: Color, bold: bool| {
            let mut spec = ColorSpec::new();
            spec.set_fg(Some(color)).set_bold(bold).set_intense(true);
            spec
        };

        let yellow_bold = make_color(Color::Yellow, true);
        let white_bold = make_color(Color::White, true);
        let white = make_color(Color::White, false);
        let blue = make_color(Color::Blue, true);

        let writer = BufferWriter::stderr(ColorChoice::Auto);
        let mut buffer = writer.buffer();

        buffer.set_color(&yellow_bold)?;
        write!(&mut buffer, "warning")?;
        buffer.set_color(&white_bold)?;
        writeln!(&mut buffer, ": {}", title)?;

        buffer.set_color(&blue)?;
        writeln!(&mut buffer, "  | ")?;

        write!(&mut buffer, "  | ")?;
        buffer.set_color(&white)?;
        writeln!(&mut buffer, "{}", content)?;

        buffer.set_color(&blue)?;
        writeln!(&mut buffer, "  | ")?;

        write!(&mut buffer, "  = ")?;
        buffer.set_color(&white_bold)?;
        write!(&mut buffer, "note: ")?;
        buffer.set_color(&white)?;
        writeln!(&mut buffer, "{}", note)?;

        writer.print(&buffer)
    }
}

/// formats the generic arguments (like A, B in struct X<A, B>{..}) as "<X>" where x is a comma
/// seperated list of generic arguments, or an empty string if there are no type generics (lifetime/const generics are ignored).
/// this expands to an expression which evaluates to a `String`.
///
/// If a default type arg is encountered, it will be added to the dependencies.
pub fn format_generics(
    deps: &mut Dependencies,
    crate_rename: &Path,
    generics: &Generics,
) -> TokenStream {
    let mut expanded_params = generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(type_param) => {
                let ty = type_param.ident.to_string();
                if let Some(default) = &type_param.default {
                    deps.push(default);
                    Some(quote!(
                        format!("{} = {}", #ty, <#default as #crate_rename::TS>::name())
                    ))
                } else {
                    Some(quote!(#ty.to_owned()))
                }
            }
            _ => None,
        })
        .peekable();

    if expanded_params.peek().is_none() {
        return quote!("");
    }

    let comma_separated = quote!([#(#expanded_params),*].join(", "));
    quote!(format!("<{}>", #comma_separated))
}

pub fn get_traits_from_bounds(bounds: &Punctuated<TypeParamBound, Token![+]>) -> Vec<Ident> {
    let ignored_traits = vec![
        "Copy",
        "Clone",
        "Debug",
        "Hash",
        "Eq",
        "PartialEq",
        "Ord",
        "PartialOrd",
        "ToString",
        "TS",
    ];

    bounds
        .iter()
        .filter_map(|b| match b {
            TypeParamBound::Trait(t) => Some(t),
            _ => None,
        })
        .map(|b| {
            b.path
                .segments
                .iter()
                .map(|s| s.ident.clone())
                .filter(|i| !ignored_traits.iter().any(|it| i == it))
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect::<Vec<_>>()
}
