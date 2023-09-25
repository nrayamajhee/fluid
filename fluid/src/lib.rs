use gloo_utils::window;
use gloo_timers::callback::Timeout;
use std::cell::{Ref, RefCell};
use std::rc::Rc;

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::EventTarget;

pub struct Signal<T> {
  inner: RefCell<T>,
  subscribers: RefCell<Vec<Rc<dyn Fn()>>>,
  context: Rc<RefCell<Vec<Rc<dyn Fn()>>>>,
}

impl<T> Signal<T> {
  pub fn get(&self) -> Ref<'_, T> {
    if let Some(sub) = self.context.borrow().last() {
      if let Ok(mut subscribers) = self.subscribers.try_borrow_mut() {
        subscribers.push(sub.clone());
      }
    }
    self.inner.borrow()
  }
  pub fn set(&self, value: T) {
    *self.inner.borrow_mut() = value;
    for sub in self.subscribers.borrow().iter() {
      sub();
    }
  }
}

pub struct Context {
  observers: Rc<RefCell<Vec<Rc<dyn Fn()>>>>,
}

impl Context {
  pub fn new() -> Self {
    Self {
      observers: Rc::new(RefCell::new(Vec::new())),
    }
  }
  pub fn create_signal<T>(&self, value: T) -> Rc<Signal<T>> {
    Rc::new(Signal {
      inner: RefCell::new(value),
      subscribers: RefCell::new(Vec::new()),
      context: self.observers.clone(),
    })
  }
  pub fn create_effect(&self, closure: impl Fn() + 'static) {
    let closure = Rc::new(closure);
    self.observers.borrow_mut().push(closure.clone());
    closure();
  }
}

pub fn add_event_and_forget<E>(el: &EventTarget, name: &str, event: E)
where
  E: FnMut(web_sys::Event) + 'static,
{
  let cl = Closure::wrap(Box::new(event) as Box<dyn FnMut(web_sys::Event)>);
  el.add_event_listener_with_callback(name, cl.as_ref().unchecked_ref())
    .unwrap();
  cl.forget();
}

pub fn now() -> f64 {
  window().performance().unwrap().now()
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) -> i32 {
  window()
    .request_animation_frame(f.as_ref().unchecked_ref())
    .expect("should register `requestAnimationFrame` OK")
}

pub fn on_animation_frame(mut closure: impl FnMut(f64) + 'static, fps: Option<f64>) {
  let t = Rc::new(RefCell::new(0.));
  let f = Rc::new(RefCell::new(None));
  let g = f.clone();
  let then = t.clone();
  let closure = Closure::wrap(Box::new(move || {
    let mut then = then.borrow_mut();
    let delta = now() - *then;
    closure(delta);
    *then = now();
    let h = f.clone();
    let next_frame = move || {
      request_animation_frame(h.borrow().as_ref().unwrap());
    };
    if let Some(fps) = fps {
      Timeout::new(((1000. / fps) - delta) as u32, next_frame).forget();
    } else {
      next_frame();
    };
  }) as Box<dyn FnMut()>);
  *g.borrow_mut() = Some(closure);
  *t.borrow_mut() = now();
  request_animation_frame(g.borrow().as_ref().unwrap());
}
