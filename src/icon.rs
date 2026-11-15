use std::ptr::{null, null_mut};

use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateCompatibleDC, CreateDIBSection,
    CreatePen, CreateSolidBrush, DIB_RGB_COLORS, DeleteDC, DeleteObject, GdiFlush, GetDC,
    GetStockObject, LineTo, MoveToEx, NULL_BRUSH, PS_SOLID, ReleaseDC, RoundRect, SelectObject,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON,
};

fn rgb(r: u8, g: u8, b: u8) -> u32 {
    u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16)
}

fn scale(v: i32, size: i32) -> i32 {
    (v * size + 8) / 16
}

pub fn make(active: bool) -> HICON {
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.max(16);
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }; 1],
    };

    const SENTINEL: u32 = 0x00_12_34_56;

    unsafe {
        let screen_dc = GetDC(null_mut());
        let mem_dc = CreateCompatibleDC(screen_dc);
        let mut bits_ptr: *mut core::ffi::c_void = null_mut();
        let hbm_color = CreateDIBSection(
            screen_dc,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            null_mut(),
            0,
        );
        ReleaseDC(null_mut(), screen_dc);

        let count = (size as usize) * (size as usize);
        let pixels = std::slice::from_raw_parts_mut(bits_ptr.cast::<u32>(), count);
        pixels.fill(SENTINEL);

        let old_bmp = SelectObject(mem_dc, hbm_color);

        let stroke_w = (size / 16).max(1);
        let outline_color = if active {
            rgb(230, 194, 159)
        } else {
            rgb(160, 160, 160)
        };
        let fill_color = rgb(111, 78, 55); // coffee brown

        let pen = CreatePen(PS_SOLID, stroke_w, outline_color);
        let brush = if active {
            CreateSolidBrush(fill_color)
        } else {
            GetStockObject(NULL_BRUSH)
        };
        let old_pen = SelectObject(mem_dc, pen);
        let old_brush = SelectObject(mem_dc, brush);

        // Handle on the right (drawn before body so body overlaps cleanly)
        let null_brush = GetStockObject(NULL_BRUSH);
        SelectObject(mem_dc, null_brush);
        RoundRect(
            mem_dc,
            scale(9, size),
            scale(7, size),
            scale(14, size),
            scale(12, size),
            scale(3, size),
            scale(3, size),
        );

        // Cup body
        SelectObject(mem_dc, brush);
        RoundRect(
            mem_dc,
            scale(2, size),
            scale(5, size),
            scale(11, size),
            scale(14, size),
            scale(3, size),
            scale(3, size),
        );

        // Saucer line at bottom
        MoveToEx(mem_dc, scale(1, size), scale(14, size), null_mut());
        LineTo(mem_dc, scale(12, size), scale(14, size));

        // Steam wisps when active
        if active {
            for x in [4, 6, 8] {
                MoveToEx(mem_dc, scale(x, size), scale(3, size), null_mut());
                LineTo(mem_dc, scale(x + 1, size), scale(1, size));
            }
        }

        GdiFlush();

        for px in pixels.iter_mut() {
            if *px == SENTINEL {
                *px = 0;
            } else {
                *px |= 0xFF00_0000;
            }
        }

        SelectObject(mem_dc, old_brush);
        SelectObject(mem_dc, old_pen);
        SelectObject(mem_dc, old_bmp);
        if active {
            DeleteObject(brush);
        }
        DeleteObject(pen);
        DeleteDC(mem_dc);

        // 1bpp mask of all zeros (alpha channel in hbm_color controls transparency)
        let mask_stride = ((size as usize + 15) / 16) * 2;
        let mask_bytes = vec![0u8; mask_stride * (size as usize)];
        let hbm_mask = CreateBitmap(size, size, 1, 1, mask_bytes.as_ptr().cast());

        let ii = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let icon = CreateIconIndirect(&ii);

        DeleteObject(hbm_mask);
        DeleteObject(hbm_color);
        let _ = null::<u8>();
        icon
    }
}
