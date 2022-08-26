use std::cell::{Ref, RefCell};
use std::rc::Rc;

pub struct Signal<T> {
    inner: RefCell<T>,
    subscribers: RefCell<Vec<Rc<dyn Fn()>>>,
    context: Rc<RefCell<Vec<Rc<dyn Fn()>>>>,
}

impl<T> Signal<T> {
    pub fn get(&self) -> Ref<'_, T> {
        if let Some(sub) = self.context.borrow_mut().pop() {
            self.subscribers.borrow_mut().push(sub.clone());
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
    pub fn create_signal<T>(&self, value: T) -> Signal<T> {
        Signal {
            inner: RefCell::new(value),
            subscribers: RefCell::new(Vec::new()),
            context: self.observers.clone(),
        }
    }
    pub fn create_effect(&self, closure: impl Fn() + 'static) {
        let closure = Rc::new(closure);
        self.observers.borrow_mut().push(closure.clone());
        closure();
    }
}
