//! <h1 align="center" style="padding-top: 0; margin-top: 0;">
//! <br/>
//! ts-gen
//! </h1>
//! <p align="center">
//! Generate typescript type declarations from rust types
//! </p>
//!
//! ## Why?
//! When building a web application in rust, data structures have to be shared between backend and frontend.
//! Using this library, you can easily generate TypeScript bindings to your rust structs & enums so that you can keep your
//! types in one place.
//!
//! ts-gen might also come in handy when working with webassembly.
//!
//! ```rust
//! use ts_gen::TS;
//!
//! #[derive(TS)]
//! #[ts(export)]
//! struct User {
//!     user_id: i32,
//!     first_name: String,
//!     last_name: String,
//! }
//! ```
//! When running `cargo test` or using CLI Tool, the TypeScript bindings will be exported to the file `bindings/User.ts`.
//!
//! ## Features
//! - generate type declarations from rust structs
//! - generate union declarations from rust enums
//! - inline types
//! - flatten structs/types
//! - generate necessary imports when exporting to multiple files
//! - serde compatibility
//! - generic types
//! - support for ESM imports
//!
//! ## cargo features
//! | **Feature**        | **Description**                                                                                                                                                                                           |
//! |:-------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
//! | serde-compat       | **Enabled by default** <br/>See the *"serde compatibility"* section below for more information.                                                                                                           |
//! | format             | Enables formatting of the generated TypeScript bindings. <br/>Currently, this unfortunately adds quite a few dependencies.                                                                                |
//! | no-serde-warnings  | By default, warnings are printed during build if unsupported serde attributes are encountered. <br/>Enabling this feature silences these warnings.                                                        |
//! | import-esm         | When enabled,`import` statements in the generated file will have the `.js` extension in the end of the path to conform to the ES Modules spec. <br/> Example: `import { MyStruct } from "./my_struct.js"` |
//! | serde-json-impl    | Implement `TS` for types from *serde_json*                                                                                                                                                                |
//! | chrono-impl        | Implement `TS` for types from *chrono*                                                                                                                                                                    |
//! | bigdecimal-impl    | Implement `TS` for types from *bigdecimal*                                                                                                                                                                |
//! | url-impl           | Implement `TS` for types from *url*                                                                                                                                                                       |
//! | uuid-impl          | Implement `TS` for types from *uuid*                                                                                                                                                                      |
//! | bson-uuid-impl     | Implement `TS` for types from *bson*                                                                                                                                                                      |
//! | bytes-impl         | Implement `TS` for types from *bytes*                                                                                                                                                                     |
//! | indexmap-impl      | Implement `TS` for types from *indexmap*                                                                                                                                                                  |
//! | ordered-float-impl | Implement `TS` for types from *ordered_float*                                                                                                                                                             |
//! | heapless-impl      | Implement `TS` for types from *heapless*                                                                                                                                                                  |
//! | semver-impl        | Implement `TS` for types from *semver*                                                                                                                                                                    |
//!
//! <br/>
//!
//! If there's a type you're dealing with which doesn't implement `TS`, use either
//! `#[ts(as = "..")]` or `#[ts(type = "..")]`, or open a PR.
//!
//! ## `serde` compatability
//! With the `serde-compat` feature (enabled by default), serde attributes can be parsed for enums and structs.
//! Supported serde attributes:
//! - `rename`
//! - `rename-all`
//! - `rename-all-fields`
//! - `tag`
//! - `content`
//! - `untagged`
//! - `skip`
//! - `flatten`
//! - `default`
//!
//! Note: `skip_serializing` and `skip_deserializing` are ignored. If you wish to exclude a field
//! from the generated type, but cannot use `#[serde(skip)]`, use `#[ts(skip)]` instead.
//!
//! When ts-gen encounters an unsupported serde attribute, a warning is emitted, unless the feature `no-serde-warnings` is enabled.

use std::{
    any::TypeId,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize, NonZeroU128,
        NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize,
    },
    ops::{Range, RangeInclusive},
    path::{Path, PathBuf},
};

#[cfg(feature = "chrono-impl")]
mod chrono;
pub mod error;
mod export;
#[cfg(feature = "serde-json-impl")]
mod serde_json;
pub mod typelist;

pub use ts_gen_macros::TS;

use error::{Error, Result};
use typelist::TypeList;

