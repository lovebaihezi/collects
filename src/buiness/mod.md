% Rules of Adding functions that will interact with external resource

# Background

# Intro

Overview of the tasks of one GUI Application, despite the UI rendering, there can be such types of tasks:

+ Computing Tasks(heavy or light)

+ IO Tasks(including network IO), Async

+ Background Tasks(Scheduling, periodic tasks, etc.)

Why such three types of tasks?

Such types is build on top of the basic concept of the functional programming and the concept of the side effect.

Computing Tasks are pure functions, it will not change the state of the application state or system resource(ignore the memory); IO tasks means it will affect the system resource, and can not been undo, undo just means execute the opposite action; and Background Tasks, they based on times, OS times, specific duration after one event.


What about other logic? The task here means it will took a while to finish, one task that over 16.6 is taking so long, and as we can't render the UI within 1ms, so any action which is not O(1), it should been seen as one business, and must run in another thread.

# How UI thread communicate with business threads

Different business means different input and output, each business should follow such pattern:

```rust
// in file: read_local_file_to_string.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct Input {
    pub path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Output(String);
```

and in defines.rs, add

```rust
// in file: mod.rs
pub enum Business {
    // ...previous business
    ReadLocalFileToString(read_local_file_to_string::Input),
}

pub enum BusinessOutput {
    // ...previous business output
    ReadLocalFileToString(read_local_file_to_string::Output),
}
```

and for example impl, it could be

```rust

```
