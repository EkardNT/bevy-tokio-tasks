# bevy-tokio-tasks

A simple Bevy plugin which integrates a Tokio runtime into a Bevy app.

[![crates.io](https://img.shields.io/crates/v/bevy-tokio-tasks)](https://crates.io/crates/bevy-tokio-tasks) [![docs.rs](https://img.shields.io/docsrs/bevy-tokio-tasks)](https://docs.rs/bevy-tokio-tasks/latest/bevy_tokio_tasks/)

## How To

### How to initialize this plugin

To initialize the plugin, simply install the `TokioTasksPlugin` when initializing your Bevy app. The simplest
way to do this is to use the `TokioTasksPlugin::default()` method, which sets up a Tokio `Runtime` with default
settings (the plugin calls `Runtime::enable_all` to enable Tokio's IO and timing functionality).

```rust
fn main() {
    bevy::App::new()
        .add_plugins(bevy_tokio_tasks::TokioTasksPlugin::default())
}
```

If you want to customize the Tokio `Runtime` setup, you may do so by specifying a `make_runtime` callback on the `TokioTasksPlugin`.

```rust
fn main() {
    bevy::App::new()
        .add_plugins(bevy_tokio_tasks::TokioTasksPlugin {
            make_runtime: Box::new(|| {
                let mut runtime = tokio::runtime::Builder::new_multi_thread();
                runtime.enable_all();
                runtime.build().unwrap()
            }),
            ..bevy_tokio_tasks::TokioTasksPlugin::default()
        })
}
```

### How to spawn a background task

To spawn a background task from a Bevy system function, add a `TokioTasksRuntime` as a resource parameter and call
the `spawn_background_task` function.

```rust
fn example_system(runtime: ResMut<TokioTasksRuntime>) {
    runtime.spawn_background_task(|_ctx| async move {
        println!("This task is running on a background thread");
    });
}
```

### How to synchronize with the main thread

Often times, background tasks will need to synchronize with the main Bevy app at certain points. You may do this
by calling the `run_on_main_thread` function on the `TaskContext` that is passed to each background task.

```rust
fn example_system(runtime: ResMut<TokioTasksRuntime>) {
    runtime.spawn_background_task(|mut ctx| async move {
        println!("This print executes from a background Tokio runtime thread");
        ctx.run_on_main_thread(move |ctx| {
            // The inner context gives access to a mutable Bevy World reference.
            let _world: &mut World = ctx.world;
        }).await;
    });
}
```

## Examples

- [change_clear_color](examples/change_clear_color.rs) - This example spawns a background task which
runs forever. Every second, the background task updates the app's background clear color. This demonstrates
how background tasks can synchronize with the main thread to update game state.
- [current_thread_runtime](examples/current_thread_runtime.rs) - This
example demonstrates how you can customize the Tokio Runtime. It configures a
current_thread Runtime instead of a multi-threading Runtime.
- [async_fn](examples/async_fn.rs) - Does the same thing as the change_clear_color example,
except that it shows how you can pass an `async fn` to `spawn_background_task`.
- [shutdown_after_sleep](examples/shutdown_after_sleep.rs) - This example spawns a background task which
sleeps for 120 Bevy game updates, then shuts down the Bevy app.

## Version Compatibility

This crate's major and minor version numbers will match Bevy's. To allow this crate to publish updates
between Bevy updates, the patch version is allowed to increment independent of Bevy's release cycle.

| bevy-tokio-tasks version | bevy version | tokio version |
|--------------------------|--------------|---------------|
| 0.13.0                   | 0.13.0       | 1             |
| 0.12.0                   | 0.12.0       | 1             |
| 0.11.0                   | 0.11.0       | 1             |
| 0.10.2                   | 0.10.1       | 1             |
| 0.10.1                   | 0.10.0       | 1             |
| 0.10.0                   | 0.10.0       | 1             |
| 0.9.5                    | 0.9.1        | 1             |
| 0.9.4                    | 0.9.1        | 1             |
| 0.9.3                    | 0.9.1        | 1             |
| 0.9.2                    | 0.9.1        | 1             |
| 0.9.1                    | 0.9.1        | 1             |
| 0.9.0                    | 0.9.1        | 1             |
