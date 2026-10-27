//! The DOM-free reactive core: signals, effects, memos, batching and owner
//! scopes.
//!
//! Writes use push-pull colouring: a write marks its direct observers
//! `Dirty` and everything downstream of a memo `Check`, queueing effects.
//! A queued effect runs only if it is `Dirty`, or `Check` and one of its
//! memo sources really changed when pulled. Reading a memo pulls it up to
//! date first, so no computation ever sees a stale value.

use std::any::{Any, TypeId};
use std::cell::{Cell, Ref, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

/// A flush running this many computations is assumed to be a cycle.
const MAX_RUNS_PER_FLUSH: usize = 100_000;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum State {
  Clean,
  Check,
  Dirty,
}

/// Something computations read: a signal, or a memo's output.
pub(crate) struct Source {
  observers: RefCell<Vec<Weak<Node>>>,
  /// The computation behind a memo's output, pulled before it is read.
  memo: RefCell<Weak<Node>>,
}

impl Source {
  fn new() -> Rc<Self> {
    Rc::new(Self {
      observers: RefCell::new(Vec::new()),
      memo: RefCell::new(Weak::new()),
    })
  }

  fn track(self: &Rc<Self>) {
    let observer = RT.with(|rt| rt.observer.borrow().clone());
    if let Some(node) = observer {
      node.subscribe(self);
    }
  }

  fn mark_observers(&self, state: State) {
    let observers: Vec<_> = {
      let mut observers = self.observers.borrow_mut();
      observers.retain(|observer| observer.strong_count() > 0);
      observers.iter().filter_map(Weak::upgrade).collect()
    };
    for observer in observers {
      mark(&observer, state);
    }
  }

  /// A write: marks observers and runs what's stale unless batching.
  fn notify(&self) {
    self.mark_observers(State::Dirty);
    flush_if_idle();
  }
}

type Compute = Box<dyn FnMut() -> bool>;

/// An effect or a memo's computation.
struct Node {
  /// Returns whether the output changed (always `false` for effects).
  compute: RefCell<Option<Compute>>,
  sources: RefCell<Vec<Rc<Source>>>,
  state: Cell<State>,
  disposed: Cell<bool>,
  /// A memo's value as others read it; `None` for effects.
  output: Option<Rc<Source>>,
  /// The scope for whatever a run creates, cleared before the next run.
  owner: Weak<Owner>,
}

impl Node {
  fn subscribe(self: &Rc<Self>, source: &Rc<Source>) {
    let mut sources = self.sources.borrow_mut();
    if !sources.iter().any(|known| Rc::ptr_eq(known, source)) {
      sources.push(source.clone());
      source.observers.borrow_mut().push(Rc::downgrade(self));
    }
  }

  fn unsubscribe_all(self: &Rc<Self>) {
    let sources = std::mem::take(&mut *self.sources.borrow_mut());
    let me = Rc::downgrade(self);
    for source in sources {
      source
        .observers
        .borrow_mut()
        .retain(|observer| !observer.ptr_eq(&me));
    }
  }

  fn update_if_necessary(self: &Rc<Self>) {
    if self.state.get() == State::Check {
      let sources = self.sources.borrow().clone();
      for source in sources {
        let memo = source.memo.borrow().upgrade();
        if let Some(memo) = memo {
          memo.update_if_necessary();
          if self.state.get() == State::Dirty {
            break;
          }
        }
      }
      if self.state.get() == State::Check {
        self.state.set(State::Clean);
      }
    }
    if self.state.get() == State::Dirty {
      self.run();
    }
  }

  fn run(self: &Rc<Self>) {
    if self.disposed.get() {
      return;
    }
    let Some(owner) = self.owner.upgrade() else {
      return;
    };
    owner.clean();
    self.unsubscribe_all();
    // Clean before running, so a write the run makes to its own sources
    // queues it again rather than being lost.
    self.state.set(State::Clean);
    let Some(mut compute) = self.compute.borrow_mut().take() else {
      return;
    };
    let changed = with_scope(Some(self.clone()), Some(owner), || batch(&mut compute));
    if self.disposed.get() {
      drop(compute);
    } else {
      *self.compute.borrow_mut() = Some(compute);
    }
    if changed {
      if let Some(output) = &self.output {
        output.mark_observers(State::Dirty);
      }
    }
  }

  fn dispose(self: &Rc<Self>) {
    if self.disposed.replace(true) {
      return;
    }
    let compute = self.compute.borrow_mut().take();
    drop(compute);
    self.unsubscribe_all();
  }
}

fn mark(node: &Rc<Node>, state: State) {
  if node.disposed.get() || node.state.get() >= state {
    return;
  }
  let was_clean = node.state.get() == State::Clean;
  node.state.set(state);
  if was_clean {
    match &node.output {
      Some(output) => output.mark_observers(State::Check),
      None => RT.with(|rt| rt.queue.borrow_mut().push_back(node.clone())),
    }
  }
}

/// A scope that owns effects, memos, cleanups, nested scopes and context
/// values, and disposes them together.
pub(crate) struct Owner {
  parent: Weak<Owner>,
  children: RefCell<Vec<Rc<Owner>>>,
  node: RefCell<Option<Rc<Node>>>,
  cleanups: RefCell<Vec<Box<dyn FnOnce()>>>,
  contexts: RefCell<Vec<(TypeId, Rc<dyn Any>)>>,
  disposed: Cell<bool>,
}

impl Owner {
  fn new(parent: Weak<Owner>) -> Rc<Self> {
    Rc::new(Self {
      parent,
      children: RefCell::new(Vec::new()),
      node: RefCell::new(None),
      cleanups: RefCell::new(Vec::new()),
      contexts: RefCell::new(Vec::new()),
      disposed: Cell::new(false),
    })
  }

  fn child(self: &Rc<Self>) -> Rc<Self> {
    let child = Self::new(Rc::downgrade(self));
    if self.disposed.get() {
      child.disposed.set(true);
    } else {
      self.children.borrow_mut().push(child.clone());
    }
    child
  }

  /// Disposes what the scope owns but keeps the scope (and its node) usable.
  fn clean(&self) {
    let children = std::mem::take(&mut *self.children.borrow_mut());
    for child in children.iter().rev() {
      child.dispose_contents();
    }
    let cleanups = std::mem::take(&mut *self.cleanups.borrow_mut());
    for cleanup in cleanups.into_iter().rev() {
      untrack(cleanup);
    }
    self.contexts.borrow_mut().clear();
  }

  fn dispose_contents(&self) {
    if self.disposed.replace(true) {
      return;
    }
    self.clean();
    let node = self.node.borrow_mut().take();
    if let Some(node) = node {
      node.dispose();
    }
  }

  fn dispose(self: &Rc<Self>) {
    self.dispose_contents();
    match self.parent.upgrade() {
      Some(parent) => parent
        .children
        .borrow_mut()
        .retain(|child| !Rc::ptr_eq(child, self)),
      None => RT.with(|rt| rt.roots.borrow_mut().retain(|root| !Rc::ptr_eq(root, self))),
    }
  }

  fn is_within(self: &Rc<Self>, ancestor: &Rc<Owner>) -> bool {
    let mut owner = Some(self.clone());
    while let Some(current) = owner {
      if Rc::ptr_eq(&current, ancestor) {
        return true;
      }
      owner = current.parent.upgrade();
    }
    false
  }

  fn lookup<T: Clone + 'static>(self: &Rc<Self>) -> Option<T> {
    let mut owner = Some(self.clone());
    while let Some(current) = owner {
      let found = current
        .contexts
        .borrow()
        .iter()
        .rev()
        .find(|(id, _)| *id == TypeId::of::<T>())
        .and_then(|(_, value)| value.downcast_ref::<T>().cloned());
      if found.is_some() {
        return found;
      }
      owner = current.parent.upgrade();
    }
    None
  }

  fn create_node(self: &Rc<Self>, compute: Compute, output: Option<Rc<Source>>) -> Rc<Node> {
    let owner = self.child();
    let node = Rc::new(Node {
      compute: RefCell::new(Some(compute)),
      sources: RefCell::new(Vec::new()),
      state: Cell::new(State::Dirty),
      disposed: Cell::new(owner.disposed.get()),
      output,
      owner: Rc::downgrade(&owner),
    });
    *owner.node.borrow_mut() = Some(node.clone());
    node
  }
}

