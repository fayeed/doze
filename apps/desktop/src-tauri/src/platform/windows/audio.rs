use crate::platform::AudioMonitor;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use windows::{
    core::{implement, PCWSTR},
    Win32::{
        Media::Audio::{
            Endpoints::{IAudioEndpointVolume, IAudioMeterInformation},
            *,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
        },
        UI::Shell::PropertiesSystem::PROPERTYKEY,
    },
};

#[implement(IMMNotificationClient)]
struct DeviceEvents(Arc<AtomicBool>);
#[allow(non_snake_case)]
impl IMMNotificationClient_Impl for DeviceEvents_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        _: EDataFlow,
        _: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        self.0.store(true, Ordering::Release);
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

pub struct NativeAudio {
    enumerator: IMMDeviceEnumerator,
    listener: IMMNotificationClient,
    dirty: Arc<AtomicBool>,
    meters: Vec<(IAudioMeterInformation, IAudioEndpointVolume)>,
    _apartment: Apartment,
}
struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
impl NativeAudio {
    pub fn new() -> Result<Self, String> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|e| e.to_string())?;
        }
        let result = (|| -> windows::core::Result<Self> {
            let enumerator: IMMDeviceEnumerator =
                unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
            let dirty = Arc::new(AtomicBool::new(true));
            let listener: IMMNotificationClient = DeviceEvents(dirty.clone()).into();
            unsafe {
                enumerator.RegisterEndpointNotificationCallback(&listener)?;
            }
            Ok(Self {
                enumerator,
                listener,
                dirty,
                meters: Vec::new(),
                _apartment: Apartment,
            })
        })();
        match result {
            Ok(audio) => Ok(audio),
            Err(e) => {
                unsafe {
                    CoUninitialize();
                }
                Err(e.to_string())
            }
        }
    }
    fn rebuild(&mut self) -> Result<(), String> {
        self.meters.clear();
        unsafe {
            let devices = self
                .enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
                .map_err(|e| e.to_string())?;
            for i in 0..devices.GetCount().map_err(|e| e.to_string())? {
                let device = devices.Item(i).map_err(|e| e.to_string())?;
                let meter = device
                    .Activate::<IAudioMeterInformation>(CLSCTX_ALL, None)
                    .map_err(|e| e.to_string())?;
                let volume = device
                    .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                    .map_err(|e| e.to_string())?;
                self.meters.push((meter, volume));
            }
        }
        Ok(())
    }
}
impl AudioMonitor for NativeAudio {
    fn sample(&mut self) -> Result<bool, String> {
        if self.dirty.swap(false, Ordering::AcqRel) {
            if let Err(e) = self.rebuild() {
                self.dirty.store(true, Ordering::Release);
                return Err(e);
            }
            return Err("Audio devices changed; waiting for fresh playback.".into());
        }
        if self.meters.is_empty() {
            return Err("No active audio output device.".into());
        }
        let result = (|| -> windows::core::Result<bool> {
            let mut playing = false;
            for (meter, volume) in &self.meters {
                unsafe {
                    // Endpoint meters are pre-volume: explicitly account for mute and output level.
                    let level = if volume.GetMute()?.as_bool() {
                        0.0
                    } else {
                        volume.GetMasterVolumeLevelScalar()?
                    };
                    playing |= meter.GetPeakValue()? * level > 0.001; // -60 dBFS; silent sessions do not arm.
                }
            }
            Ok(playing)
        })();
        result.map_err(|e| {
            self.dirty.store(true, Ordering::Release);
            e.to_string()
        })
    }
}
impl Drop for NativeAudio {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .enumerator
                .UnregisterEndpointNotificationCallback(&self.listener);
        }
        self.meters.clear();
        // COM interface fields release after Drop; do not uninitialize before they release.
    }
}
