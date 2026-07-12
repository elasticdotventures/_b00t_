/// 🥾 b00t Tray Test — minimal standalone tray icon test
/// 
/// Tests that Shell_NotifyIconW works on this Windows system.
/// Compile: rustc scripts/tray-test.rs -o target/tray-test.exe
/// Run:     ./target/tray-test.exe
/// 
/// If this works but ledgerr-tauri doesn't show a tray icon,
/// the issue is in ledgerr-tauri's tray setup code.
/// If this fails, the issue is in the Windows API or permissions.

#[cfg(windows)]
fn main() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;

    // Win32 constants
    const NIM_ADD: u32 = 0;
    const NIM_DELETE: u32 = 2;
    const WM_APP: u32 = 0x8000;
    const NIF_MESSAGE: u32 = 1;
    const NIF_ICON: u32 = 2;
    const NIF_TIP: u32 = 4;

    #[repr(C)]
    struct NOTIFYICONDATAW {
        cb_size: u32,
        hwnd: isize,
        u_id: u32,
        u_flags: u32,
        u_callback_message: u32,
        h_icon: isize,
        tip: [u16; 128],
        dw_state: u32,
        dw_state_mask: u32,
        sz_info: [u16; 256],
        u_timeout: u32,
        u_version: u32,
    }

    type WNDPROC = Option<unsafe extern "system" fn(isize, u32, isize, isize) -> isize>;

    #[link(name = "user32")]
    extern "system" {
        fn CreateWindowExW(
            ex_style: u32, class: *const u16, name: *const u16,
            style: u32, x: i32, y: i32, w: i32, h: i32,
            parent: isize, menu: isize, instance: isize, param: *mut u8,
        ) -> isize;
        fn DefWindowProcW(hwnd: isize, msg: u32, wparam: isize, lparam: isize) -> isize;
        fn RegisterClassW(class: *const u16) -> u16;
        fn GetMessageW(msg: *mut u8, hwnd: isize, min: u32, max: u32) -> i32;
        fn TranslateMessage(msg: *mut u8);
        fn DispatchMessageW(msg: *mut u8);
        fn PostQuitMessage(exit: i32);
        fn DestroyWindow(hwnd: isize) -> i32;
    }

    #[link(name = "shell32")]
    extern "system" {
        fn Shell_NotifyIconW(dw_message: u32, lp_data: *mut NOTIFYICONDATAW) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn CreateSolidBrush(color: u32) -> isize;
    }

    unsafe {
        // Register window class
        let class_name: Vec<u16> = OsStr::new("b00tTrayTest\0").encode_wide().collect();
        
        extern "system" fn wnd_proc(hwnd: isize, msg: u32, wparam: isize, lparam: isize) -> isize {
            match msg {
                0x8001 => { // WM_APP + 1 (callback from tray)
                    eprintln!("  Tray callback: lparam={}", lparam);
                    if lparam == 0x0203 { // WM_LBUTTONDBLCLK
                        eprintln!("  Tray icon double-clicked!");
                    }
                    if lparam == 0x0204 { // WM_RBUTTONUP
                        eprintln!("  Right-click detected — menu would show here");
                    }
                    0
                }
                0x0012 => { // WM_DESTROY
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }

        let wc = [0u16; 48]; // Simplified: use real WNDCLASSW in production
        let class_atom = RegisterClassW(class_name.as_ptr());
        
        // Create hidden window
        let hwnd = CreateWindowExW(
            0, class_name.as_ptr(), class_name.as_ptr(),
            0, 0, 0, 0, 0, 0, 0, 0, null_mut(),
        );

        if hwnd == 0 {
            eprintln!("❌ Failed to create window");
            std::process::exit(1);
        }
        eprintln!("✅ Hidden window created: hwnd={:x}", hwnd);

        // Create a simple icon (solid blue 16x16)
        let brush = CreateSolidBrush(0x00b8fdf8); // b00t blue
        let mut icon = NOTIFYICONDATAW {
            cb_size: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hwnd,
            u_id: 1,
            u_flags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            u_callback_message: WM_APP + 1,
            h_icon: brush, // Simplified: use real icon creation
            tip: {
                let mut t = [0u16; 128];
                let tip = OsStr::new("b00t Tray Test\0").encode_wide();
                for (i, c) in tip.enumerate().take(128) { t[i] = c; }
                t
            },
            dw_state: 0,
            dw_state_mask: 0,
            sz_info: [0u16; 256],
            u_timeout: 0,
            u_version: 0,
        };

        // Add tray icon
        let result = Shell_NotifyIconW(NIM_ADD, &mut icon);
        if result == 0 {
            eprintln!("❌ Shell_NotifyIconW(NIM_ADD) failed — error 0");
            eprintln!("   Common causes:");
            eprintln!("   - Application is not running as the same user as the shell");
            eprintln!("   - Too many tray icons already created");
            eprintln!("   - Running in a session without Explorer (e.g. WinRM, SSH)");
            DestroyWindow(hwnd);
            std::process::exit(1);
        }
        eprintln!("✅ Shell_NotifyIconW(NIM_ADD) succeeded");
        eprintln!("🥾 b00t tray icon should appear in the notification area");
        eprintln!("   Press Ctrl+C or close this window to remove the icon");

        // Message loop (runs until window is destroyed)
        let mut msg = std::mem::zeroed();
        while GetMessageW(&mut msg as *mut _ as *mut u8, 0, 0, 0) > 0 {
            TranslateMessage(&mut msg as *mut _ as *mut u8);
            DispatchMessageW(&mut msg as *mut _ as *mut u8);
        }

        // Clean up
        Shell_NotifyIconW(NIM_DELETE, &mut icon);
        DestroyWindow(hwnd);
        eprintln!("✅ Tray icon removed");
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("❌ This test only runs on Windows");
    std::process::exit(1);
}
