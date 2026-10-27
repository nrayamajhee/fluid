//! DOM pieces behind `html!`: node children, attribute values, reactive
//! regions and listeners.

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Comment, DocumentFragment, Element, Event, EventTarget, HtmlElement, Node, Text};

use crate::list::{self, Siblings};
use crate::reactive::{create_effect, has_owner, on_cleanup, untrack, Context};

pub const SVG_NS: &str = "http://www.w3.org/2000/svg";

fn document() -> web_sys::Document {
  gloo_utils::document()
}

/// Values `html!` can insert as a child with `{ expr }`. Every
/// `AsRef<web_sys::Node>` type and every `Display` type works without this;
/// implement it for your own types (components, wrappers).
pub trait IntoNode {
  fn into_node(self) -> Node;
}

macro_rules! into_node_via_as_ref {
  ($($ty:ty),*) => {$(
    impl IntoNode for $ty {
      fn into_node(self) -> Node {
        self.into()
      }
    }
  )*};
}

into_node_via_as_ref!(Element, HtmlElement, Text, Comment, DocumentFragment);

impl IntoNode for Node {
  fn into_node(self) -> Node {
    self
  }
}

/// `None` inserts nothing (an empty text node holds its place).
impl<T: IntoNode> IntoNode for Option<T> {
  fn into_node(self) -> Node {
    match self {
      Some(node) => node.into_node(),
      None => text("").into(),
    }
  }
}

/// A list inserts each item in order.
impl<T: IntoNode> IntoNode for Vec<T> {
  fn into_node(self) -> Node {
    let fragment = document().create_document_fragment();
    for item in self {
      append(&fragment, &item.into_node());
    }
    fragment.into()
  }
}

/// What a child expression produced.
#[doc(hidden)]
pub enum Child {
  Node(Node),
  Text(String),
}

impl Child {
  pub fn into_node(self) -> Node {
    match self {
      Self::Node(node) => node,
      Self::Text(value) => text(&value).into(),
    }
  }
}

/// Autoref-specialization wrapper: `(&&&Wrap::new(v)).__child()` picks
/// [`IntoNode`], then `AsRef<Node>`, then `Display`, for the concrete type at
/// the macro call site.
#[doc(hidden)]
pub struct Wrap<T>(Cell<Option<T>>);

impl<T> Wrap<T> {
  pub fn new(value: T) -> Self {
    Self(Cell::new(Some(value)))
  }
  fn take(&self) -> T {
    self.0.take().expect("a child is converted once")
  }
}

#[doc(hidden)]
pub trait ViaIntoNode {
  fn __child(&self) -> Child;
}
impl<T: IntoNode> ViaIntoNode for &&Wrap<T> {
  fn __child(&self) -> Child {
    Child::Node(self.take().into_node())
  }
}

#[doc(hidden)]
pub trait ViaAsNode {
  fn __child(&self) -> Child;
}
impl<T: AsRef<Node>> ViaAsNode for &Wrap<T> {
  fn __child(&self) -> Child {
    Child::Node(self.take().as_ref().clone())
  }
}

#[doc(hidden)]
pub trait ViaDisplay {
  fn __child(&self) -> Child;
}
impl<T: Display> ViaDisplay for Wrap<T> {
  fn __child(&self) -> Child {
    Child::Text(self.take().to_string())
  }
}

/// Attribute values: strings set the attribute, `bool` adds or removes it,
/// `Option` removes it on `None`, numbers are formatted.
pub trait AttrValue {
  fn apply(self, element: &Element, name: &str);
}

impl<T: AsRef<str> + ?Sized> AttrValue for &T {
  fn apply(self, element: &Element, name: &str) {
    set_attribute(element, name, self.as_ref());
  }
}

macro_rules! attr_value_via_as_ref {
  ($($ty:ty),*) => {$(
    impl AttrValue for $ty {
      fn apply(self, element: &Element, name: &str) {
        set_attribute(element, name, self.as_ref());
      }
    }
  )*};
}

