pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

use std::collections::HashSet;

use std::cell::{Ref, RefCell};
use std::rc::Rc;

pub struct Context {
    subscriptions: Rc<RefCell<Vec<Rc<dyn Fn()>>>>,
}

impl Context {
    pub fn new() -> Self {
        Self {
            subscriptions: Rc::new(RefCell::new(Vec::new())),
        }
    }
    pub fn create_signal<T: Clone>(&self, value: T) -> (impl Fn() -> T, impl Fn(T)) {
        let state_subscriptions: Rc<RefCell<Vec<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(Vec::new()));
        let inner = Rc::new(RefCell::new(value));
        let i = inner.clone();
        let c_s = self.subscriptions.clone();
        let s_s = state_subscriptions.clone();
        let read = move || {
            if let Some(sub) = c_s.borrow_mut().pop() {
                s_s.borrow_mut().push(sub);
            }
            // Removing this clone and returning i.borrow() was a lifetime nightmare
            // Please help!
            i.borrow().clone()
        };
        let write = move |new_val| {
            *inner.borrow_mut() = new_val;
            for sub in state_subscriptions.borrow().iter() {
                sub();
            }
        };
        (read, write)
    }
    pub fn create_effect(&mut self, closure: impl Fn() + 'static) {
        let closure = Rc::new(closure);
        self.subscriptions.borrow_mut().push(closure.clone());
        closure();
    }
}
