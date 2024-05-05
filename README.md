# ts-gen

<h1 align="center" style="padding-top: 0; margin-top: 0;">
<br/>
ts-gen
</h1>
<p align="center">
Generate typescript type declarations from rust types
</p>

## Why?

When building a web application in rust, data structures have to be shared between backend and frontend.
Using this library, you can easily generate TypeScript bindings to your rust structs & enums so that you can keep your
types in one place.

ts-gen might also come in handy when working with webassembly.

```rust
use ts_gen::TS;

#[derive(TS)]
#[ts(export)]
struct User {
    user_id: i32,
    first_name: String,
    last_name: String,
}
```

When running `cargo test` or using CLI Tool, the TypeScript bindings will be exported to the file `bindings/User.ts`.

## Features

- generate type declarations from rust structs
- generate union declarations from rust enums
- inline types
- flatten structs/types
- generate necessary imports when exporting to multiple files
- serde compatibility
- generic types
- support for ESM imports

## cargo features

| **Feature**        | **Description**                                                                                                                                                                                           |
|:-------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| serde-compat       | **Enabled by default** <br/>See the *"serde compatibility"* section below for more information.                                                                                                           |
| format             | Enables formatting of the generated TypeScript bindings. <br/>Currently, this unfortunately adds quite a few dependencies.                                                                                |
| no-serde-warnings  | By default, warnings are printed during build if unsupported serde attributes are encountered. <br/>Enabling this feature silences these warnings.                                                        |
| import-esm         | When enabled,`import` statements in the generated file will have the `.js` extension in the end of the path to conform to the ES Modules spec. <br/> Example: `import { MyStruct } from "./my_struct.js"` |
| serde-json-impl    | Implement `TS` for types from *serde_json*                                                                                                                                                                |
| chrono-impl        | Implement `TS` for types from *chrono*                                                                                                                                                                    |
| bigdecimal-impl    | Implement `TS` for types from *bigdecimal*                                                                                                                                                                |
| url-impl           | Implement `TS` for types from *url*                                                                                                                                                                       |
| uuid-impl          | Implement `TS` for types from *uuid*                                                                                                                                                                      |
| bson-uuid-impl     | Implement `TS` for types from *bson*                                                                                                                                                                      |
| bytes-impl         | Implement `TS` for types from *bytes*                                                                                                                                                                     |
| indexmap-impl      | Implement `TS` for types from *indexmap*                                                                                                                                                                  |
| ordered-float-impl | Implement `TS` for types from *ordered_float*                                                                                                                                                             |
| heapless-impl      | Implement `TS` for types from *heapless*                                                                                                                                                                  |
| semver-impl        | Implement `TS` for types from *semver*                                                                                                                                                                    |

<br/>

If there's a type you're dealing with which doesn't implement `TS`, use either
`#[ts(as = "..")]` or `#[ts(type = "..")]`, or open a PR.

## `serde` compatability

With the `serde-compat` feature (enabled by default), serde attributes can be parsed for enums and structs.
Supported serde attributes:

- `rename`
- `rename-all`
- `rename-all-fields`
- `tag`
- `content`
- `untagged`
- `skip`
- `flatten`
- `default`

Note: `skip_serializing` and `skip_deserializing` are ignored. If you wish to exclude a field
from the generated type, but cannot use `#[serde(skip)]`, use `#[ts(skip)]` instead.

When ts-gen encounters an unsupported serde attribute, a warning is emitted, unless the feature `no-serde-warnings` is
enabled.

## MSRV

The Minimum Supported Rust Version for this crate is 1.75.0