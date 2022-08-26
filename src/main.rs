use fluid::Context;

fn main() {
    let mut context = Context::new();
    let (state, set_state) = context.create_signal("Hello");
    context.create_effect(move || {
        dbg!(state());
    });
    set_state("Hi");
    set_state("There!");
}