struct Runtime {
  observer: RefCell<Option<Rc<Node>>>,
  owner: RefCell<Option<Rc<Owner>>>,
  batch: Cell<usize>,
  flushing: Cell<bool>,
  queue: RefCell<VecDeque<Rc<Node>>>,
  /// Roots stay alive until disposed, even when every `Context` is dropped.
  roots: RefCell<Vec<Rc<Owner>>>,
  /// Owns what's created outside any scope; never disposed.
  detached: Rc<Owner>,
}

thread_local! {
  static RT: Runtime = Runtime {
    observer: RefCell::new(None),
    owner: RefCell::new(None),
    batch: Cell::new(0),
    flushing: Cell::new(false),
    queue: RefCell::new(VecDeque::new()),
    roots: RefCell::new(Vec::new()),
    detached: Owner::new(Weak::new()),
  };
}

/// Runs `f` with the given tracking observer and current owner, restoring
/// the previous ones afterwards (also on unwind).
fn with_scope<R>(observer: Option<Rc<Node>>, owner: Option<Rc<Owner>>, f: impl FnOnce() -> R) -> R {
  struct Restore(Option<Rc<Node>>, Option<Rc<Owner>>);
  impl Drop for Restore {
    fn drop(&mut self) {
      RT.with(|rt| {
        *rt.observer.borrow_mut() = self.0.take();
        *rt.owner.borrow_mut() = self.1.take();
      });
    }
  }
  let _restore = RT.with(|rt| Restore(rt.observer.replace(observer), rt.owner.replace(owner)));
  f()
}