/// A typescript type which is depended upon by other types.
/// This information is required for generating the correct import statements.
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Dependency {
    /// Type ID of the rust type
    pub type_id: TypeId,
    /// Name of the type in TypeScript
    pub ts_name: String,
    /// Path to where the type would be exported. By default, a filename is derived from the types
    /// name, which can be customized with `#[ts(export_to = "..")]`.
    /// This path does _not_ include a base directory.
    pub output_path: &'static Path,
}

impl Dependency {
    /// Constructs a [`Dependency`] from the given type `T`.
    /// If `T` is not exportable (meaning `T::EXPORT_TO` is `None`), this function will return `None`
    pub fn from_ty<T: TS + 'static + ?Sized>() -> Option<Self> {
        let output_path = T::output_path()?;
        Some(Dependency {
            type_id: TypeId::of::<T>(),
            ts_name: T::ident(),
            output_path,
        })
    }
}

/// A type which can be represented in TypeScript.
/// Most of the time, you'd want to derive this trait instead of implementing it manually.
/// ts-gen comes with implementations for all primitives, most collections, tuples,
/// arrays and containers.
///
/// ### exporting
/// Because Rusts procedural macros are evaluated before other compilation steps, TypeScript
/// bindings __cannot__ be exported during compile time.
///
/// Bindings can be exported within a test, which ts-gen generates for you by adding `#[ts(export)]`
/// to a type you wish to export to a file.
/// When `cargo test` is run, all types annotated with `#[ts(export)]` and all of their
/// dependencies will be written to `TS_GEN_EXPORT_DIR`, or `./bindings` by default.
/// But for using tests you must add 'export' feature to your Cargo.toml file.
/// You can also use the CLI Tool provided by this crate to export your types automatically.
/// To explore CLI usage you can run `cargo ts-gen --help` command.
///
/// For each individual type, path and filename within the output directory can be changed using
/// `#[ts(export_to = "...")]`. By default, the filename will be derived from the name of the type.
///
/// If, for some reason, you need to do this during runtime or cannot use `#[ts(export)]`, bindings
/// can be exported manually:
///
/// | Function              | Includes Dependencies | To                 |
/// |-----------------------|-----------------------|--------------------|
/// | [`TS::export`]        | ❌                    | `TS_GEN_EXPORT_DIR` |
/// | [`TS::export_all`]    | ✔️                    | `TS_GEN_EXPORT_DIR` |
/// | [`TS::export_all_to`] | ✔️                    | _custom_           |
///
/// ### serde compatibility
/// By default, the feature `serde-compat` is enabled.
/// ts-gen then parses serde attributes and adjusts the generated typescript bindings accordingly.
/// Not all serde attributes are supported yet - if you use an unsupported attribute, you'll see a
/// warning.
///
/// ### container attributes
/// attributes applicable for both structs and enums
///
/// - **`#[ts(crate = "..")]`**
///   Generates code which references the module passed to it instead of defaulting to `::ts_gen`
///   This is useful for cases where you have to re-export the crate.
///
/// - **`#[ts(export)]`**
///   Generates a test which will export the type, by default to `bindings/<name>.ts` when running
///   `cargo test`. The default base directory can be overridden with the `TS_GEN_EXPORT_DIR` environment variable.
///   Adding the variable to a project's [config.toml](https://doc.rust-lang.org/cargo/reference/config.html#env) can
///   make it easier to manage.
///   ```toml
///   # <project-root>/.cargo/config.toml
///   [env]
///   TS_GEN_EXPORT_DIR = { value = "<OVERRIDE_DIR>", relative = true }
///   ```
///   <br/>
///
/// - **`#[ts(export_to = "..")]`**
///   Specifies where the type should be exported to. Defaults to `<name>.ts`.
///   The path given to the `export_to` attribute is relative to the `TS_GEN_EXPORT_DIR` environment variable,
///   or, if `TS_GEN_EXPORT_DIR` is not set, to `./bindings`
///   If the provided path ends in a trailing `/`, it is interpreted as a directory.
///   Note that you need to add the `export` attribute as well, in order to generate a test which exports the type.
///   <br/><br/>
///
/// - **`#[ts(as = "..")]`**
///   Overrides the type used in Typescript, using the provided Rust type instead.
///   This is useful when you have a custom serializer and deserializer and don't want to implement `TS` manually
///   <br/><br/>
///
/// - **`#[ts(type = "..")]`**
///   Overrides the type used in TypeScript.
///   This is useful when you have a custom serializer and deserializer and don't want to implement `TS` manually
///   <br/><br/>
///
/// - **`#[ts(rename = "..")]`**
///   Sets the typescript name of the generated type
///   <br/><br/>
///
/// - **`#[ts(rename_all = "..")]`**
///   Rename all fields/variants of the type.
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
///
/// ### struct attributes
/// - **`#[ts(tag = "..")]`**
///   Include the structs name (or value of `#[ts(rename = "..")]`) as a field with the given key.
///   <br/><br/>
///
/// ### struct field attributes
/// - **`#[ts(type = "..")]`**
///   Overrides the type used in TypeScript.
///   This is useful when there's a type for which you cannot derive `TS`.
///   <br/><br/>
///
/// - **`#[ts(as = "..")]`**
///   Overrides the type of the annotated field, using the provided Rust type instead.
///   This is useful when there's a type for which you cannot derive `TS`.
///   `_` may be used to refer to the type of the field, e.g `#[ts(as = "Option<_>")]`.
///   <br/><br/>
///
/// - **`#[ts(rename = "..")]`**
///   Renames this field. To rename all fields of a struct, see the container attribute `#[ts(rename_all = "..")]`.
///   <br/><br/>
///
/// - **`#[ts(inline)]`**
///   Inlines the type of this field, replacing its name with its definition.
///   <br/><br/>
///
/// - **`#[ts(skip)]`**
///   Skips this field, omitting it from the generated *TypeScript* type.
///   <br/><br/>
///
/// - **`#[ts(optional)]`**
///   May be applied on a struct field of type `Option<T>`. By default, such a field would turn into `t: T | null`.
///   If `#[ts(optional)]` is present, `t?: T` is generated instead.
///   If `#[ts(optional = nullable)]` is present, `t?: T | null` is generated.
///   <br/><br/>
///
/// - **`#[ts(flatten)]`**
///   Flatten this field, inlining all the keys of the field's type into its parent.
///   <br/><br/>
///
/// ### enum attributes
/// - **`#[ts(tag = "..")]`**
///   Changes the representation of the enum to store its tag in a separate field.
///   See [the serde docs](https://serde.rs/enum-representations.html) for more information.
///   <br/><br/>
///
/// - **`#[ts(content = "..")]`**
///   Changes the representation of the enum to store its content in a separate field.
///   See [the serde docs](https://serde.rs/enum-representations.html) for more information.
///   <br/><br/>
///
/// - **`#[ts(untagged)]`**
///   Changes the representation of the enum to not include its tag.
///   See [the serde docs](https://serde.rs/enum-representations.html) for more information.
///   <br/><br/>
///
/// - **`#[ts(rename_all = "..")]`**
///   Rename all variants of this enum.
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
///
/// - **`#[ts(rename_all_fieds = "..")]`**
///   Renames the fields of all the struct variants of this enum. This is equivalent to using
///   `#[ts(rename_all = "..")]` on all of the enum's variants.
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
///
/// ### enum variant attributes
/// - **`#[ts(rename = "..")]`**
///   Renames this variant. To rename all variants of an enum, see the container attribute `#[ts(rename_all = "..")]`.
///   <br/><br/>
///
/// - **`#[ts(skip)]`**
///   Skip this variant, omitting it from the generated *TypeScript* type.
///   <br/><br/>
///
/// - **`#[ts(untagged)]`**
///   Changes this variant to be treated as if the enum was untagged, regardless of the enum's tag
///   and content attributes
///   <br/><br/>
///
/// - **`#[ts(rename_all = "..")]`**
///   Renames all the fields of a struct variant.
///   Valid values are `lowercase`, `UPPERCASE`, `camelCase`, `snake_case`, `PascalCase`, `SCREAMING_SNAKE_CASE`, "kebab-case" and "SCREAMING-KEBAB-CASE"
///   <br/><br/>
pub trait TS {
    /// JSDoc comment to describe this type in TypeScript - when `TS` is derived, docs are
    /// automatically read from your doc comments or `#[doc = ".."]` attributes
    const DOCS: Option<&'static str> = None;

