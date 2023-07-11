use std::time::Duration;

use bevy::{prelude::{App, ResMut, ClearColor, Color, Commands, Camera2dBundle, Update}, DefaultPlugins};
use bevy_tokio_tasks::{TokioTasksPlugin, TokioTasksRuntime};

static COLORS: [Color; 5] = [
    Color::RED,
    Color::GREEN,
    Color::BLUE,
    Color::WHITE,
    Color::BLACK,
];

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(TokioTasksPlugin::default())
        .add_systems(Update, bevy::window::close_on_esc)
        .add_systems(Update, demo)
        .run();
}

fn demo(runtime: ResMut<TokioTasksRuntime>, mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    runtime.spawn_background_task(|mut ctx| async move {
        let mut color_index = 0;
        loop {
            ctx.run_on_main_thread(move |ctx| {
                if let Some(mut clear_color) = ctx.world.get_resource_mut::<ClearColor>() {
                    clear_color.0 = COLORS[color_index];
                    println!("Changed clear color to {:?}", clear_color.0);
                }
            }).await;
            color_index = (color_index + 1) % COLORS.len();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}