fn current_observer() -> Option<Rc<Node>> {
  RT.with(|rt| rt.observer.borrow().clone())
}

pub(crate) fn current_owner() -> Option<Rc<Owner>> {
  RT.with(|rt| rt.owner.borrow().clone())
}

fn current_or_detached() -> Rc<Owner> {
  current_owner().unwrap_or_else(|| RT.with(|rt| rt.detached.clone()))
}

fn flush_if_idle() {
  let idle = RT.with(|rt| rt.batch.get() == 0 && !rt.flushing.get());
  if idle {
    flush();
  }
}

fn flush() {
  struct Done;
  impl Drop for Done {
    fn drop(&mut self) {
      RT.with(|rt| rt.flushing.set(false));
    }
  }
  RT.with(|rt| rt.flushing.set(true));
  let _done = Done;
  let mut runs = 0;
  while let Some(node) = RT.with(|rt| rt.queue.borrow_mut().pop_front()) {
    runs += 1;
    if runs > MAX_RUNS_PER_FLUSH {
      RT.with(|rt| rt.queue.borrow_mut().clear());
      panic!("fluid: effects ran {MAX_RUNS_PER_FLUSH} times in one flush; an effect probably writes a signal it reads");
    }
    node.update_if_necessary();
  }
}

/// Runs `f`, deferring effects until it returns, so each one runs at most
/// once and sees every write `f` made.
pub fn batch<R>(f: impl FnOnce() -> R) -> R {
  struct Leave;
  impl Drop for Leave {
    fn drop(&mut self) {
      RT.with(|rt| rt.batch.set(rt.batch.get() - 1));
    }
  }
  RT.with(|rt| rt.batch.set(rt.batch.get() + 1));
  let result = {
    let _leave = Leave;
    f()
  };
  flush_if_idle();
  result
}

/// Runs `f` without subscribing the running effect or memo to what it reads.
pub fn untrack<R>(f: impl FnOnce() -> R) -> R {
  let owner = current_owner();
  with_scope(None, owner, f)
}

/// A reactive value. Reading it inside an effect or memo subscribes that
/// computation; writing it re-runs them.
pub struct Signal<T> {
  value: RefCell<T>,
  source: Rc<Source>,
}

impl<T> Signal<T> {
  pub fn new(value: T) -> Self {
    Self {
      value: RefCell::new(value),
      source: Source::new(),
    }
  }

