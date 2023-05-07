use bevy::{
    app::AppExit,
    prelude::{App, ResMut},
    DefaultPlugins,
};
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(TokioTasksPlugin::default())
        .add_system(bevy::window::close_on_esc)
        .add_startup_system(demo)
        .run();
}

fn demo(runtime: ResMut<TokioTasksRuntime>) {
    runtime.spawn_background_task(|mut ctx| async move {
        println!("Task spawned on tick {}", ctx.current_tick());
        ctx.sleep_updates(120).await;
        println!("Task finished initial wait on tick {}", ctx.current_tick());
        ctx.run_on_main_thread(move |ctx| {
            println!(
                "Task going to request app exit on tick {}",
                ctx.current_tick
            );
            ctx.world.send_event(AppExit {});
        })
        .await;
    });
}