    /// Name of this type in TypeScript, including generic parameters
    fn name() -> String;

    /// Identifier of this type, excluding generic parameters.
    fn ident() -> String {
        // by default, fall back to `TS::name()`.
        let name = Self::name();

        match name.find('<') {
            None => name,
            Some(i) => name[..i].to_owned(),
        }
    }

    /// Declaration of this type, e.g. `type User = { user_id: number, ... }`.
    /// This function will panic if the type has no declaration.
    ///
    /// If this type is generic, then all provided generic parameters will be swapped for
    /// placeholders, resulting in a generic typescript definition.
    /// Both `SomeType::<i32>::decl()` and `SomeType::<String>::decl()` will therefore result in
    /// the same TypeScript declaration `type SomeType<A> = ...`.
    fn decl() -> String;

    /// Declaration of this type using the supplied generic arguments.
    /// The resulting TypeScript definition will not be generic. For that, see `TS::decl()`.
    /// If this type is not generic, then this function is equivalent to `TS::decl()`.
    fn decl_concrete() -> String;

    /// Formats this types definition in TypeScript, e.g `{ user_id: number }`.
    /// This function will panic if the type cannot be inlined.
    fn inline() -> String;

    /// Flatten a type declaration.
    /// This function will panic if the type cannot be flattened.
    fn inline_flattened() -> String;