  /// Borrows the value and subscribes the running computation. Don't hold
  /// the `Ref` across a write to the same signal; prefer [`Signal::with`].
  pub fn get(&self) -> Ref<'_, T> {
    self.source.track();
    self.value.borrow()
  }

  pub fn get_untracked(&self) -> Ref<'_, T> {
    self.value.borrow()
  }

  pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    self.source.track();
    f(&self.value.borrow())
  }

  pub fn with_untracked<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    f(&self.value.borrow())
  }

  pub fn set(&self, value: T) {
    *self.borrow_mut() = value;
    self.source.notify();
  }

  /// Mutates the value in place, then notifies once.
  pub fn update<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
    let result = f(&mut self.borrow_mut());
    self.source.notify();
    result
  }

  /// Re-runs subscribers without changing the value.
  pub fn notify(&self) {
    self.source.notify();
  }

  fn borrow_mut(&self) -> std::cell::RefMut<'_, T> {
    self.value.try_borrow_mut().unwrap_or_else(|_| {
      panic!("fluid: a signal was written while a `Ref` from `get()` was alive; read it with `with`, `value` or `cloned` instead")
    })
  }
}

impl<T: Copy> Signal<T> {
  /// Copies the value out, subscribing the running computation.
  pub fn value(&self) -> T {
    self.with(|value| *value)
  }
}

impl<T: Clone> Signal<T> {
  pub fn cloned(&self) -> T {
    self.with(T::clone)
  }
}

impl<T: PartialEq> Signal<T> {
  /// Sets the value only if it differs, returning whether it did.
  pub fn set_if_changed(&self, value: T) -> bool {
    if *self.value.borrow() == value {
      return false;
    }
    self.set(value);
    true
  }
}

impl<T: Default> Default for Signal<T> {
  fn default() -> Self {
    Self::new(T::default())
  }
}

/// A derived value, recomputed when what it reads changes and notifying
/// only when the result differs. Cloning is cheap: clones share the value.
pub struct Memo<T> {
  value: Rc<RefCell<Option<T>>>,
  node: Rc<Node>,
}

impl<T> Clone for Memo<T> {
  fn clone(&self) -> Self {
    Self {
      value: self.value.clone(),
      node: self.node.clone(),
    }
  }
}

impl<T: PartialEq + 'static> Memo<T> {
  fn new_in(owner: &Rc<Owner>, mut f: impl FnMut() -> T + 'static) -> Self {
    let value: Rc<RefCell<Option<T>>> = Rc::new(RefCell::new(None));
    let output = Source::new();
    let compute: Compute = {
      let value = value.clone();
      Box::new(move || {
        let next = f();
        let mut value = value.borrow_mut();
        if value.as_ref() == Some(&next) {
          false
        } else {
          *value = Some(next);
          true
        }
      })
    };
    let node = owner.create_node(compute, Some(output.clone()));
    *output.memo.borrow_mut() = Rc::downgrade(&node);
    if node.disposed.get() {
      // Created in a disposed scope: compute once and stay constant.
      let compute = node.compute.borrow_mut().take();
      if let Some(mut compute) = compute {
        untrack(&mut compute);
      }
    } else {
      batch(|| node.run());
    }
    Self { value, node }
  }
}

impl<T> Memo<T> {
  fn source(&self) -> &Rc<Source> {
    self.node.output.as_ref().expect("a memo has an output")
  }

  pub fn get(&self) -> Ref<'_, T> {
    self.node.update_if_necessary();
    self.source().track();
    Ref::map(self.value.borrow(), |value| {
      value.as_ref().expect("a memo is computed when created")
    })
  }

  pub fn get_untracked(&self) -> Ref<'_, T> {
    self.node.update_if_necessary();
    Ref::map(self.value.borrow(), |value| {
      value.as_ref().expect("a memo is computed when created")
    })
  }

  pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
    f(&self.get())
  }
}

impl<T: Copy> Memo<T> {
  pub fn value(&self) -> T {
    *self.get()
  }
}

impl<T: Clone> Memo<T> {
  pub fn cloned(&self) -> T {
    self.get().clone()
  }
}

fn create_effect_in(owner: &Rc<Owner>, mut f: impl FnMut() + 'static) {
  let node = owner.create_node(
    Box::new(move || {
      f();
      false
    }),
    None,
  );
  batch(|| node.run());
}

