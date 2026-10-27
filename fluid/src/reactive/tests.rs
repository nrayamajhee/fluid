use super::*;

/// Counts calls, shareable into closures.
#[derive(Clone, Default)]
struct Calls(Rc<Cell<usize>>);

impl Calls {
  fn hit(&self) {
    self.0.set(self.0.get() + 1);
  }
  fn get(&self) -> usize {
    self.0.get()
  }
}

fn log() -> Rc<RefCell<Vec<String>>> {
  Rc::default()
}

#[test]
fn effect_runs_now_and_on_every_write() {
  let ctx = Context::new();
  let count = ctx.create_signal(0);
  let seen = log();
  {
    let (count, seen) = (count.clone(), seen.clone());
    ctx.create_effect(move || seen.borrow_mut().push(count.value().to_string()));
  }
  count.set(1);
  count.set(1);
  count.set(2);
  assert_eq!(*seen.borrow(), ["0", "1", "1", "2"]);
}

#[test]
fn set_if_changed_skips_equal_values() {
  let count = Rc::new(Signal::new(0));
  let calls = Calls::default();
  {
    let (count, calls) = (count.clone(), calls.clone());
    create_effect(move || {
      count.value();
      calls.hit();
    });
  }
  assert!(!count.set_if_changed(0));
  assert!(count.set_if_changed(3));
  assert_eq!(calls.get(), 2);
}

#[test]
fn update_with_value_and_cloned() {
  let list = Signal::new(vec![1, 2]);
  list.update(|list| list.push(3));
  assert_eq!(list.with(|list| list.len()), 3);
  assert_eq!(list.cloned(), [1, 2, 3]);
  let flag = Signal::new(true);
  assert!(flag.value());
}

#[test]
#[should_panic(expected = "while a `Ref` from `get()` was alive")]
fn writing_while_borrowed_explains_itself() {
  let count = Signal::new(0);
  let held = count.get();
  count.set(*held + 1);
}

#[test]
fn update_reading_its_own_signal_does_not_panic() {
  let count = Rc::new(Signal::new(1));
  {
    let count = count.clone();
    create_effect(move || {
      count.value();
    });
  }
  // `with` releases its borrow before the write.
  let next = count.with(|count| count + 1);
  count.set(next);
  assert_eq!(count.value(), 2);
}

#[test]
fn batch_runs_each_effect_once_after_all_writes() {
  let a = Rc::new(Signal::new(1));
  let b = Rc::new(Signal::new(1));
  let seen = log();
  {
    let (a, b, seen) = (a.clone(), b.clone(), seen.clone());
    create_effect(move || seen.borrow_mut().push(format!("{}", a.value() + b.value())));
  }
  batch(|| {
    a.set(10);
    b.set(20);
  });
  assert_eq!(*seen.borrow(), ["2", "30"]);
}

/// studio's `select_preset`: the selected index must be valid for the preset
/// whenever an effect reads both.
#[test]
fn batch_avoids_reading_an_index_mid_update() {
  let presets: Rc<Vec<Vec<&str>>> = Rc::new(vec![vec!["a", "b", "c"], vec!["x"]]);
  let preset = Rc::new(Signal::new(0usize));
  let selected = Rc::new(Signal::new(2usize));
  let seen = log();
  {
    let (preset, selected, seen) = (preset.clone(), selected.clone(), seen.clone());
    create_effect(move || {
      seen
        .borrow_mut()
        .push(presets[preset.value()][selected.value()].to_string())
    });
  }
  batch(|| {
    preset.set(1);
    selected.set(0);
  });
  assert_eq!(*seen.borrow(), ["c", "x"]);
}

#[test]
fn batch_returns_its_value_and_nests() {
  let a = Rc::new(Signal::new(0));
  let calls = Calls::default();
  {
    let (a, calls) = (a.clone(), calls.clone());
    create_effect(move || {
      a.value();
      calls.hit();
    });
  }
  let result = batch(|| {
    a.set(1);
    batch(|| a.set(2));
    assert_eq!(calls.get(), 1, "inner batch must not flush");
    "done"
  });
  assert_eq!(result, "done");
  assert_eq!(calls.get(), 2);
}

