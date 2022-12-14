#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(js_namespace = console, js_name = log)]
  fn logv(x: &JsValue);
}

pub mod pool;
mod start;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use crate::pool::RayonWorkers;
use log::Level;

use rayon::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub async fn run() -> Result<(), JsValue> {
  console_error_panic_hook::set_once();
  console_log::init_with_level(Level::Debug).unwrap_or_else(|e| {
    crate::logv(&JsValue::from_str(&format!(
      "Couldn't initialize logger:\n{}",
      e,
    )))
  });

  let workers = RayonWorkers::new(None);
  workers.run(move || {
    let sum = (1..=1000).into_par_iter().sum::<usize>();
    log::info!("Raypon par iter test: {:?}", sum);
  });
  start::start().await
}