attr_value_via_as_ref!(String, Cow<'_, str>, Rc<str>, Box<str>);

macro_rules! attr_value_via_display {
  ($($ty:ty),*) => {$(
    impl AttrValue for $ty {
      fn apply(self, element: &Element, name: &str) {
        set_attribute(element, name, &self.to_string());
      }
    }
  )*};
}

attr_value_via_display!(
  char, i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64
);

impl AttrValue for bool {
  fn apply(self, element: &Element, name: &str) {
    if self {
      set_attribute(element, name, "");
    } else {
      let _ = element.remove_attribute(name);
    }
  }
}

impl<T: AttrValue> AttrValue for Option<T> {
  fn apply(self, element: &Element, name: &str) {
    match self {
      Some(value) => value.apply(element, name),
      None => {
        let _ = element.remove_attribute(name);
      }
    }
  }
}

/// Autoref dispatch for attribute values: [`AttrValue`] first, then
/// anything that derefs twice to a string (`&Ref<String>`, `&Rc<String>`),
/// which `set_attribute(name, &value)` used to accept by deref coercion.
#[doc(hidden)]
pub trait AttrViaValue {
  fn __apply(&self, element: &Element, name: &str);
}
impl<T: AttrValue> AttrViaValue for &&Wrap<T> {
  fn __apply(&self, element: &Element, name: &str) {
    self.take().apply(element, name);
  }
}

#[doc(hidden)]
pub trait AttrViaDeref {
  fn __apply(&self, element: &Element, name: &str);
}
impl<T> AttrViaDeref for &Wrap<T>
where
  T: Deref,
  T::Target: Deref,
  <T::Target as Deref>::Target: AsRef<str>,
{
  fn __apply(&self, element: &Element, name: &str) {
    let value = self.take();
    let value: &str = (**value).as_ref();
    set_attribute(element, name, value);
  }
}

// Names are checked by `html!` at compile time, so these can't fail on its
// behalf; elements made elsewhere are the caller's concern.

#[doc(hidden)]
pub fn element(tag: &str) -> Element {
  document()
    .create_element(tag)
    .expect_throw("fluid: invalid tag name")
}

#[doc(hidden)]
pub fn svg_element(tag: &str) -> Element {
  document()
    .create_element_ns(Some(SVG_NS), tag)
    .expect_throw("fluid: invalid tag name")
}

#[doc(hidden)]
pub fn text(value: &str) -> Text {
  document().create_text_node(value)
}

#[doc(hidden)]
pub fn set_attribute(element: &Element, name: &str, value: &str) {
  let _ = element.set_attribute(name, value);
}

#[doc(hidden)]
pub fn set_property(element: &Element, name: &str, value: impl Into<JsValue>) {
  let _ = js_sys::Reflect::set(element, &JsValue::from_str(name), &value.into());
}

/// Adds or removes each whitespace-separated class.
#[doc(hidden)]
pub fn toggle_class(element: &Element, classes: &str, on: bool) {
  let list = element.class_list();
  for class in classes.split_whitespace() {
    let _ = list.toggle_with_force(class, on);
  }
}

#[doc(hidden)]
pub fn append(parent: &Node, child: &Node) {
  parent
    .append_child(child)
    .expect_throw("fluid: can't append a node to one of its own children");
}

/// The nodes a region shows: a fragment's children, never empty.
fn flatten(node: Node) -> Vec<Node> {
  if node.node_type() != Node::DOCUMENT_FRAGMENT_NODE {
    return vec![node];
  }
  let children = node.child_nodes();
  let nodes: Vec<Node> = (0..children.length())
    .filter_map(|index| children.get(index))
    .collect();
  if nodes.is_empty() {
    vec![text("").into()]
  } else {
    nodes
  }
}

/// One node for a region's initial content, to append into the parent.
fn gather(nodes: &[Node]) -> Node {
  match nodes {
    [node] => node.clone(),
    nodes => {
      let fragment = document().create_document_fragment();
      for node in nodes {
        append(&fragment, node);
      }
      fragment.into()
    }
  }
}

/// Swaps a region's nodes for new ones in place.
fn replace(current: &mut Vec<Node>, next: Vec<Node>) {
  if let Some(parent) = current.first().and_then(Node::parent_node) {
    let first = current[0].clone();
    for node in &next {
      let _ = parent.insert_before(node, Some(&first));
    }
    for old in current.iter() {
      if !next.iter().any(|node| node.is_same_node(Some(old))) {
        let _ = parent.remove_child(old);
      }
    }
  }
  *current = next;
}

/// A child that re-renders when what `render` reads changes: text updates a
/// Text node in place, nodes replace the region.
#[doc(hidden)]
pub fn dynamic(mut render: impl FnMut() -> Child + 'static) -> Node {
  let nodes: Rc<RefCell<Vec<Node>>> = Rc::default();
  {
    let nodes = nodes.clone();
    create_effect(move || {
      let next = render();
      let mut nodes = nodes.borrow_mut();
      match next {
        Child::Text(value) => {
          if let [only] = nodes.as_slice() {
            if let Some(text) = only.dyn_ref::<Text>() {
              if text.data() != value {
                text.set_data(&value);
              }
              return;
            }
          }
          replace(&mut nodes, vec![text(&value).into()]);
        }
        Child::Node(node) => replace(&mut nodes, flatten(node)),
      }
    });
  }
  let nodes = nodes.borrow();
  gather(&nodes)
}

#[doc(hidden)]
pub fn fragment() -> DocumentFragment {
  document().create_document_fragment()
}

/// Builds a branch or row in its own scope, without tracking what it reads.
fn build_in(scope: &Context, build: impl FnOnce() -> Node) -> Vec<Node> {
  flatten(scope.run(|| untrack(build)))
}

/// A branch builder for [`switch`].
#[doc(hidden)]
pub type Branch = Box<dyn FnOnce() -> Node>;

/// `if`/`match`: `select` names the chosen arm, whether it binds values,
/// and how to build it. The region is rebuilt in a fresh scope when the arm
/// changes, or on every change for arms that bind values.
#[doc(hidden)]
pub fn switch(mut select: impl FnMut() -> (usize, bool, Branch) + 'static) -> Node {
  let parent = Context::current();
  let nodes: Rc<RefCell<Vec<Node>>> = Rc::default();
  let shown: Rc<RefCell<Option<(usize, Context)>>> = Rc::default();
  {
    let nodes = nodes.clone();
    create_effect(move || {
      let (arm, binds, build) = select();
      if !binds && matches!(&*shown.borrow(), Some((current, _)) if *current == arm) {
        return;
      }
      let old = shown.borrow_mut().take();
      if let Some((_, scope)) = old {
        scope.dispose();
      }
      let scope = parent.exact_child();
      let next = build_in(&scope, build);
      replace(&mut nodes.borrow_mut(), next);
      *shown.borrow_mut() = Some((arm, scope));
    });
  }
  let nodes = nodes.borrow();
  gather(&nodes)
}

struct Row {
  scope: Context,
  nodes: Vec<Node>,
}

/// A parent's children, for [`list::arrange`].
struct Dom(Node);

impl Siblings for Dom {
  type Node = Node;
  fn next_sibling(&self, node: &Node) -> Option<Node> {
    node.next_sibling()
  }
  fn same(&self, a: &Node, b: &Node) -> bool {
    a.is_same_node(Some(b))
  }
  fn insert_before(&mut self, node: &Node, before: &Node) {
    let _ = self.0.insert_before(node, Some(before));
  }
}

/// A keyed list: rows are built once per key, removed with their scope when
/// the key goes, and moved (not rebuilt) when the order changes. Later
/// duplicates of a key are skipped.
#[doc(hidden)]
pub fn each<T: 'static, K: Eq + Hash + Clone + 'static>(
  mut items: impl FnMut() -> Vec<T> + 'static,
  key: impl Fn(&T) -> K + 'static,
  view: impl Fn(T) -> Node + 'static,
) -> Node {
  let parent = Context::current();
  let container = fragment();
  let end: Node = text("").into();
  append(&container, &end);
  let rows: Rc<RefCell<HashMap<K, Row>>> = Rc::default();
  let order: Rc<RefCell<Vec<K>>> = Rc::default();
  {
    let end = end.clone();
    create_effect(move || {
      let items = items();
      untrack(|| {
        let mut rows = rows.borrow_mut();
        let mut order = order.borrow_mut();
        let keyed: Vec<(K, T)> = items.into_iter().map(|item| (key(&item), item)).collect();
        let diff = list::diff(&order, keyed.iter().map(|(key, _)| key.clone()));
        let mut fresh: HashMap<K, T> = HashMap::new();
        for (key, item) in keyed {
          fresh.entry(key).or_insert(item);
        }
        for gone in &diff.removed {
          if let Some(row) = rows.remove(gone) {
            row.scope.dispose();
            for node in &row.nodes {
              if let Some(parent) = node.parent_node() {
                let _ = parent.remove_child(node);
              }
            }
          }
        }
        for &at in &diff.added {
          let key = &diff.order[at];
          if let Some(item) = fresh.remove(key) {
            let scope = parent.exact_child();
            let nodes = build_in(&scope, || view(item));
            rows.insert(key.clone(), Row { scope, nodes });
          }
        }
        if let Some(parent) = end.parent_node() {
          let placed: Vec<&[Node]> = diff
            .order
            .iter()
            .filter_map(|key| rows.get(key).map(|row| row.nodes.as_slice()))
            .collect();
          list::arrange(&mut Dom(parent), &placed, &end);
        }
        *order = diff.order;
      });
    });
  }
  container.into()
}

/// `addEventListener` options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListenerOptions {
  pub capture: bool,
  pub once: bool,
  /// `None` leaves it to the browser (passive for `wheel`/`touch*` on
  /// window, document and body).
  pub passive: Option<bool>,
}

