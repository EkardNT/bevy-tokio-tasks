use std::time::Duration;

use bevy::color::Srgba;
use bevy::prelude::{App, Camera2d, ClearColor, Commands, DefaultPlugins, ResMut};
use bevy_app::Startup;

use bevy_tokio_tasks::TokioTasksRuntime;

static COLORS: [Srgba; 5] = [
    bevy::color::palettes::css::RED,
    bevy::color::palettes::css::GREEN,
    bevy::color::palettes::css::BLUE,
    bevy::color::palettes::css::WHITE,
    bevy::color::palettes::css::BLACK,
];

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(bevy_tokio_tasks::TokioTasksPlugin {
            make_runtime: Box::new(|| {
                let mut runtime = tokio::runtime::Builder::new_current_thread();
                runtime.enable_all();
                runtime.build().unwrap()
            }),
            ..bevy_tokio_tasks::TokioTasksPlugin::default()
        })
        .add_systems(Startup, demo)
        .run();
}

fn demo(runtime: ResMut<TokioTasksRuntime>, mut commands: Commands) {
    commands.spawn(Camera2d);
    runtime.spawn_background_task(|mut ctx| async move {
        let mut color_index = 0;
        loop {
            println!("Loop start");
            ctx.run_on_main_thread(move |ctx| {
                if let Some(mut clear_color) = ctx.world.get_resource_mut::<ClearColor>() {
                    clear_color.0 = bevy::prelude::Color::Srgba(COLORS[color_index]);
                    println!("Changed clear color to {:?}", clear_color.0);
                }
            })
            .await;
            color_index = (color_index + 1) % COLORS.len();
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}
