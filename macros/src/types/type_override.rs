use quote::quote;
use syn::Result;

use crate::{
    attr::{ContainerAttr, EnumAttr, StructAttr},
    deps::Dependencies,
    DerivedTS,
};

pub(crate) fn type_override_struct(
    attr: &StructAttr,
    name: &str,
    type_override: &str,
) -> Result<DerivedTS> {
    let crate_rename = attr.crate_rename();

    Ok(DerivedTS {
        crate_rename: crate_rename.clone(),
        inline: quote!(#type_override.to_owned()),
        inline_flattened: None,
        docs: attr.docs.clone(),
        dependencies: Dependencies::new(crate_rename),
        export: attr.export,
        export_to: attr.export_to.clone(),
        ts_name: name.to_owned(),
        bound: attr.bound.clone(),
    })
}

pub(crate) fn type_override_enum(
    attr: &EnumAttr,
    name: &str,
    type_override: &str,
) -> Result<DerivedTS> {
    let crate_rename = attr.crate_rename();

    Ok(DerivedTS {
        crate_rename: crate_rename.clone(),
        inline: quote!(#type_override.to_owned()),
        inline_flattened: None,
        docs: attr.docs.clone(),
        dependencies: Dependencies::new(crate_rename),
        export: attr.export,
        export_to: attr.export_to.clone(),
        ts_name: name.to_owned(),
        bound: attr.bound.clone(),
    })
}
