use std::time::Duration;

use bevy::prelude::{
    App, Camera2dBundle, ClearColor, Color, Commands, DefaultPlugins, ResMut, Update,
};

use bevy_app::Startup;
use bevy_tokio_tasks::{TaskContext, TokioTasksPlugin, TokioTasksRuntime};

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
        .add_systems(Startup, demo)
        .run();
}

fn demo(runtime: ResMut<TokioTasksRuntime>, mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    runtime.spawn_background_task(update_colors);
}

async fn update_colors(mut ctx: TaskContext) {
    let mut color_index = 0;
    loop {
        ctx.run_on_main_thread(move |ctx| {
            if let Some(mut clear_color) = ctx.world.get_resource_mut::<ClearColor>() {
                clear_color.0 = COLORS[color_index];
                println!("Changed clear color to {:?}", clear_color.0);
            }
        })
        .await;
        color_index = (color_index + 1) % COLORS.len();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
