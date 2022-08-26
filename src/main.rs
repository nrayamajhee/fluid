use fluid::Context;
use std::rc::Rc;

fn main() {
    let ctx = Context::new();
    let a = ctx.create_signal("Hello");
    let a = Rc::new(a);
    {
        let a = a.clone();
        ctx.create_effect(move || {
            println!("Effect One: {}", a.get());
        });
    }
    {
        let a = a.clone();
        ctx.create_effect(move || {
            println!("Effect Two: {}", a.get());
        });
    }
    a.set("Hi");
    a.set("There!");
}
