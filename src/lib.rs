use std::{sync::{Arc, atomic::{Ordering, AtomicUsize}}, future::Future, ops::{Deref, DerefMut}};

use bevy_app::{CoreStage, Plugin, App};
use bevy_ecs::{system::Resource, prelude::World};
use tokio::{runtime::Runtime, task::{JoinHandle}};

/// An internal struct keeping track of how many ticks have elapsed since the start of the program.
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

/// The Bevy [`Plugin`] which sets up the [`TokioTasksRuntime`] Bevy resource and registers
/// the [`tick_runtime_update`] exclusive system.
pub struct TokioTasksPlugin {
    /// Callback which is used to create a Tokio runtime when the plugin is installed. The
    /// default value for this field configures a multi-threaded [`Runtime`] with IO and timer
    /// functionality enabled.
    pub make_runtime: Box<dyn Fn() -> Runtime + Send + Sync + 'static>,
    /// The stage to which the [`tick_runtime_update`] system will be added. The default
    /// value for this field is [`CoreStage::Update`].
    pub tick_stage: CoreStage,
}

impl Default for TokioTasksPlugin {
    /// Configures the plugin to build a new multi-threaded Tokio [`Runtime`] with both
    /// IO and timer functionality enabled.
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

/// The Bevy exclusive system which executes the main thread callbacks that background
/// tasks requested using [`run_on_main_thread`](TaskContext::run_on_main_thread). You
/// can control which [`CoreStage`] this system executes in by specifying a custom
/// [`tick_stage`](TokioTasksPlugin::tick_stage) value.
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

/// The Bevy [`Resource`] which stores the Tokio [`Runtime`] and allows for spawning new
/// background tasks.
#[derive(Resource)]
pub struct TokioTasksRuntime(pub Box<TokioTasksRuntimeInner>);

impl Deref for TokioTasksRuntime {
    type Target = TokioTasksRuntimeInner;

    fn deref(&self) -> &Self::Target {
        return &self.0;
    }
}

impl DerefMut for TokioTasksRuntime {
    fn deref_mut(&mut self) -> &mut Self::Target {
        return &mut self.0;
    }
}

pub struct TokioTasksRuntimeInner {
    /// The Tokio [`Runtime`] on which background tasks are executed. You can specify
    /// how this is created by providing a custom [`make_runtime`](TokioTasksPlugin::make_runtime).
    pub runtime: Runtime,
    ticks: Arc<AtomicUsize>,
    update_watch_rx: tokio::sync::watch::Receiver<()>,
    update_run_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    update_run_rx: tokio::sync::mpsc::UnboundedReceiver<MainThreadCallback>,
}

impl TokioTasksRuntime {
    fn new(
            ticks: Arc<AtomicUsize>,
            runtime: Runtime,
            update_watch_rx: tokio::sync::watch::Receiver<()>) -> Self {
        let (update_run_tx, update_run_rx) = tokio::sync::mpsc::unbounded_channel();

        Self(Box::new(TokioTasksRuntimeInner {
            runtime,
            ticks,
            update_watch_rx,
            update_run_tx,
            update_run_rx,
        }))
    }

    /// Spawn a task which will run on the background Tokio [`Runtime`] managed by this [`TokioTasksRuntime`]. The
    /// background task is provided a [`TaskContext`] which allows it to do things like
    /// [sleep for a given number of main thread updates](TaskContext::sleep_updates) or 
    /// [invoke callbacks on the main Bevy thread](TaskContext::run_on_main_thread).
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

/// The context arguments which are available to main thread callbacks requested using
/// [`run_on_main_thread`](TaskContext::run_on_main_thread).
pub struct MainThreadContext<'a> {
    /// A mutable reference to the main Bevy [World].
    pub world: &'a mut World,
    /// The current update tick in which the current main thread callback is executing.
    pub current_tick: usize,
}

/// The context arguments which are available to background tasks spawned onto the
/// [`TokioTasksRuntime`].
#[derive(Clone)]
pub struct TaskContext {
    update_watch_rx: tokio::sync::watch::Receiver<()>,
    update_run_tx: tokio::sync::mpsc::UnboundedSender<MainThreadCallback>,
    ticks: Arc<AtomicUsize>,
}

impl TaskContext {
    /// Returns the current value of the ticket count from the main thread - how many updates
    /// have occurred since the start of the program. Because the tick count is updated from the
    /// main thread, the tick count may change any time after this function call returns.
    pub fn current_tick(&self) -> usize {
        self.ticks.load(Ordering::SeqCst)
    }

    /// Sleeps the background task until a given number of main thread updates have occurred. If
    /// you instead want to sleep for a given length of wall-clock time, call the normal Tokio sleep
    /// function.
    pub async fn sleep_updates(&mut self, updates_to_sleep: usize) {
        let target_tick = self.ticks.load(Ordering::SeqCst).wrapping_add(updates_to_sleep);
        while self.ticks.load(Ordering::SeqCst) < target_tick {
            if self.update_watch_rx.changed().await.is_err() {
                return;
            }
        }
    }

    /// Invokes a synchronous callback on the main Bevy thread. The callback will have mutable access to the
    /// main Bevy [`World`], allowing it to update any resources or entities that it wants. The callback can
    /// report results back to the background thread by returning an output value, which will then be returned from
    /// this async function once the callback runs.
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