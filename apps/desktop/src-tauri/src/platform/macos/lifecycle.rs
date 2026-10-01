//! Native sleep/wake observation through IOKit root power domain notifications.
use crate::state::Request;
use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{self, Sender},
    },
};

type IONotificationPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFStringRef = *const c_void;
type InterestCallback =
    unsafe extern "C" fn(refcon: *mut c_void, service: u32, message: u32, argument: *mut c_void);

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    fn IORegisterForSystemPower(
        refcon: *mut c_void,
        port: *mut IONotificationPortRef,
        callback: InterestCallback,
        notifier: *mut u32,
    ) -> u32;
    fn IODeregisterForSystemPower(notifier: *mut u32) -> i32;
    fn IOAllowPowerChange(kernel_port: u32, notification_id: isize) -> i32;
    fn IONotificationPortGetRunLoopSource(port: IONotificationPortRef) -> CFRunLoopSourceRef;
    fn IONotificationPortDestroy(port: IONotificationPortRef);
    fn IOServiceClose(connection: u32) -> i32;
}
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    static kCFRunLoopDefaultMode: CFStringRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(run_loop: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRunLoopStop(run_loop: CFRunLoopRef);
}

// iokit_common_msg values from IOKit/IOMessage.h.
const CAN_SYSTEM_SLEEP: u32 = 0xe000_0270;
const SYSTEM_WILL_SLEEP: u32 = 0xe000_0280;
const SYSTEM_HAS_POWERED_ON: u32 = 0xe000_0300;

struct Context {
    sender: Sender<Request>,
    root_port: AtomicU32,
}

unsafe extern "C" fn changed(refcon: *mut c_void, _: u32, message: u32, argument: *mut c_void) {
    // The context outlives the registration; the observer thread frees it after deregistering.
    let context = &*(refcon as *const Context);
    match message {
        // Idle sleep is the system's decision; Doze's own assertion already vetoes it while awake.
        CAN_SYSTEM_SLEEP => {
            IOAllowPowerChange(context.root_port.load(Ordering::Acquire), argument as isize);
        }
        SYSTEM_WILL_SLEEP => {
            let _ = context.sender.send(Request::Lifecycle);
            IOAllowPowerChange(context.root_port.load(Ordering::Acquire), argument as isize);
        }
        SYSTEM_HAS_POWERED_ON => {
            let _ = context.sender.send(Request::Lifecycle);
        }
        _ => {}
    }
}

struct RunLoop(CFRunLoopRef);
// CFRunLoopStop is documented as safe to call from any thread.
unsafe impl Send for RunLoop {}

pub struct Registration {
    run_loop: RunLoop,
}

impl Registration {
    pub fn new(sender: Sender<Request>) -> Result<Self, String> {
        let (ready, registered) = mpsc::channel();
        std::thread::Builder::new()
            .name("doze-power-observer".into())
            .spawn(move || unsafe {
                let context = Box::into_raw(Box::new(Context {
                    sender,
                    root_port: AtomicU32::new(0),
                }));
                let mut port = std::ptr::null_mut();
                let mut notifier = 0;
                let root = IORegisterForSystemPower(
                    context.cast(),
                    &mut port,
                    changed,
                    &mut notifier,
                );
                if root == 0 {
                    drop(Box::from_raw(context));
                    let _ = ready.send(Err("IORegisterForSystemPower failed.".to_string()));
                    return;
                }
                (*context).root_port.store(root, Ordering::Release);
                let run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(
                    run_loop,
                    IONotificationPortGetRunLoopSource(port),
                    kCFRunLoopDefaultMode,
                );
                let _ = ready.send(Ok(RunLoop(run_loop)));
                CFRunLoopRun();
                IODeregisterForSystemPower(&mut notifier);
                IOServiceClose(root);
                IONotificationPortDestroy(port);
                drop(Box::from_raw(context));
            })
            .map_err(|error| error.to_string())?;
        match registered.recv() {
            Ok(Ok(run_loop)) => Ok(Self { run_loop }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err("Power observer thread stopped.".into()),
        }
    }
}

impl Drop for Registration {
    // The observer thread deregisters and frees its context once its run loop stops. It is not
    // joined: a stop that arrives before the loop starts must not block quitting.
    fn drop(&mut self) {
        unsafe { CFRunLoopStop(self.run_loop.0) };
    }
}
