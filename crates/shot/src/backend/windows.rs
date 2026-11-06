use std::{mem, ptr};

use windows_sys::Win32::{
    Foundation::{BOOL, LPARAM, RECT},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC,
        DIB_RGB_COLORS, DeleteDC, DeleteObject, EnumDisplayMonitors, GetDC, GetDIBits,
        GetMonitorInfoW, HBITMAP, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW, ReleaseDC, SRCCOPY,
        SelectObject,
    },
    UI::WindowsAndMessaging::SetProcessDPIAware,
};

use crate::{
    backend::{Backend, BackendTarget, PositionedFrame, stitch_positioned},
    frame::Frame,
};

pub struct WindowsBackend {
    monitors: Vec<Monitor>,
    selected_monitor: Option<usize>,
}

#[derive(Clone)]
struct Monitor {
    name: String,
    rect: RECT,
}

impl Backend for WindowsBackend {
    fn initialize(target: BackendTarget) -> crate::Result<Self> {
        Self::initialize_with_monitors(match target {
            BackendTarget::Monitor(index) => Some(index),
            BackendTarget::All => None,
        })
    }

    fn capture_output(&mut self) -> crate::Result<Frame> {
        let index = self.selected_monitor.unwrap_or(0);
        let monitor = self
            .monitors
            .get(index)
            .cloned()
            .ok_or(crate::Error::NoMonitors)?;
        capture_monitor(&monitor.rect)
    }

    fn capture_all_outputs(&mut self) -> crate::Result<Frame> {
        let mut frames = Vec::with_capacity(self.monitors.len());

        for monitor in self.monitors.clone() {
            frames.push(PositionedFrame {
                frame: capture_monitor(&monitor.rect)?,
                x: monitor.rect.left,
                y: monitor.rect.top,
            });
        }

        stitch_positioned(&frames)
    }

    fn monitor_count(&self) -> usize {
        self.monitors.len()
    }

    fn monitor_names(&self) -> Vec<String> {
        self.monitors
            .iter()
            .map(|monitor| monitor.name.clone())
            .collect()
    }

    fn select_monitor(&mut self, monitor_index: usize) -> crate::Result<()> {
        if monitor_index >= self.monitors.len() {
            return Err(crate::Error::MonitorOutOfRange {
                index: monitor_index,
                count: self.monitors.len(),
            });
        }

        self.selected_monitor = Some(monitor_index);
        Ok(())
    }
}

impl WindowsBackend {
    fn initialize_with_monitors(monitor_index: Option<usize>) -> crate::Result<Self> {
        // Prevent Windows DPI virtualization from changing the coordinates returned
        // by GetMonitorInfoW relative to the pixels captured by GDI.
        // SAFETY: SetProcessDPIAware has no pointer or handle preconditions.
        unsafe {
            SetProcessDPIAware();
        }

        let mut monitors = Vec::new();
        // SAFETY: The callback receives a live, exclusive Vec pointer, used only
        // synchronously during enumeration. Null arguments select every monitor.
        unsafe {
            EnumDisplayMonitors(
                ptr::null_mut(),
                ptr::null(),
                Some(enumerate_monitor),
                &raw mut monitors as LPARAM,
            );
        }

        if monitors.is_empty() {
            return Err(crate::Error::NoMonitors);
        }

        if let Some(index) = monitor_index
            && index >= monitors.len()
        {
            return Err(crate::Error::MonitorOutOfRange {
                index,
                count: monitors.len(),
            });
        }

        Ok(Self {
            monitors,
            selected_monitor: monitor_index,
        })
    }
}

/// # Safety
/// `data` must be the exclusive Vec pointer supplied to EnumDisplayMonitors.
unsafe extern "system" fn enumerate_monitor(
    monitor: HMONITOR,
    _dc: HDC,
    _rect: *mut RECT,
    data: LPARAM,
) -> BOOL {
    // SAFETY: Enumeration synchronously forwards the live exclusive Vec pointer.
    let monitors = unsafe { &mut *(data as *mut Vec<Monitor>) };
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: u32::try_from(mem::size_of::<MONITORINFOEXW>())
                .expect("Windows header size fits in u32"),
            // SAFETY: MONITORINFO consists of plain C integer fields.
            ..unsafe { mem::zeroed() }
        },
        szDevice: [0; 32],
    };

    // SAFETY: The callback receives a valid monitor; info has its required size.
    if unsafe { GetMonitorInfoW(monitor, &raw mut info.monitorInfo) } == 0 {
        return 1;
    }

    let name = String::from_utf16_lossy(
        &info.szDevice[..info
            .szDevice
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(info.szDevice.len())],
    );
    monitors.push(Monitor {
        name,
        rect: info.monitorInfo.rcMonitor,
    });
    1
}