impl ListenerOptions {
  pub const fn new() -> Self {
    Self {
      capture: false,
      once: false,
      passive: None,
    }
  }
  pub const fn capture(mut self) -> Self {
    self.capture = true;
    self
  }
  pub const fn once(mut self) -> Self {
    self.once = true;
    self
  }
  pub const fn passive(mut self, passive: bool) -> Self {
    self.passive = Some(passive);
    self
  }
}

/// An event listener, removed (and its closure freed) when dropped, unless
/// [`forget`](Listener::forget)-ten or [`owned`](Listener::owned) by a scope.
#[must_use = "a Listener is removed when dropped; call .owned() or .forget() to keep it"]
pub struct Listener {
  target: EventTarget,
  name: Cow<'static, str>,
  capture: bool,
  closure: Option<Closure<dyn FnMut(JsValue)>>,
}

impl Listener {
  pub fn remove(self) {}

  /// Keeps the listener for the page's lifetime, as `add_event_and_forget`.
  pub fn forget(mut self) {
    if let Some(closure) = self.closure.take() {
      closure.forget();
    }
  }

  /// Hands the listener to the current scope, which removes it when
  /// disposed; with no current scope it is forgotten.
  pub fn owned(self) {
    if has_owner() {
      on_cleanup(move || drop(self));
    } else {
      self.forget();
    }
  }
}