#[test]
fn diamond_is_glitch_free() {
  let a = Rc::new(Signal::new(1));
  let double = {
    let a = a.clone();
    create_memo(move || a.value() * 2)
  };
  let triple = {
    let a = a.clone();
    create_memo(move || a.value() * 3)
  };
  let seen = log();
  {
    let (a, double, triple, seen) = (a.clone(), double.clone(), triple.clone(), seen.clone());
    create_effect(move || {
      seen.borrow_mut().push(format!(
        "{} {} {}",
        a.value(),
        double.value(),
        triple.value()
      ))
    });
  }
  a.set(2);
  a.set(3);
  assert_eq!(*seen.borrow(), ["1 2 3", "2 4 6", "3 6 9"]);
}

#[test]
fn memo_cuts_off_unchanged_results() {
  let count = Rc::new(Signal::new(0));
  let computed = Calls::default();
  let even = {
    let (count, computed) = (count.clone(), computed.clone());
    create_memo(move || {
      computed.hit();
      count.value() % 2 == 0
    })
  };
  let calls = Calls::default();
  {
    let (even, calls) = (even.clone(), calls.clone());
    create_effect(move || {
      even.value();
      calls.hit();
    });
  }
  count.set(2);
  count.set(4);
  assert_eq!(calls.get(), 1);
  assert_eq!(computed.get(), 3);
  count.set(5);
  assert_eq!(calls.get(), 2);
  assert!(!even.value());
}

#[test]
fn chained_memos_update_in_order() {
  let a = Rc::new(Signal::new(1));
  let b = {
    let a = a.clone();
    create_memo(move || a.value() + 1)
  };
  let c = {
    let b = b.clone();
    create_memo(move || b.value() * 10)
  };
  let seen = log();
  {
    let (a, c, seen) = (a.clone(), c.clone(), seen.clone());
    create_effect(move || {
      seen
        .borrow_mut()
        .push(format!("{} {}", a.value(), c.value()))
    });
  }
  a.set(5);
  assert_eq!(*seen.borrow(), ["1 20", "5 60"]);
}

#[test]
fn memo_read_inside_a_batch_is_fresh() {
  let a = Rc::new(Signal::new(1));
  let double = {
    let a = a.clone();
    create_memo(move || a.value() * 2)
  };
  batch(|| {
    a.set(4);
    assert_eq!(double.value(), 8);
  });
}

#[test]
fn effects_retrack_their_dependencies() {
  let flag = Rc::new(Signal::new(true));
  let a = Rc::new(Signal::new(0));
  let b = Rc::new(Signal::new(0));
  let calls = Calls::default();
  {
    let (flag, a, b, calls) = (flag.clone(), a.clone(), b.clone(), calls.clone());
    create_effect(move || {
      calls.hit();
      if flag.value() {
        a.value();
      } else {
        b.value();
      }
    });
  }
  flag.set(false);
  assert_eq!(calls.get(), 2);
  a.set(1);
  assert_eq!(calls.get(), 2, "no longer reads `a`");
  b.set(1);
  assert_eq!(calls.get(), 3);
}

#[test]
fn writes_inside_an_effect_run_after_it() {
  let source = Rc::new(Signal::new(1));
  let derived = Rc::new(Signal::new(0));
  let seen = log();
  {
    let (derived, seen) = (derived.clone(), seen.clone());
    create_effect(move || {
      seen
        .borrow_mut()
        .push(format!("derived {}", derived.value()))
    });
  }
  {
    let (source, derived, seen) = (source.clone(), derived.clone(), seen.clone());
    create_effect(move || {
      let value = source.value() * 100;
      derived.set(value);
      seen.borrow_mut().push(format!("wrote {value}"));
    });
  }
  source.set(2);
  assert_eq!(
    *seen.borrow(),
    [
      "derived 0",
      "wrote 100",
      "derived 100",
      "wrote 200",
      "derived 200"
    ]
  );
}

