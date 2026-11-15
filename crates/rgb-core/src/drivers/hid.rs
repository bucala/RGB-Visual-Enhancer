//! A single hidapi context shared by every USB HID detector, so devices are
//! enumerated once per scan pass and never from two threads at a time.

use std::sync::{Arc, Mutex};

use hidapi::HidApi;

#[derive(Clone, Default)]
pub struct SharedHidApi(Arc<Mutex<Option<HidApi>>>);

impl SharedHidApi {
    /// Run `f` with a freshly refreshed device list. Blocking: call it from
    /// `spawn_blocking`.
    pub fn with_refreshed<T>(
        &self,
        f: impl FnOnce(&mut HidApi, &mut Vec<String>) -> T,
    ) -> Result<T, String> {
        let mut guard = self
            .0
            .lock()
            .map_err(|_| "HID context poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(HidApi::new().map_err(|e| format!("cannot initialise hidapi: {e}"))?);
        }
        let api = guard.as_mut().expect("initialised above");
        let mut warnings = Vec::new();
        if let Err(e) = api.refresh_devices() {
            warnings.push(format!("HID enumeration failed: {e}"));
        }
        let out = f(api, &mut warnings);
        // Callers that care about warnings collect them inside `f`.
        for w in warnings {
            log::warn!("{w}");
        }
        Ok(out)
    }
}
