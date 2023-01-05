use std::{sync::{Arc, atomic::{Ordering, AtomicUsize}}, future::Future};

use bevy_app::{CoreStage, Plugin, App};
use bevy_ecs::{system::Resource, prelude::World};
use tokio::{runtime::Runtime, task::{JoinHandle}};


#[derive(Resource)]
struct UpdateTicks {
    ticks: Arc<AtomicUsize>,
    update_watch_tx: tokio::sync::watch::Sender<()>,
}

impl UpdateTicks {
    fn increment_ticks(&self) -> usize {
        let new_ticks = self.ticks.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        self.update_watch_tx.send(()).expect("Failed to send update_watch channel message");
        new_ticks
    }
}

pub struct TokioTasksPlugin {
    /// Callback which is used to create a Tokio runtime when the plugin is installed.
    pub make_runtime: Box<dyn Fn() -> Runtime + Send + Sync + 'static>,
    /// The stage to which the `tick_runtime_update` system will be added.
    pub tick_stage: CoreStage,
}

impl Default for TokioTasksPlugin {
    fn default() -> Self {
        Self {
            make_runtime: Box::new(|| {
                let mut runtime = tokio::runtime::Builder::new_multi_thread();
                runtime.enable_all();
                runtime.build().expect("Failed to create Tokio runtime for background tasks")
            }),
            tick_stage: CoreStage::Update,
        }
    }
}

impl Plugin for TokioTasksPlugin {
    fn build(&self, app: &mut App) {
        let ticks = Arc::new(AtomicUsize::new(0));
        let (update_watch_tx, update_watch_rx) = tokio::sync::watch::channel(());
        let runtime = (self.make_runtime)();
        app.insert_resource(UpdateTicks {
            ticks: ticks.clone(),
            update_watch_tx,
        });
        app.insert_resource(TokioTasksRuntime::new(
            ticks,
            runtime,
            update_watch_rx,
        ));
        app.add_system_to_stage(self.tick_stage.clone(), tick_runtime_update);
    }
}

pub fn tick_runtime_update(world: &mut World) {
    let current_tick = {
        let tick_counter = match world.get_resource::<UpdateTicks>() {
            Some(counter) => counter,
            None => return
        };
        
        // Increment update ticks and notify watchers of update tick.
        tick_counter.increment_ticks()
    };

    if let Some(mut runtime) = world.remove_resource::<TokioTasksRuntime>() {
        runtime.execute_main_thread_work(world, current_tick);
        world.insert_resource(runtime);
    }
}

type MainThreadCallback = Box<dyn FnOnce(MainThreadContext) + Send + 'static>;

#[derive(Resource)]
pub struct TokioTasksRuntime {
    pub runtime: Runtime,
    ticks: Arc<AtomicUsize>,
    update_watch_rx: tokio::sync::watch::Receiver<()>,
    update_run_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    update_run_rx: tokio::sync::mpsc::UnboundedReceiver<MainThreadCallback>,
}

impl TokioTasksRuntime {
    pub fn new(
            ticks: Arc<AtomicUsize>,
            runtime: Runtime,
            update_watch_rx: tokio::sync::watch::Receiver<()>) -> Self {
        let (update_run_tx, update_run_rx) = tokio::sync::mpsc::unbounded_channel();

        Self {
            runtime,
            ticks,
            update_watch_rx,
            update_run_tx,
            update_run_rx,
        }
    }

    pub fn spawn_background_task<Task, Output, Spawnable>(&self, spawnable_task: Spawnable) -> JoinHandle<Output>
    where 
        Task: Future<Output = Output> + Send + 'static,
        Output: Send + 'static,
        Spawnable: FnOnce(TaskContext) -> Task + Send + 'static,
    {
        let context = TaskContext {
            update_watch_rx: self.update_watch_rx.clone(),
            ticks: self.ticks.clone(),
            update_run_tx: self.update_run_tx.clone(),
        };
        let future = spawnable_task(context);
        self.runtime.spawn(future)
    }

    /// Execute all of the requested runnables on the main thread.
    pub(crate) fn execute_main_thread_work(&mut self, world: &mut World, current_tick: usize) {
        while let Ok(runnable) = self.update_run_rx.try_recv() {
            let context = MainThreadContext {
                world,
                current_tick
            };
            runnable(context);
        }
    }
}

pub struct MainThreadContext<'a> {
    pub world: &'a mut World,
    pub current_tick: usize,
}


pub struct TaskContext {
    update_watch_rx: tokio::sync::watch::Receiver<()>,
    update_run_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    ticks: Arc<AtomicUsize>,
}

impl TaskContext {
    pub fn current_tick(&self) -> usize {
        self.ticks.load(Ordering::SeqCst)
    }

    pub async fn sleep_updates(&mut self, updates_to_sleep: usize) {
        let target_tick = self.ticks.load(Ordering::SeqCst).wrapping_add(updates_to_sleep);
        while self.ticks.load(Ordering::SeqCst) < target_tick {
            if self.update_watch_rx.changed().await.is_err() {
                return;
            }
        }
    }

    pub async fn run_on_main_thread<Runnable, Output>(&mut self, runnable: Runnable) -> Output
    where
        Runnable: FnOnce(MainThreadContext) -> Output + Send + 'static,
        Output: Send + 'static
    {
        
        let (output_tx, output_rx) = tokio::sync::oneshot::channel();
        if self.update_run_tx.send(Box::new(move |ctx| {
            if output_tx.send(runnable(ctx)).is_err() {
                panic!("Failed to sent output from operation run on main thread back to waiting task");
            }
        })).is_err() {
            panic!("Failed to send operation to be run on main thread");
        }
        output_rx.await.expect("Failed to receive output from operation on main thread")
    }
}