#[test]
fn an_effect_may_write_what_it_reads_until_it_settles() {
  let count = Rc::new(Signal::new(0));
  {
    let count = count.clone();
    create_effect(move || {
      let value = count.value();
      if value < 5 {
        count.set(value + 1);
      }
    });
  }
  assert_eq!(count.value(), 5);
}

#[test]
#[should_panic(expected = "an effect probably writes a signal it reads")]
fn runaway_effects_panic() {
  let count = Rc::new(Signal::new(0));
  let count2 = count.clone();
  create_effect(move || {
    let value = count2.value();
    count2.set(value + 1);
  });
}

#[test]
fn untrack_does_not_subscribe() {
  let a = Rc::new(Signal::new(0));
  let b = Rc::new(Signal::new(0));
  let calls = Calls::default();
  {
    let (a, b, calls) = (a.clone(), b.clone(), calls.clone());
    create_effect(move || {
      a.value();
      untrack(|| b.value());
      b.get_untracked();
      calls.hit();
    });
  }
  b.set(1);
  assert_eq!(calls.get(), 1);
  a.set(1);
  assert_eq!(calls.get(), 2);
}

#[test]
fn legacy_get_and_set_still_work() {
  let ctx = Context::new();
  let counter = ctx.create_signal(0);
  let seen = log();
  {
    let (counter, seen) = (counter.clone(), seen.clone());
    ctx.create_effect(move || seen.borrow_mut().push(counter.get().to_string()));
  }
  let next = *counter.get() + 1;
  counter.set(next);
  assert_eq!(*seen.borrow(), ["0", "1"]);
}

fn counting_effect(ctx: &Context, signal: &Rc<Signal<i32>>, calls: &Calls) {
  let (signal, calls) = (signal.clone(), calls.clone());
  ctx.create_effect(move || {
    signal.value();
    calls.hit();
  });
}

#[test]
fn dispose_stops_effects_and_runs_cleanups_in_reverse_once() {
  let ctx = Context::new();
  let count = ctx.create_signal(0);
  let calls = Calls::default();
  counting_effect(&ctx, &count, &calls);
  let seen = log();
  for name in ["first", "second"] {
    let seen = seen.clone();
    ctx.on_cleanup(move || seen.borrow_mut().push(name.to_string()));
  }
  ctx.dispose();
  ctx.dispose();
  count.set(1);
  assert_eq!(calls.get(), 1);
  assert_eq!(*seen.borrow(), ["second", "first"]);
  assert!(ctx.is_disposed());
}

#[test]
fn dropping_a_context_keeps_its_effects() {
  let count = Rc::new(Signal::new(0));
  let calls = Calls::default();
  {
    let ctx = Context::new();
    counting_effect(&ctx, &count, &calls);
  }
  count.set(1);
  assert_eq!(calls.get(), 2);
}

#[test]
fn child_scopes_dispose_alone_or_with_their_parent() {
  let root = Context::new();
  let count = Rc::new(Signal::new(0));
  let (a, b, c) = (Calls::default(), Calls::default(), Calls::default());
  counting_effect(&root, &count, &a);
  let first = root.child();
  counting_effect(&first, &count, &b);
  let second = root.child();
  counting_effect(&second, &count, &c);

  first.dispose();
  count.set(1);
  assert_eq!((a.get(), b.get(), c.get()), (2, 1, 2));
  root.dispose();
  count.set(2);
  assert_eq!((a.get(), b.get(), c.get()), (2, 1, 2));
  assert!(second.is_disposed());
}

#[test]
fn effects_in_a_disposed_scope_never_run() {
  let ctx = Context::new();
  ctx.dispose();
  let calls = Calls::default();
  counting_effect(&ctx, &Rc::new(Signal::new(0)), &calls);
  assert_eq!(calls.get(), 0);
  let memo = ctx.create_memo(|| 7);
  assert_eq!(memo.value(), 7);
}

#[test]
fn cleanups_run_before_an_effect_reruns() {
  let count = Rc::new(Signal::new(0));
  let seen = log();
  {
    let (count, seen) = (count.clone(), seen.clone());
    create_effect(move || {
      let value = count.value();
      seen.borrow_mut().push(format!("run {value}"));
      let seen = seen.clone();
      on_cleanup(move || seen.borrow_mut().push(format!("clean {value}")));
    });
  }
  count.set(1);
  assert_eq!(*seen.borrow(), ["run 0", "clean 0", "run 1"]);
}