    /// Returns a [`TypeList`] of all types on which this type depends.
    fn dependency_types() -> impl TypeList
    where
        Self: 'static,
    {
    }

    /// Returns a [`TypeList`] containing all generic parameters of this type.
    /// If this type is not generic, this will return an empty [`TypeList`].
    fn generics() -> impl TypeList
    where
        Self: 'static,
    {
    }

    // Resolves all dependencies of this type recursively.
    fn dependencies() -> Vec<Dependency>
    where
        Self: 'static,
    {
        use crate::typelist::TypeVisitor;

        struct Visit<'a>(&'a mut Vec<Dependency>);
        impl<'a> TypeVisitor for Visit<'a> {
            fn visit<T: TS + 'static + ?Sized>(&mut self) {
                if let Some(dep) = Dependency::from_ty::<T>() {
                    self.0.push(dep);
                }
            }
        }

        let mut deps: Vec<Dependency> = vec![];
        Self::dependency_types().for_each(&mut Visit(&mut deps));
        deps
    }

    /// Manually export this type to the filesystem.
    /// To export this type together with all of its dependencies, use [`TS::export_all`].
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.
    /// In that case, there is no need to manually call this function.
    ///
    /// # Target Directory
    /// The target directory to which the type will be exported may be changed by setting the
    /// `TS_GEN_EXPORT_DIR` environment variable. By default, `./bindings` will be used.
    ///
    /// To specify a target directory manually, use [`TS::export_all_to`], which also exports all
    /// dependencies.
    ///
    /// To alter the filename or path of the type within the target directory,
    /// use `#[ts(export_to = "...")]`.
    fn export() -> Result<()>
    where
        Self: 'static,
    {
        let path = Self::default_output_path()
            .ok_or_else(std::any::type_name::<Self>)
            .map_err(Error::CannotBeExported)?;

        export::export_to::<Self, _>(path)
    }

    /// Manually export this type to the filesystem, together with all of its dependencies.
    /// To export only this type, without its dependencies, use [`TS::export`].
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.
    /// In that case, there is no need to manually call this function.
    ///
    /// # Target Directory
    /// The target directory to which the types will be exported may be changed by setting the
    /// `TS_GEN_EXPORT_DIR` environment variable. By default, `./bindings` will be used.
    ///
    /// To specify a target directory manually, use [`TS::export_all_to`].
    ///
    /// To alter the filenames or paths of the types within the target directory,
    /// use `#[ts(export_to = "...")]`.
    fn export_all() -> Result<()>
    where
        Self: 'static,
    {
        export::export_all_into::<Self>(&*export::default_out_dir())
    }

    /// Manually export this type into the given directory, together with all of its dependencies.
    /// To export only this type, without its dependencies, use [`TS::export`].
    ///
    /// Unlike [`TS::export_all`], this function disregards `TS_GEN_EXPORT_DIR`, using the provided
    /// directory instead.
    ///
    /// To alter the filenames or paths of the types within the target directory,
    /// use `#[ts(export_to = "...")]`.
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.
    /// In that case, there is no need to manually call this function.
    fn export_all_to(out_dir: impl AsRef<Path>) -> Result<()>
    where
        Self: 'static,
    {
        export::export_all_into::<Self>(out_dir)
    }

    /// Manually generate bindings for this type, returning a [`String`].
    /// This function does not format the output, even if the `format` feature is enabled. TODO
    ///
    /// # Automatic Exporting
    /// Types annotated with `#[ts(export)]`, together with all of their dependencies, will be
    /// exported automatically whenever `cargo test` is run.
    /// In that case, there is no need to manually call this function.
    fn export_to_string() -> Result<String>
    where
        Self: 'static,
    {
        export::export_to_string::<Self>()
    }

    // Returns the output path to where `T` should be exported.
    /// The returned path does _not_ include the base directory from `TS_GEN_EXPORT_DIR`.
    ///
    /// To get the output path containing `TS_GEN_EXPORT_DIR`, use [`TS::default_output_path`].
    ///
    /// When deriving `TS`, the output path can be altered using `#[ts(export_to = "...")]`.
    /// See the documentation of [`TS`] for more details.
    ///
    /// The output of this function depends on the environment variable `TS_GEN_EXPORT_DIR`, which is
    /// used as base directory. If it is not set, `./bindings` is used as default directory.
    ///
    /// If `T` cannot be exported (e.g. because it's a primitive type), this function will return
    /// `None`.
    fn output_path() -> Option<&'static Path> {
        None
    }

    /// Returns the output path to where `T` should be exported.
    ///
    /// The output of this function depends on the environment variable `TS_GEN_EXPORT_DIR`, which is
    /// used as base directory. If it is not set, `./bindings` is used as default directory.
    ///
    /// To get the output path relative to `TS_GEN_EXPORT_DIR` and without reading the environment
    /// variable, use [`TS::output_path`].
    ///
    /// When deriving `TS`, the output path can be altered using `#[ts(export_to = "...")]`.
    /// See the documentation of [`TS`] for more details.
    ///
    /// If `T` cannot be exported (e.g. because it's a primitive type), this function will return
    /// `None`.
    fn default_output_path() -> Option<PathBuf> {
        Some(export::default_out_dir().join(Self::output_path()?))
    }
}

