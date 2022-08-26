use fluid::Context;
use std::rc::Rc;

fn main() {
    let ctx = Context::new();
    let a = ctx.create_signal("Hello");
    let a = Rc::new(a);
    {
        let a = a.clone();
        ctx.create_effect(move || {
            println!("Effect 1: {}", a.get());
        });
    }
    ctx.create_effect(move || {
        println!("Effect Mid");
    });
    {
        let a = a.clone();
        ctx.create_effect(move || {
            println!("Effect 2: {}", a.get());
        });
    }
    a.set("Hi");
    a.set("There!");
}
