const context = [];

function getCurrentObserver() {
  return context[context.length - 1];
}

function createSignal(value) {
  const subscribers = new Set();
  const read = () => {
    const current = getCurrentObserver();
    if (current) subscribers.add(current);
    return value;
  };
  const write = (nextValue) => {
    value = nextValue;
    for (const sub of subscribers) {
      sub();
    }
  };
  return [read, write];
}

function createEffect(fn) {
  const execute = () => {
    context.push(execute);
    try {
      fn();
    } finally {
      context.pop();
    }
  };
  execute();
}

const [count, setCount] = createSignal(0);

//setInterval(() => {
//  setCount(count() + 1);
//}, 2000);

createEffect(() => {
  console.log(count());
});

setCount(1);
setCount(2);
setCount(3);