// generate impls for primitive types
macro_rules! impl_primitives {
    ($($($ty:ty),* => $l:literal),*) => { $($(
        impl TS for $ty {
            fn name() -> String { $l.to_owned() }
            fn decl() -> String { panic!("{} cannot be declared", <Self as $crate::TS>::name()) }
            fn decl_concrete() -> String { panic!("{} cannot be declared", <Self as $crate::TS>::name()) }
            fn inline() -> String { <Self as $crate::TS>::name() }
            fn inline_flattened() -> String { panic!("{} cannot be flattened", <Self as $crate::TS>::name()) }
        }
    )*)* };
}

// generate impls for tuples
macro_rules! impl_tuples {
    ( impl $($i:ident),* ) => {
        impl<$($i: TS),*> TS for ($($i,)*) {
            fn name() -> String {
                format!("[{}]", [$($i::name()),*].join(", "))
            }
            fn decl() -> String { panic!("tuple cannot be declared") }
            fn decl_concrete() -> String { panic!("tuple cannot be declared") }
            fn inline() -> String {
                panic!("tuple cannot be inlined!");
            }
            fn inline_flattened() -> String { panic!("tuple cannot be flattened") }
            fn dependency_types() -> impl TypeList
            where
                Self: 'static
            {
                ()$(.push::<$i>())*
            }
        }
    };
    ( $i2:ident $(, $i:ident)* ) => {
        impl_tuples!(impl $i2 $(, $i)* );
        impl_tuples!($($i),*);
    };
    () => {};
}

// generate impls for wrapper types
macro_rules! impl_wrapper {
    ($($t:tt)*) => {
        $($t)* {
            fn name() -> String { T::name() }
            fn decl() -> String { panic!("wrapper type cannot be declared") }
            fn decl_concrete() -> String { panic!("wrapper type cannot be declared") }
            fn inline() -> String { T::inline() }
            fn inline_flattened() -> String { T::inline_flattened() }
            fn dependency_types() -> impl $crate::typelist::TypeList
            where
                Self: 'static
            {
                T::dependency_types()
            }
            fn generics() -> impl $crate::typelist::TypeList
            where
                Self: 'static
            {
                ((std::marker::PhantomData::<T>,), T::generics())
            }
        }
    };
}

