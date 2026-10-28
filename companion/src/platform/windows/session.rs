//! Whether the session is locked, and the default microphone (Core Audio's
//! communications capture device, the one calls use).

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, AudioSessionStateActive, IAudioSessionManager2, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, OpenInputDesktop, SwitchDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_SWITCHDESKTOP,
};

use crate::platform::Mic;

/// While the lock screen is up, the input desktop is Winlogon's, which a
/// user process can't open or switch to.
pub fn session_locked() -> Option<bool> {
    unsafe {
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_SWITCHDESKTOP) {
            Ok(desk) => {
                let usable = SwitchDesktop(desk).is_ok();
                let _ = CloseDesktop(desk);
                Some(!usable)
            }
            Err(_) => Some(true),
        }
    }
}

fn capture_device() -> Option<IMMDevice> {
    super::winrt_init(); // COM, multithreaded
    unsafe {
        let devices: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
        devices.GetDefaultAudioEndpoint(eCapture, eCommunications).ok()
    }
}

/// None without a microphone.
pub fn mic() -> Option<Mic> {
    let device = capture_device()?;
    unsafe {
        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).ok()?;
        let muted = volume.GetMute().ok()?.as_bool();
        // An app recording from it has an active session on it.
        let in_use = (|| -> windows::core::Result<bool> {
            let manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
            let sessions = manager.GetSessionEnumerator()?;
            for i in 0..sessions.GetCount()? {
                if sessions.GetSession(i)?.GetState()? == AudioSessionStateActive {
                    return Ok(true);
                }
            }
            Ok(false)
        })()
        .unwrap_or(false);
        Some(Mic { in_use, muted })
    }
}

pub fn toggle_mic() {
    let Some(device) = capture_device() else { return };
    unsafe {
        if let Ok(volume) = device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) {
            if let Ok(muted) = volume.GetMute() {
                let _ = volume.SetMute(!muted.as_bool(), std::ptr::null());
            }
        }
    }
}
