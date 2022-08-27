importScripts("pkg/fluid_example.js");
self.onmessage = (event) => {
  let initialised = wasm_bindgen(...event.data).catch((err) => {
    setTimeout(() => {
      throw err;
    });
    throw err;
  });
  self.onmessage = async (event) => {
    await initialised;
    wasm_bindgen.child_entry_point(event.data);
  };
};