// implement TS for the $shadow, deferring to the impl $s
macro_rules! impl_shadow {
    (as $s:ty: $($impl:tt)*) => {
        $($impl)* {
            fn name() -> String { <$s>::name() }
            fn ident() -> String { <$s>::ident() }
            fn decl() -> String { <$s>::decl() }
            fn decl_concrete() -> String { <$s>::decl_concrete() }
            fn inline() -> String { <$s>::inline() }
            fn inline_flattened() -> String { <$s>::inline_flattened() }
            fn dependency_types() -> impl $crate::typelist::TypeList
            where
                Self: 'static
            {
                <$s>::dependency_types()
            }
            fn generics() -> impl $crate::typelist::TypeList
            where
                Self: 'static
            {
                <$s>::generics()
            }
            fn output_path() -> Option<&'static std::path::Path> { <$s>::output_path() }
        }
    };
}

impl<T: TS> TS for Option<T> {
    fn name() -> String {
        format!("{} | null", T::name()) // TODO: null will be replaced in interface field with `field`?: `type`
    }
    fn decl() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn inline() -> String {
        format!("{} | null", T::inline()) // TODO: null will be replaced in interface field with `field`?: `type`
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", Self::name())
    }

    fn dependency_types() -> impl TypeList
    where
        Self: 'static,
    {
        T::dependency_types()
    }

    fn generics() -> impl TypeList
    where
        Self: 'static,
    {
        T::generics().push::<T>()
    }
}

impl<T: TS, E: TS> TS for std::result::Result<T, E> {
    fn name() -> String {
        format!("{{ Ok : {} }} | {{ Err : {} }}", T::name(), E::name())
    }
    fn decl() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn inline() -> String {
        format!("{{ Ok : {} }} | {{ Err : {} }}", T::inline(), E::inline())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", Self::name())
    }

    fn dependency_types() -> impl TypeList
    where
        Self: 'static,
    {
        T::dependency_types().extend(E::dependency_types())
    }

    fn generics() -> impl TypeList
    where
        Self: 'static,
    {
        T::generics().push::<T>().extend(E::generics()).push::<E>()
    }
}

impl<T: TS> TS for Vec<T> {
    fn name() -> String {
        format!("Array<{}>", T::name())
    }

    fn ident() -> String {
        "Array".to_owned()
    }

    fn decl() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn inline() -> String {
        format!("Array<{}>", T::inline())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", Self::name())
    }

    fn dependency_types() -> impl TypeList
    where
        Self: 'static,
    {
        T::dependency_types()
    }

    fn generics() -> impl TypeList
    where
        Self: 'static,
    {
        T::generics().push::<T>()
    }
}

// Arrays longer than this limit will be emitted as Array<T>
const ARRAY_TUPLE_LIMIT: usize = 64;
impl<T: TS, const N: usize> TS for [T; N] {
    fn name() -> String {
        if N > ARRAY_TUPLE_LIMIT {
            return Vec::<T>::name();
        }

        format!(
            "[{}]",
            (0..N).map(|_| T::name()).collect::<Box<[_]>>().join(", ")
        )
    }

    fn decl() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn inline() -> String {
        if N > ARRAY_TUPLE_LIMIT {
            return Vec::<T>::inline();
        }

        format!(
            "[{}]",
            (0..N).map(|_| T::inline()).collect::<Box<[_]>>().join(", ")
        )
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", Self::name())
    }

    fn dependency_types() -> impl TypeList
    where
        Self: 'static,
    {
        T::dependency_types()
    }

    fn generics() -> impl TypeList
    where
        Self: 'static,
    {
        T::generics().push::<T>()
    }
}

impl<K: TS, V: TS, S> TS for HashMap<K, V, S> {
    fn name() -> String {
        format!("{{ [key: {}]: {} }}", K::name(), V::name())
    }

    fn ident() -> String {
        panic!()
    }

    fn decl() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn inline() -> String {
        format!("{{ [key: {}]: {} }}", K::inline(), V::inline())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", Self::name())
    }

    fn dependency_types() -> impl TypeList
    where
        Self: 'static,
    {
        K::dependency_types().extend(V::dependency_types())
    }

    fn generics() -> impl TypeList
    where
        Self: 'static,
    {
        K::generics().push::<K>().extend(V::generics()).push::<V>()
    }
}

impl<I: TS> TS for Range<I> {
    fn name() -> String {
        format!("{{ start: {}, end: {}, }}", I::name(), I::name())
    }

