#![macro_use]

use std::{
    collections::{HashMap, HashSet},
    iter::FilterMap,
};

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    parse_quote, punctuated::Iter, spanned::Spanned, ConstParam, GenericParam, Generics, Item,
    LifetimeParam, Path, Result, Type, TypeArray, TypeParam, TypeParen, TypePath, TypeReference,
    TypeSlice, TypeTuple, WhereClause, WherePredicate,
};

use crate::utils::get_traits_from_bounds;
use crate::{deps::Dependencies, utils::format_generics};

#[macro_use]
mod utils;
mod attr;
mod deps;
mod types;

#[derive(Debug)]
struct DerivedTS {
    crate_rename: Path,
    ts_name: String,
    docs: String,
    inline: TokenStream,
    inline_flattened: Option<TokenStream>,
    dependencies: Dependencies,
    bound: Option<Vec<WherePredicate>>,

    export: bool,
    export_to: Option<String>,
}

impl DerivedTS {
    fn into_impl(mut self, rust_ty: Ident, generics: Generics) -> TokenStream {
        let allow_export = cfg!(feature = "export") && self.export;
        let export = allow_export.then(|| self.generate_export_test(&rust_ty, &generics));

        let output_path_fn = {
            let path = match self.export_to.as_deref() {
                Some(dirname) if dirname.ends_with('/') => {
                    format!("{}{}.ts", dirname, self.ts_name)
                }
                Some(filename) => filename.to_owned(),
                None => format!("{}.ts", self.ts_name),
            };

            quote! {
                fn output_path() -> Option<&'static std::path::Path> {
                    Some(std::path::Path::new(#path))
                }
            }
        };

        let docs = match &*self.docs {
            "" => None,
            docs => Some(quote!(const DOCS: Option<&'static str> = Some(#docs);)),
        };

        let crate_rename = self.crate_rename.clone();

        let ident = self.ts_name.clone();
        let impl_start = generate_impl_block_header(
            &crate_rename,
            &rust_ty,
            &generics,
            self.bound.as_deref(),
            &self.dependencies,
        );
        let name = self.generate_name_fn(&generics);
        let inline = self.generate_inline_fn();
        let decl = self.generate_decl_fn(&rust_ty, &generics);
        let dependencies = &self.dependencies;
        let generics_fn = self.generate_generics_fn(&generics);

        quote! {
            #impl_start {

                fn ident() -> String {
                    #ident.to_owned()
                }

                #docs
                #name
                #decl
                #inline
                #generics_fn
                #output_path_fn

                #[allow(clippy::unused_unit)]
                fn dependency_types() -> impl #crate_rename::typelist::TypeList
                where
                    Self: 'static,
                {
                    #dependencies
                }
            }

            #export
        }
    }

    /// Returns an expression which evaluates to the TypeScript name of the type, including generic
    /// parameters.
    fn name_with_generics(&self, generics: &Generics) -> TokenStream {
        let name = &self.ts_name;
        let crate_rename = &self.crate_rename;
        let mut generics_ts_names = generics
            .type_params()
            .map(|ty| &ty.ident)
            .map(|generic| quote!(<#generic as #crate_rename::TS>::name()))
            .peekable();

        if generics_ts_names.peek().is_some() {
            quote! {
                format!("{}<{}>", #name, vec![#(#generics_ts_names),*].join(", "))
            }
        } else {
            quote!(#name.to_owned())
        }
    }

    /// Generate a dummy unit struct for every generic type parameter of this type.
    /// # Example:
    /// ```compile_fail
    /// struct Generic<A, B, const C: usize> { /* ... */ }
    /// ```
    /// has two generic type parameters, `A` and `B`. This function will therefore generate
    /// ```compile_fail
    /// struct A;
    /// impl ts_gen::TS for A { /* .. */ }
    ///
    /// struct B;
    /// impl ts_gen::TS for B { /* .. */ }
    /// ```
    fn generate_generic_types(&self, generics: &Generics) -> TokenStream {
        let crate_rename = &self.crate_rename;

        let mut traits: HashMap<Ident, Vec<Ident>> = HashMap::new();

        let bounds = generics
            .params
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(TypeParam { ident, bounds, .. }) => Some((ident, bounds)),
                _ => None,
            })
            .map(|b| {
                let bounded_ty = b.0.clone();
                let bounds = get_traits_from_bounds(b.1);
                (bounded_ty, bounds)
            });

        traits.extend(bounds);

        if let Some(where_clause) = &generics.where_clause {
            traits.extend(
                where_clause
                    .predicates
                    .iter()
                    .filter_map(|p| match p {
                        WherePredicate::Type(t) => Some(t),
                        _ => None,
                    })
                    .map(|p| {
                        let bounded_ty = p.bounded_ty.clone();
                        let bounds = get_traits_from_bounds(&p.bounds);
                        (bounded_ty, bounds)
                    })
                    .filter_map(|a| match a.0 {
                        Type::Path(p) => {
                            Some((p.path.segments.first().unwrap().ident.clone(), a.1))
                        }
                        _ => None,
                    }),
            );
        }

        let generics = generics.type_params().map(|ty| ty.ident.clone());
        let name = quote![<Self as #crate_rename::TS>::name()];

        let mut results = vec![];
        for g in generics {
            let g_traits = traits.get(&g).cloned().unwrap_or_else(|| vec![]);
            let res = quote! {
                #[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd, #(#g_traits,)*)]
                struct #g;
                impl std::fmt::Display for #g {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, "{:?}", self)
                    }
                }
                impl #crate_rename::TS for #g {
                    fn name() -> String { stringify!(#g).to_owned() }
                    fn inline() -> String { panic!("{} cannot be inlined", #name) }
                    fn inline_flattened() -> String { panic!("{} cannot be flattened", #name) }
                    fn decl() -> String { panic!("{} cannot be declared", #name) }
                    fn decl_concrete() -> String { panic!("{} cannot be declared", #name) }
                }
            };
            results.push(res);
        }

        quote! {
            #(#results)*
        }
    }

    fn generate_export_test(&self, rust_ty: &Ident, generics: &Generics) -> TokenStream {
        let test_fn = format_ident!(
            "export_bindings_{}",
            rust_ty.to_string().to_lowercase().replace("r#", "")
        );
        let crate_rename = &self.crate_rename;

        let generic_types = self.generate_generic_types(generics);

        let generic_params = filter_generic_params(generics);

        let ty = quote!(<#rust_ty<#(#generic_params),*> as #crate_rename::TS>);

        quote! {
            #[cfg(test)]
            #[test]
            fn #test_fn() {
                #generic_types

                #ty::export_all().expect("could not export type");
            }
        }
    }

    fn generate_generics_fn(&self, generics: &Generics) -> TokenStream {
        let crate_rename = &self.crate_rename;
        let generics = generics
            .type_params()
            .map(|TypeParam { ident, .. }| quote![.push::<#ident>().extend(<#ident as #crate_rename::TS>::generics())]);
        quote! {
            #[allow(clippy::unused_unit)]
            fn generics() -> impl #crate_rename::typelist::TypeList
            where
                Self: 'static,
            {
                use #crate_rename::typelist::TypeList;
                ()#(#generics)*
            }
        }
    }

    fn generate_name_fn(&self, generics: &Generics) -> TokenStream {
        let name = self.name_with_generics(generics);
        quote! {
            fn name() -> String {
                #name
            }
        }
    }

    fn generate_inline_fn(&self) -> TokenStream {
        let inline = &self.inline;
        let crate_rename = &self.crate_rename;

        let inline_flattened = self.inline_flattened.as_ref().map_or_else(
            || {
                quote! {
                    fn inline_flattened() -> String {
                        panic!("{} cannot be flattened", <Self as #crate_rename::TS>::name())
                    }
                }
            },
            |inline_flattened| {
                quote! {
                    fn inline_flattened() -> String {
                        #inline_flattened
                    }
                }
            },
        );
        let inline = quote! {
            fn inline() -> String {
                #inline
            }
        };
        quote! {
            #inline
            #inline_flattened
        }
    }

    /// Generates the `decl()` and `decl_concrete()` methods.
    /// `decl_concrete()` is simple, and simply defers to `inline()`.
    /// For `decl()`, however, we need to change out the generic parameters of the type, replacing
    /// them with the dummy types generated by `generate_generic_types()`.
    fn generate_decl_fn(&mut self, rust_ty: &Ident, generics: &Generics) -> TokenStream {
        let name = &self.ts_name;
        let crate_rename = &self.crate_rename;
        let ts_generics = format_generics(&mut self.dependencies, crate_rename, generics);

        let generic_idents = filter_generic_params(&generics);

        quote! {
            fn decl_concrete() -> String {
                format!("type {} = {};", #name, <Self as #crate_rename::TS>::inline())
            }
            fn decl() -> String {
                let inline = <#rust_ty<#(#generic_idents,)*> as #crate_rename::TS>::inline();
                let generics = #ts_generics;
                format!("type {}{generics} = {inline};", #name)
            }
        }
    }
}

/// These are the generic parameters we'll be using.
fn filter_generic_params(
    generics: &Generics,
) -> FilterMap<Iter<GenericParam>, fn(&GenericParam) -> Option<TokenStream>> {
    generics.params.iter().filter_map(|p| match p {
        GenericParam::Lifetime(_) => None,
        GenericParam::Type(TypeParam { ident, .. }) => Some(quote!(#ident)),
        // We keep const parameters as they are, since there's no sensible default value we can
        // use instead. This might be something to change in the future.
        GenericParam::Const(ConstParam { ident, .. }) => Some(quote!(#ident)),
    })
}

// generate start of the `impl TS for #ty` block, up to (excluding) the open brace
fn generate_impl_block_header(
    crate_rename: &Path,
    ty: &Ident,
    generics: &Generics,
    bounds: Option<&[WherePredicate]>,
    dependencies: &Dependencies,
) -> TokenStream {
    let params = generics.params.iter().map(|param| match param {
        GenericParam::Type(TypeParam {
            ident,
            colon_token,
            bounds,
            ..
        }) => quote!(#ident #colon_token #bounds),
        GenericParam::Lifetime(LifetimeParam {
            lifetime,
            colon_token,
            bounds,
            ..
        }) => quote!(#lifetime #colon_token #bounds),
        GenericParam::Const(ConstParam {
            const_token,
            ident,
            colon_token,
            ty,
            ..
        }) => quote!(#const_token #ident #colon_token #ty),
    });
    let type_args = generics.params.iter().map(|param| match param {
        GenericParam::Type(TypeParam { ident, .. })
        | GenericParam::Const(ConstParam { ident, .. }) => quote!(#ident),
        GenericParam::Lifetime(LifetimeParam { lifetime, .. }) => quote!(#lifetime),
    });

    let where_bound = match bounds {
        Some(bounds) => quote! { where #(#bounds),* },
        None => {
            let bounds = generate_where_clause(crate_rename, generics, dependencies);
            quote! { #bounds }
        }
    };

    quote!(impl <#(#params),*> #crate_rename::TS for #ty <#(#type_args),*> #where_bound)
}

fn generate_where_clause(
    crate_rename: &Path,
    generics: &Generics,
    dependencies: &Dependencies,
) -> WhereClause {
    let used_types = {
        let is_type_param = |id: &Ident| generics.type_params().any(|p| &p.ident == id);

        let mut used_types = HashSet::new();
        for ty in dependencies.used_types() {
            used_type_params(&mut used_types, ty, is_type_param);
        }
        used_types.into_iter()
    };

    let existing = generics.where_clause.iter().flat_map(|w| &w.predicates);

    parse_quote! {
        where #(#existing,)* #(#used_types: #crate_rename::TS),*
    }
}

// Extracts all type parameters which are used within the given type.
// Associated types of a type parameter are extracted as well.
// Note: This will not extract `I` from `I::Item`, but just `I::Item`!
fn used_type_params<'ty, 'out>(
    out: &'out mut HashSet<&'ty Type>,
    ty: &'ty Type,
    is_type_param: impl Fn(&'ty Ident) -> bool + Copy + 'out,
) {
    use syn::{
        AngleBracketedGenericArguments as GenericArgs, GenericArgument as G, PathArguments as P,
    };

    match ty {
        Type::Array(TypeArray { elem, .. })
        | Type::Paren(TypeParen { elem, .. })
        | Type::Reference(TypeReference { elem, .. })
        | Type::Slice(TypeSlice { elem, .. }) => used_type_params(out, elem, is_type_param),
        Type::Tuple(TypeTuple { elems, .. }) => elems
            .iter()
            .for_each(|elem| used_type_params(out, elem, is_type_param)),
        Type::Path(TypePath { path, .. }) => {
            let first = path.segments.first().unwrap();
            if is_type_param(&first.ident) {
                // The type is either a generic parameter (e.g `T`), or an associated type of that
                // generic parameter (e.g `I::Item`). Either way, we return it.
                out.insert(ty);
                return;
            }

            let last = path.segments.last().unwrap();
            if let P::AngleBracketed(GenericArgs { ref args, .. }) = last.arguments {
                for generic in args {
                    if let G::Type(ty) = generic {
                        used_type_params(out, ty, is_type_param);
                    }
                }
            }
        }
        _ => (),
    }
}

/// Derives [TS](./trait.TS.html) for a struct or enum.
/// Please take a look at [TS](./trait.TS.html) for documentation.
#[proc_macro_derive(TS, attributes(ts))]
pub fn typescript(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    entry(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn entry(input: proc_macro::TokenStream) -> Result<TokenStream> {
    let input = syn::parse::<Item>(input)?;
    let (ts, ident, generics) = match input {
        Item::Struct(s) => (types::struct_def(&s)?, s.ident, s.generics),
        Item::Enum(e) => (types::enum_def(&e)?, e.ident, e.generics),
        _ => syn_err!(input.span(); "unsupported item"),
    };

    Ok(ts.into_impl(ident, generics))
}
