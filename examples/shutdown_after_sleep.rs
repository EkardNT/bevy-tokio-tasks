use bevy::app::AppExit;
use bevy::prelude::{App, DefaultPlugins, ResMut, Update};

use bevy_app::Startup;
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TokioTasksPlugin::default())
        .add_systems(Update, bevy::window::close_on_esc)
        .add_systems(Startup, demo)
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
