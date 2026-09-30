use crate::state::Request;
use std::{ffi::c_void, sync::mpsc::Sender};
use windows::Win32::{
    Foundation::HANDLE,
    System::Power::{
        PowerRegisterSuspendResumeNotification, PowerUnregisterSuspendResumeNotification,
        DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS, HPOWERNOTIFY,
    },
    UI::WindowsAndMessaging::DEVICE_NOTIFY_CALLBACK,
};

pub struct Registration {
    handle: *mut c_void,
    subscription: Option<Box<Subscription>>,
}
struct Subscription {
    sender: Sender<Request>,
    parameters: DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS,
}
unsafe extern "system" fn changed(
    context: *const c_void,
    _kind: u32,
    _setting: *const c_void,
) -> u32 {
    // Context remains allocated until the native registration is unregistered.
    let sender = &*(context as *const Sender<Request>);
    let _ = sender.send(Request::Lifecycle);
    0
}
impl Registration {
    pub fn new(sender: Sender<Request>) -> Result<Self, String> {
        // Both the callback parameters and their context have stable heap addresses.
        let mut subscription = Box::new(Subscription {
            sender,
            parameters: DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS::default(),
        });
        subscription.parameters.Callback = Some(changed);
        subscription.parameters.Context = (&mut subscription.sender as *mut Sender<Request>).cast();
        let mut handle = std::ptr::null_mut();
        unsafe {
            PowerRegisterSuspendResumeNotification(
                DEVICE_NOTIFY_CALLBACK,
                HANDLE(
                    (&subscription.parameters as *const DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS)
                        .cast_mut()
                        .cast(),
                ),
                &mut handle,
            )
            .ok()
            .map_err(|e| e.to_string())?;
        }
        Ok(Self {
            handle,
            subscription: Some(subscription),
        })
    }
}
impl Drop for Registration {
    fn drop(&mut self) {
        let result =
            unsafe { PowerUnregisterSuspendResumeNotification(HPOWERNOTIFY(self.handle as isize)) };
        if result.is_err() {
            // A failed unregister may leave callbacks alive. Preserve their context until exit.
            if let Some(subscription) = self.subscription.take() {
                Box::leak(subscription);
            }
        }
    }
}