/// Runs `f` now and again whenever a signal or memo it read changes. It is
/// owned by the current scope (the running effect, a control-flow branch,
/// `Context::run`), or lives forever outside of one.
pub fn create_effect(f: impl FnMut() + 'static) {
  create_effect_in(&current_or_detached(), f);
}

pub fn create_memo<T: PartialEq + 'static>(f: impl FnMut() -> T + 'static) -> Memo<T> {
  Memo::new_in(&current_or_detached(), f)
}

/// Registers `f` to run when the current scope is disposed or, inside an
/// effect, before it re-runs. Outside any scope it never runs.
pub fn on_cleanup(f: impl FnOnce() + 'static) {
  if let Some(owner) = current_owner() {
    owner.cleanups.borrow_mut().push(Box::new(f));
  }
}

/// Whether there is a current scope that will dispose what's created now.
pub fn has_owner() -> bool {
  current_owner().is_some()
}

pub fn provide_context<T: Clone + 'static>(value: T) {
  current_or_detached()
    .contexts
    .borrow_mut()
    .push((TypeId::of::<T>(), Rc::new(value)));
}

/// Looks up a value provided by the current scope or one of its ancestors.
pub fn use_context<T: Clone + 'static>() -> Option<T> {
  current_owner().and_then(|owner| owner.lookup())
}

/// An owner scope. `Context::new()` makes a root; effects, memos, cleanups
/// and child scopes created through it are disposed together by
/// [`Context::dispose`]. Dropping a `Context` never disposes anything.
#[derive(Clone)]
pub struct Context {
  owner: Rc<Owner>,
}

impl Default for Context {
  fn default() -> Self {
    Self::new()
  }
}

impl Context {
  pub fn new() -> Self {
    let owner = Owner::new(Weak::new());
    RT.with(|rt| rt.roots.borrow_mut().push(owner.clone()));
    Self { owner }
  }

  /// The current scope (or the never-disposed detached one).
  pub fn current() -> Self {
    Self {
      owner: current_or_detached(),
    }
  }

  pub fn child(&self) -> Self {
    Self {
      owner: self.target().child(),
    }
  }

  /// A child of exactly this scope, even while an effect inside it runs.
  pub(crate) fn exact_child(&self) -> Self {
    Self {
      owner: self.owner.child(),
    }
  }

  /// Where things created through this context go: the running scope if
  /// it is inside this one (an effect re-running inside `self`) or if `self`
  /// is a root, which apps pass everywhere and so stands for "the current
  /// scope"; otherwise `self`.
  fn target(&self) -> Rc<Owner> {
    match current_owner() {
      Some(current) if current.is_within(&self.owner) || self.owner.parent.upgrade().is_none() => {
        current
      }
      _ => self.owner.clone(),
    }
  }

  /// Runs `f` with this scope as the current one.
  pub fn run<R>(&self, f: impl FnOnce() -> R) -> R {
    with_scope(current_observer(), Some(self.owner.clone()), f)
  }

  pub fn dispose(&self) {
    self.owner.dispose();
  }

  pub fn is_disposed(&self) -> bool {
    self.owner.disposed.get()
  }

  pub fn create_signal<T>(&self, value: T) -> Rc<Signal<T>> {
    Rc::new(Signal::new(value))
  }

  pub fn create_effect(&self, f: impl FnMut() + 'static) {
    create_effect_in(&self.target(), f);
  }

  pub fn create_memo<T: PartialEq + 'static>(&self, f: impl FnMut() -> T + 'static) -> Memo<T> {
    Memo::new_in(&self.target(), f)
  }

  pub fn on_cleanup(&self, f: impl FnOnce() + 'static) {
    let owner = self.target();
    if owner.disposed.get() {
      untrack(f);
    } else {
      owner.cleanups.borrow_mut().push(Box::new(f));
    }
  }

  pub fn provide_context<T: Clone + 'static>(&self, value: T) {
    self
      .owner
      .contexts
      .borrow_mut()
      .push((TypeId::of::<T>(), Rc::new(value)));
  }

  /// Looks up a value from the running scope's chain, then this one's.
  pub fn use_context<T: Clone + 'static>(&self) -> Option<T> {
    self.target().lookup().or_else(|| self.owner.lookup())
  }
}