fn capture_monitor(rect: &RECT) -> crate::Result<Frame> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    if width <= 0 || height <= 0 {
        return Err(crate::Error::WindowsCapture(
            "monitor has no area".to_string(),
        ));
    }

    // SAFETY: A null HWND requests the desktop device context.
    let screen_dc = unsafe { GetDC(ptr::null_mut()) };

    if screen_dc.is_null() {
        return Err(crate::Error::WindowsCapture("GetDC failed".to_string()));
    }

    // SAFETY: The checked DC stays live throughout capture, which receives positive dimensions.
    let result = unsafe { capture_with_dc(screen_dc, rect, width, height) };
    // SAFETY: This releases the desktop DC obtained above exactly once.
    unsafe { ReleaseDC(ptr::null_mut(), screen_dc) };
    result
}

/// # Safety
/// `screen_dc` must remain live; width and height must be positive pixel dimensions.
unsafe fn capture_with_dc(
    screen_dc: HDC,
    rect: &RECT,
    width: i32,
    height: i32,
) -> crate::Result<Frame> {
    // SAFETY: The caller keeps screen_dc live while creating a compatible DC.
    let memory_dc = unsafe { CreateCompatibleDC(screen_dc) };

    if memory_dc.is_null() {
        return Err(crate::Error::WindowsCapture(
            "CreateCompatibleDC failed".to_string(),
        ));
    }

    // SAFETY: The caller supplies a live DC and positive bitmap dimensions.
    let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };

    if bitmap.is_null() {
        // SAFETY: memory_dc is the owned compatible DC created above.
        unsafe { DeleteDC(memory_dc) };
        return Err(crate::Error::WindowsCapture(
            "CreateCompatibleBitmap failed".to_string(),
        ));
    }

    // SAFETY: Both objects are live, and bitmap is compatible with memory_dc.
    let old_bitmap = unsafe { SelectObject(memory_dc, bitmap.cast()) };
    // SAFETY: Both DCs and the selected bitmap stay live for the positive capture rectangle.
    let copied = unsafe {
        BitBlt(
            memory_dc, 0, 0, width, height, screen_dc, rect.left, rect.top, SRCCOPY,
        )
    };

    let result = if copied == 0 {
        Err(crate::Error::WindowsCapture("BitBlt failed".to_string()))
    } else {
        // SAFETY: The DC and its compatible bitmap are live with the given dimensions.
        unsafe { read_bitmap(memory_dc, bitmap, width, height) }
    };

    // SAFETY: Restore the original selection before deleting our owned bitmap and DC.
    unsafe {
        SelectObject(memory_dc, old_bitmap);
        DeleteObject(bitmap.cast());
        DeleteDC(memory_dc);
    }

    result
}

/// # Safety
/// `dc` and `bitmap` must be live compatible GDI objects with the given positive dimensions.
unsafe fn read_bitmap(dc: HDC, bitmap: HBITMAP, width: i32, height: i32) -> crate::Result<Frame> {
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: u32::try_from(mem::size_of::<BITMAPINFOHEADER>())
                .expect("Windows header size fits in u32"),
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            // SAFETY: The remaining BITMAPINFOHEADER fields are plain C integers.
            ..unsafe { mem::zeroed() }
        },
        // SAFETY: RGBQUAD contains only byte components.
        bmiColors: [unsafe { mem::zeroed() }],
    };
    let size = (width.cast_unsigned() as usize)
        .checked_mul(height.cast_unsigned() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(crate::Error::InvalidImageBuffer)?;
    let mut data = vec![0u8; size];

    // SAFETY: The caller keeps both GDI objects live. The checked allocation holds
    // all requested 32-bit rows and info describes the same dimensions and format.
    let rows = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height.cast_unsigned(),
            data.as_mut_ptr().cast(),
            &raw mut info,
            DIB_RGB_COLORS,
        )
    };

    if rows == 0 {
        return Err(crate::Error::WindowsCapture("GetDIBits failed".to_string()));
    }

    for pixel in data.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }

    Ok(Frame {
        data,
        width: width.cast_unsigned(),
        height: height.cast_unsigned(),
    })
}