    fn decl() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn decl_concrete() -> String {
        panic!("{} cannot be declared", Self::name())
    }

    fn inline() -> String {
        panic!("{} cannot be inlined", Self::name())
    }

    fn inline_flattened() -> String {
        panic!("{} cannot be flattened", Self::name())
    }

    fn dependency_types() -> impl TypeList
    where
        Self: 'static,
    {
        I::dependency_types()
    }

    fn generics() -> impl TypeList
    where
        Self: 'static,
    {
        I::generics().push::<I>()
    }
}

impl_shadow!(as Range<I>: impl<I: TS> TS for RangeInclusive<I>);
impl_shadow!(as Vec<T>: impl<T: TS, H> TS for HashSet<T, H>);
impl_shadow!(as Vec<T>: impl<T: TS> TS for BTreeSet<T>);
impl_shadow!(as HashMap<K, V>: impl<K: TS, V: TS> TS for BTreeMap<K, V>);
impl_shadow!(as Vec<T>: impl<T: TS> TS for [T]);

impl_wrapper!(impl<T: TS + ?Sized> TS for &T);
impl_wrapper!(impl<T: TS + ?Sized> TS for Box<T>);
impl_wrapper!(impl<T: TS + ?Sized> TS for std::sync::Arc<T>);
impl_wrapper!(impl<T: TS + ?Sized> TS for std::rc::Rc<T>);
impl_wrapper!(impl<'a, T: TS + ToOwned + ?Sized> TS for std::borrow::Cow<'a, T>);
impl_wrapper!(impl<T: TS> TS for std::cell::Cell<T>);
impl_wrapper!(impl<T: TS> TS for std::cell::RefCell<T>);
impl_wrapper!(impl<T: TS> TS for std::sync::Mutex<T>);
impl_wrapper!(impl<T: TS + ?Sized> TS for std::sync::Weak<T>);
impl_wrapper!(impl<T: TS> TS for std::marker::PhantomData<T>);

impl_tuples!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);

impl_primitives! {
    u8, i8, NonZeroU8, NonZeroI8,
    u16, i16, NonZeroU16, NonZeroI16,
    u32, i32, NonZeroU32, NonZeroI32,
    usize, isize, NonZeroUsize, NonZeroIsize, f32, f64 => "number",
    u64, i64, NonZeroU64, NonZeroI64,
    u128, i128, NonZeroU128, NonZeroI128 => "bigint",
    bool => "boolean",
    char, Path, PathBuf, String, str,
    Ipv4Addr, Ipv6Addr, IpAddr, SocketAddrV4, SocketAddrV6, SocketAddr => "string",
    () => "null"
}

#[cfg(feature = "bigdecimal-impl")]
impl_primitives! { bigdecimal::BigDecimal => "string" }

#[cfg(feature = "uuid-impl")]
impl_primitives! { uuid::Uuid => "string" }

#[cfg(feature = "url-impl")]
impl_primitives! { url::Url => "string" }

#[cfg(feature = "ordered-float-impl")]
impl_primitives! { ordered_float::OrderedFloat<f32> => "number" }

#[cfg(feature = "ordered-float-impl")]
impl_primitives! { ordered_float::OrderedFloat<f64> => "number" }

#[cfg(feature = "bson-uuid-impl")]
impl_primitives! { bson::Uuid => "string" }

#[cfg(feature = "indexmap-impl")]
impl_shadow!(as Vec<T>: impl<T: TS> TS for indexmap::IndexSet<T>);

#[cfg(feature = "indexmap-impl")]
impl_shadow!(as HashMap<K, V>: impl<K: TS, V: TS> TS for indexmap::IndexMap<K, V>);

#[cfg(feature = "heapless-impl")]
impl_shadow!(as Vec<T>: impl<T: TS, const N: usize> TS for heapless::Vec<T, N>);

#[cfg(feature = "semver-impl")]
impl_primitives! { semver::Version => "string" }

#[cfg(feature = "bytes-impl")]
mod bytes {
    use super::TS;

    impl_shadow!(as Vec<u8>: impl TS for bytes::Bytes);
    impl_shadow!(as Vec<u8>: impl TS for bytes::BytesMut);
}

#[rustfmt::skip]
pub(crate) use impl_primitives;
#[rustfmt::skip]
pub(crate) use impl_shadow;
