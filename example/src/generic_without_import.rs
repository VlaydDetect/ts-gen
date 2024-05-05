#![allow(dead_code)]

#[derive(ts_gen::TS)]
struct Test<T> {
    field: T,
}