impl Drop for Listener {
  fn drop(&mut self) {
    if let Some(closure) = self.closure.take() {
      let _ = self.target.remove_event_listener_with_callback_and_bool(
        &self.name,
        closure.as_ref().unchecked_ref(),
        self.capture,
      );
    }
  }
}

/// Adds a listener whose event is cast to `E` (`MouseEvent`, `WheelEvent`…).
pub fn listen<E: JsCast + 'static>(
  target: &EventTarget,
  name: impl Into<Cow<'static, str>>,
  options: ListenerOptions,
  mut handler: impl FnMut(E) + 'static,
) -> Listener {
  let name = name.into();
  let closure =
    Closure::<dyn FnMut(JsValue)>::new(move |event: JsValue| handler(event.unchecked_into::<E>()));
  let js_options = web_sys::AddEventListenerOptions::new();
  js_options.set_capture(options.capture);
  js_options.set_once(options.once);
  if let Some(passive) = options.passive {
    js_options.set_passive(passive);
  }
  let _ = target.add_event_listener_with_callback_and_add_event_listener_options(
    &name,
    closure.as_ref().unchecked_ref(),
    &js_options,
  );
  Listener {
    target: target.clone(),
    name,
    capture: options.capture,
    closure: Some(closure),
  }
}

/// `@event` in `html!`: `.prevent`/`.stop` run first, and the listener
/// belongs to the current scope.
#[doc(hidden)]
pub fn on<E: JsCast + 'static>(
  target: &EventTarget,
  name: &'static str,
  options: ListenerOptions,
  prevent: bool,
  stop: bool,
  mut handler: impl FnMut(E) + 'static,
) {
  listen(target, name, options, move |event: E| {
    let raw: &Event = event.unchecked_ref();
    if prevent {
      raw.prevent_default();
    }
    if stop {
      raw.stop_propagation();
    }
    handler(event)
  })
  .owned();
}
