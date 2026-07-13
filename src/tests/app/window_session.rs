use crate::tests::common::*;
use crate::ui::widgets::Container;
use crate::app::active_work_registry::ActiveWorkRegistry;
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::draw::traits::GraphicsEngine;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::app::window_session::*;
use crate::app::main_thread_queue::MainThreadContext;
use crate::draw::traits::{ Canvas2D, GraphicsCapabilities, UpdateStrategy };
use crate::impl_widget_component;
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::{ WidgetLifecycle };
use crate::ui::view::combinators::{dynamic_label, label};
use crate::ui::widgets::Label;
use crate::ui::{ State };

struct SessionLifecycleProbe {
    events: Rc<RefCell<Vec<&'static str>>>,
}

impl_widget_component!(SessionLifecycleProbe; Layout, Lifecycle);

impl WidgetLayout for SessionLifecycleProbe {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(24.0, 16.0))
    }
}

impl WidgetLifecycle for SessionLifecycleProbe {
    fn on_init(&mut self) {
        self.events.borrow_mut().push("init");
    }

    fn on_attach(&mut self) {
        self.events.borrow_mut().push("attach");
    }

    fn on_mount(&mut self) {
        self.events.borrow_mut().push("mount");
    }

    fn on_active(&mut self) {
        self.events.borrow_mut().push("active");
    }

    fn on_inactive(&mut self) {
        self.events.borrow_mut().push("inactive");
    }

    fn on_unmount(&mut self) {
        self.events.borrow_mut().push("unmount");
    }

    fn on_detach(&mut self) {
        self.events.borrow_mut().push("detach");
    }

    fn on_destroy(&mut self) {
        self.events.borrow_mut().push("destroy");
    }
}

struct ShutdownTrackingEngine {
    inner: NullEngine,
    shutdown_calls: Rc<Cell<usize>>,
}

impl ShutdownTrackingEngine {
    fn new(shutdown_calls: Rc<Cell<usize>>) -> Self {
        Self {
            inner: NullEngine::new(),
            shutdown_calls,
        }
    }
}

impl GraphicsEngine for ShutdownTrackingEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.inner.initialize(w, h)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.shutdown_calls.set(self.shutdown_calls.get() + 1);
        self.inner.try_shutdown()
    }

    fn resize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.inner.resize(w, h)?;
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, present_damage: &crate::draw::backend::DamageRegion) -> RenderOutcome {
        self.inner.end_frame(present_damage)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.inner.capabilities()
    }
}

fn root_label_text(root: ViewNode) -> String {
    let tree = ViewAdapter::build_nodes(root);
    tree.root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap()
        .text()
        .to_string()
}

#[test]
fn window_session_builds_tree_engine_and_p0_state() {
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(NullEngine::new()),
        320,
        240,
    );

    assert_eq!(session.loop_state(), WindowLoopState::Active);
    assert!(session.active_work().is_empty());
    assert!(session.window_visible());
    assert!(!session.view_factory().is_installed());

    let (tree, _engine) = session.tree_and_engine_mut();
    let root_id = tree.root_id().expect("session root should exist");
    let root = tree.get(root_id).expect("session root should be present");

    assert_eq!(root.frame().w, 320.0);
    assert_eq!(root.frame().h, 240.0);
    assert!(tree.has_render_work());
}

#[test]
fn window_session_injects_app_state_into_existing_mounted_tree() {
    let mut session =
        WindowSession::from_root(label("root"), Box::new(NullEngine::new()), 320, 240);
    let app_state = AppState::new();
    session.set_app_state(app_state.clone());

    let (tree, _engine) = session.tree_and_engine_mut();
    let root_id = tree.root_id().unwrap();

    assert_eq!(
        app_state.get_handle(root_id).unwrap().text().as_deref(),
        Some("root")
    );
}

#[test]
fn shared_state_marks_only_bound_window_session_dirty() {
    let state = State::new(0);
    let mut bound_session = WindowSession::from_root_factory(
        {
            let state = state.clone();
            move || {
                let state = state.clone();
                dynamic_label(move || format!("value-{}", state.get()))
            }
        },
        Box::new(NullEngine::new()),
        320,
        240,
    );
    let mut unrelated_session =
        WindowSession::from_root(label("static"), Box::new(NullEngine::new()), 320, 240);

    {
        let (tree, _) = bound_session.tree_and_engine_mut();
        tree.reset_invalidation();
        assert!(!tree.has_render_work());
    }
    {
        let (tree, _) = unrelated_session.tree_and_engine_mut();
        tree.reset_invalidation();
        assert!(!tree.has_render_work());
    }

    state.set(1);

    {
        let (tree, _) = bound_session.tree_and_engine_mut();
        assert!(tree.take_reconcile_requested());
        assert!(tree.has_render_work());
    }
    {
        let (tree, _) = unrelated_session.tree_and_engine_mut();
        assert!(!tree.take_reconcile_requested());
        assert!(!tree.has_render_work());
    }
}

#[test]
fn window_session_factory_builds_reconcile_root_when_no_pending_root() {
    let mut session = WindowSession::from_root_factory(
        || label("factory"),
        Box::new(NullEngine::new()),
        320,
        240,
    );

    assert!(session.view_factory().is_installed());
    session.request_reconcile();

    let root = session
        .take_pending_root()
        .or_else(|| session.view_factory().build())
        .unwrap();

    assert!(session.reconcile_pending());
    assert_eq!(root_label_text(root), "factory");
}

#[test]
fn window_session_pending_root_is_one_shot_and_overrides_factory() {
    let mut session = WindowSession::from_root_factory(
        || label("factory"),
        Box::new(NullEngine::new()),
        320,
        240,
    );

    {
        let parts = session.parts_mut();
        let mut context = MainThreadContext::new(parts.pending_root, parts.reconcile_pending);
        context.update_root(label("pending"));
    }

    let pending = session.take_pending_root().unwrap();
    assert_eq!(root_label_text(pending), "pending");
    assert!(session.take_pending_root().is_none());
    assert_eq!(
        root_label_text(session.view_factory().build().unwrap()),
        "factory"
    );
}

#[test]
fn window_session_shutdown_is_idempotent_and_drop_does_not_repeat_it() {
    let shutdown_calls = Rc::new(Cell::new(0));
    {
        let mut session = WindowSession::from_root(
            label("root"),
            Box::new(ShutdownTrackingEngine::new(shutdown_calls.clone())),
            320,
            240,
        );

        session.shutdown();
        session.shutdown();
        assert_eq!(shutdown_calls.get(), 1);
    }
    assert_eq!(shutdown_calls.get(), 1);
}

#[test]
fn window_session_drop_shuts_engine_down_once() {
    let shutdown_calls = Rc::new(Cell::new(0));
    {
        let _session = WindowSession::from_root(
            label("root"),
            Box::new(ShutdownTrackingEngine::new(shutdown_calls.clone())),
            320,
            240,
        );
    }
    assert_eq!(shutdown_calls.get(), 1);
}

#[test]
fn window_session_shutdown_tears_down_widget_lifecycle_once() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut session = WindowSession::from_root(
        ViewNode::leaf(SessionLifecycleProbe {
            events: events.clone(),
        }),
        Box::new(NullEngine::new()),
        320,
        240,
    );

    assert_eq!(*events.borrow(), ["init", "attach", "mount", "active"]);

    session.shutdown();
    session.shutdown();

    assert_eq!(
        *events.borrow(),
        ["init", "attach", "mount", "active", "inactive", "unmount", "detach", "destroy"]
    );
}