#[test]
fn nested_effects_are_disposed_when_the_outer_one_reruns() {
  let ctx = Context::new();
  let outer = ctx.create_signal(0);
  let inner = ctx.create_signal(0);
  let calls = Calls::default();
  {
    let (ctx2, outer, inner, calls) = (ctx.clone(), outer.clone(), inner.clone(), calls.clone());
    ctx.create_effect(move || {
      outer.value();
      // Created through the outer `ctx`, as html! does: still owned by this run.
      counting_effect(&ctx2, &inner, &calls);
    });
  }
  outer.set(1);
  outer.set(2);
  assert_eq!(calls.get(), 3);
  inner.set(1);
  assert_eq!(calls.get(), 4, "only the latest inner effect is alive");
}

#[test]
fn memos_are_owned_and_stop_when_disposed() {
  let ctx = Context::new();
  let count = ctx.create_signal(1);
  let computed = Calls::default();
  let double = {
    let (count, computed) = (count.clone(), computed.clone());
    ctx.create_memo(move || {
      computed.hit();
      count.value() * 2
    })
  };
  ctx.dispose();
  count.set(5);
  assert_eq!(double.value(), 2);
  assert_eq!(computed.get(), 1);
}

#[test]
fn context_values_are_found_up_the_chain() {
  #[derive(Clone, PartialEq, Debug)]
  struct Theme(&'static str);

  let root = Context::new();
  root.provide_context(Theme("dark"));
  root.provide_context(Rc::new(5u8));
  let child = root.child();
  assert_eq!(child.use_context::<Theme>(), Some(Theme("dark")));
  assert_eq!(child.use_context::<Rc<u8>>().as_deref(), Some(&5));
  assert_eq!(child.use_context::<String>(), None);

  let grandchild = child.child();
  grandchild.provide_context(Theme("light"));
  assert_eq!(grandchild.use_context::<Theme>(), Some(Theme("light")));
  assert_eq!(child.use_context::<Theme>(), Some(Theme("dark")));

  assert_eq!(use_context::<Theme>(), None, "no current scope");
  let found = grandchild.run(|| use_context::<Theme>());
  assert_eq!(found, Some(Theme("light")));
  let seen = Rc::new(RefCell::new(None));
  {
    let seen = seen.clone();
    child.run(|| create_effect(move || *seen.borrow_mut() = use_context::<Theme>()));
  }
  assert_eq!(*seen.borrow(), Some(Theme("dark")));
}

#[test]
fn run_makes_free_functions_use_the_scope() {
  let ctx = Context::new();
  let count = ctx.create_signal(0);
  let calls = Calls::default();
  let cleaned = Calls::default();
  ctx.run(|| {
    let (count, calls) = (count.clone(), calls.clone());
    create_effect(move || {
      count.value();
      calls.hit();
    });
    let cleaned = cleaned.clone();
    on_cleanup(move || cleaned.hit());
    assert!(has_owner());
  });
  assert!(!has_owner());
  ctx.dispose();
  count.set(1);
  assert_eq!((calls.get(), cleaned.get()), (1, 1));
}

#[test]
fn an_effect_may_dispose_its_own_scope() {
  let ctx = Context::new();
  let count = ctx.create_signal(0);
  let calls = Calls::default();
  {
    let (scope, count, calls) = (ctx.clone(), count.clone(), calls.clone());
    ctx.create_effect(move || {
      calls.hit();
      if count.value() == 1 {
        scope.dispose();
      }
    });
  }
  count.set(1);
  count.set(2);
  assert_eq!(calls.get(), 2);
}

#[test]
fn disposed_effects_unsubscribe() {
  let ctx = Context::new();
  let count = Rc::new(Signal::new(0));
  counting_effect(&ctx, &count, &Calls::default());
  assert_eq!(count.source.observers.borrow().len(), 1);
  ctx.dispose();
  assert_eq!(count.source.observers.borrow().len(), 0);
}
