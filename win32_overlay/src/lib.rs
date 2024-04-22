use std::collections::HashMap;

use anyhow::Context;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::{Dwm::*, Gdi::*, OpenGL::*},
    System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const CLASS_NAME: &[u16; 5] = &[0x73, 0x75, 0x6e, 0x67, 0x00];

const WGL_SUPPORT_OPENGL_ARB: u32 = 0x2010;
const WGL_DRAW_TO_WINDOW_ARB: u32 = 0x2001;
const WGL_TRANSPARENT_ARB: u32 = 0x200A;
const WGL_PIXEL_TYPE_ARB: u32 = 0x2013;
const WGL_TYPE_RGBA_ARB: u32 = 0x202b;
const WGL_ACCELERATION_ARB: u32 = 0x2003;
const WGL_FULL_ACCELERATION_ARB: u32 = 0x2027;
const WGL_SWAP_METHOD_ARB: u32 = 0x2007;
const WGL_SWAP_EXCHANGE_ARB: u32 = 0x2028;
const WGL_RED_BITS_ARB: u32 = 0x2015;
const WGL_GREEN_BITS_ARB: u32 = 0x2017;
const WGL_BLUE_BITS_ARB: u32 = 0x2019;
const WGL_ALPHA_BITS_ARB: u32 = 0x201b;
const WGL_DOUBLE_BUFFER_ARB: u32 = 0x2011;
const WGL_CONTEXT_PROFILE_MASK_ARB: u32 = 0x9126;
const WGL_CONTEXT_CORE_PROFILE_BIT_ARB: u32 = 0x00000001;
const WGL_CONTEXT_MAJOR_VERSION_ARB: u32 = 0x2091;
const WGL_CONTEXT_MINOR_VERSION_ARB: u32 = 0x2092;

fn win32_last_error() -> WIN32_ERROR {
    unsafe { GetLastError() }
}

fn get_process_handle() -> anyhow::Result<isize> {
    let res = unsafe { GetModuleHandleW(core::ptr::null()) };
    if res == 0 {
        anyhow::bail!("get module handle failed; win32 {}", win32_last_error());
    }
    Ok(res)
}

enum Event {
    RepaintAt(std::time::Duration),
    Input(egui::Event),
}

fn is_key_pressed(key: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(key.into()) >> 15) != 0 }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    fn lparam_egui_pos(lparam: LPARAM) -> egui::Pos2 {
        let x_pos = lparam & 0xFFFF;
        let y_pos = (lparam >> 16) & 0xFFFF;
        egui::Pos2::new(x_pos as f32, y_pos as f32)
    }
    fn wparam_mods(wparam: WPARAM) -> egui::Modifiers {
        let shift = (wparam & 0x4) != 0;
        let ctrl = (wparam & 0x8) != 0;
        let alt = is_key_pressed(VK_MENU);
        egui::Modifiers {
            alt,
            ctrl,
            shift,
            mac_cmd: false,
            command: ctrl,
        }
    }
    unsafe fn send_event(hwnd: HWND, ev: egui::Event) {
        let state = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
        (*state).tx.send(Event::Input(ev)).unwrap();
    }
    match msg {
        WM_NCCREATE => return 1,
        WM_CREATE => {
            let create_struct: *mut CREATESTRUCTW = lparam as *mut _;
            if create_struct.is_null() {
                return 0;
            }
            let window_state = (*create_struct).lpCreateParams;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, window_state as isize);
            return 1;
        }
        WM_DESTROY => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            drop(Box::from_raw(ptr));
            PostQuitMessage(0);
        }
        WM_MOUSEMOVE => {
            let pos = lparam_egui_pos(lparam);
            let ev = egui::Event::PointerMoved(pos);
            send_event(hwnd, ev);
        }
        WM_LBUTTONDOWN => {
            let pos = lparam_egui_pos(lparam);
            let modifiers = wparam_mods(wparam);
            let ev = egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers,
            };
            send_event(hwnd, ev);
        }
        WM_LBUTTONUP => {
            let pos = lparam_egui_pos(lparam);
            let modifiers = wparam_mods(wparam);
            let ev = egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers,
            };
            send_event(hwnd, ev);
        }
        WM_RBUTTONDOWN => {
            let pos = lparam_egui_pos(lparam);
            let modifiers = wparam_mods(wparam);
            let ev = egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed: true,
                modifiers,
            };
            send_event(hwnd, ev);
        }
        WM_RBUTTONUP => {
            let pos = lparam_egui_pos(lparam);
            let modifiers = wparam_mods(wparam);
            let ev = egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Secondary,
                pressed: false,
                modifiers,
            };
            send_event(hwnd, ev);
        }
        WM_MOUSEACTIVATE => {
            return MA_NOACTIVATE as _;
        }
        _ => {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
    }
    0
}

type ChoosePixelFormatARB = extern "system" fn(
    _: HDC,
    _: *const i32,
    _: *const f32,
    _: u32,
    _: *mut i32,
    _: *mut u32,
) -> BOOL;
type GetExtensionsStringEXT = extern "system" fn() -> *const i8;
type GetExtensionsStringARB = extern "system" fn(_: HDC) -> *const i8;
type CreateContextAttribsARB = extern "system" fn(_: HDC, _: HGLRC, _: *const i32) -> HGLRC;

struct Wgl {
    choose_pixel_format_arb: Option<ChoosePixelFormatARB>,
    create_context_attribs_arb: Option<CreateContextAttribsARB>,
}

mod modules {
    use windows_sys::Win32::{
        Foundation::{FreeLibrary, BOOL, PROC},
        Graphics::{Gdi::*, OpenGL::*},
        System::LibraryLoader::*,
    };

    use crate::cstr;

    pub struct Module(pub isize);

    impl Module {
        fn load(path: &std::ffi::CStr) -> anyhow::Result<Self> {
            let library = unsafe { LoadLibraryA(path.as_ptr() as *const _) };
            if library == 0 {
                anyhow::bail!("failed to load library {}", path.to_string_lossy());
            }
            Ok(Self(library))
        }
        fn get_symbol<F: Sized>(&self, name: &std::ffi::CStr) -> anyhow::Result<F> {
            let proc = unsafe { GetProcAddress(self.0, name.as_ptr() as *const _) };
            if proc.is_none() {
                anyhow::bail!("failed to acquire symbol {}", name.to_string_lossy());
            }
            Ok(unsafe { std::mem::transmute_copy(&proc.unwrap()) })
        }
    }

    impl Drop for Module {
        fn drop(&mut self) {
            unsafe { FreeLibrary(self.0) };
        }
    }

    pub type WglCreateContext = extern "system" fn(_: HDC) -> HGLRC;
    pub type WglDeleteContext = extern "system" fn(_: HGLRC) -> BOOL;
    pub type WglGetProcAddress = extern "system" fn(_: *const i8) -> PROC;
    pub type WglGetCurrentDc = extern "system" fn() -> HDC;
    pub type WglMakeCurrent = extern "system" fn(_: HDC, _: HGLRC) -> BOOL;

    pub struct LibOpengl32 {
        pub module: Module,
        pub wgl_create_context: WglCreateContext,
        pub wgl_delete_context: WglDeleteContext,
        pub wgl_get_proc_address: WglGetProcAddress,
        pub wgl_get_current_dc: WglGetCurrentDc,
        pub wgl_make_current: WglMakeCurrent,
    }

    impl LibOpengl32 {
        pub fn try_load() -> Option<LibOpengl32> {
            Module::load(cstr!("opengl32.dll"))
                .map(|module| LibOpengl32 {
                    wgl_create_context: module.get_symbol(cstr!("wglCreateContext")).unwrap(),
                    wgl_delete_context: module.get_symbol(cstr!("wglDeleteContext")).unwrap(),
                    wgl_get_proc_address: module.get_symbol(cstr!("wglGetProcAddress")).unwrap(),
                    wgl_get_current_dc: module.get_symbol(cstr!("wglGetCurrentDC")).unwrap(),
                    wgl_make_current: module.get_symbol(cstr!("wglMakeCurrent")).unwrap(),
                    module,
                })
                .ok()
        }
    }
}

#[macro_export]
macro_rules! cstr {
    ($lit:expr) => {
        #[allow(unused_unsafe)]
        unsafe {
            std::ffi::CStr::from_ptr(concat!($lit, "\0").as_ptr() as *const std::os::raw::c_char)
        }
    };
}

unsafe fn get_wgl_proc_address<T>(
    lib: &mut modules::LibOpengl32,
    proc: &std::ffi::CStr,
) -> Option<T> {
    let proc = (lib.wgl_get_proc_address)(proc.as_ptr() as *const _)
        .or_else(|| GetProcAddress(lib.module.0, proc.as_ptr() as *const _));

    proc.map(|proc| std::mem::transmute_copy(&proc))
}

fn setup_wgl(instance: isize, libopengl32: &mut modules::LibOpengl32) -> anyhow::Result<Wgl> {
    let _window_class = register_window_class(
        instance,
        DefWindowProcW,
        CLASS_NAME,
        CS_HREDRAW | CS_VREDRAW,
    )?;

    let dummy_hwnd = create_window(instance, CLASS_NAME, 0, 0, None)?;
    let dummy_dc = unsafe { GetDC(dummy_hwnd) };

    let mut pfd: PIXELFORMATDESCRIPTOR = unsafe { core::mem::zeroed() };
    pfd.nSize = core::mem::size_of_val(&pfd) as _;
    pfd.nVersion = 1;
    pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
    pfd.iPixelType = PFD_TYPE_RGBA;
    pfd.cColorBits = 24;

    let pixel_format = unsafe { ChoosePixelFormat(dummy_dc, &pfd) };
    if pixel_format == 0 {
        anyhow::bail!("no suitable pixel format");
    }
    if unsafe { SetPixelFormat(dummy_dc, pixel_format, &pfd) } == 0 {
        anyhow::bail!("failed to set pixel format");
    }
    let dummy_ctx = unsafe { wglCreateContext(dummy_dc) };
    if dummy_ctx == 0 {
        anyhow::bail!("failed to create rendering context");
    }
    if unsafe { wglMakeCurrent(dummy_dc, dummy_ctx) } == 0 {
        anyhow::bail!("failed to activate rendering context");
    }

    let get_extensions_string_ext: Option<GetExtensionsStringEXT> =
        unsafe { get_wgl_proc_address(libopengl32, cstr!("wglGetExtensionsStringEXT")) };
    let get_extensions_string_arb: Option<GetExtensionsStringARB> =
        unsafe { get_wgl_proc_address(libopengl32, cstr!("wglGetExtensionsStringARB")) };
    let create_context_attribs_arb: Option<CreateContextAttribsARB> =
        unsafe { get_wgl_proc_address(libopengl32, cstr!("wglCreateContextAttribsARB")) };
    let choose_pixel_format_arb: Option<ChoosePixelFormatARB> =
        unsafe { get_wgl_proc_address(libopengl32, cstr!("wglChoosePixelFormatARB")) };

    let wgl_ext_supported = |ext: &str| -> bool {
        if let Some(get_extensions_string_ext) = get_extensions_string_ext {
            let extensions = get_extensions_string_ext();

            if extensions.is_null() == false {
                let extensions_string =
                    unsafe { std::ffi::CStr::from_ptr(extensions) }.to_string_lossy();
                if extensions_string.contains(ext) {
                    return true;
                }
            }
        }

        if let Some(get_extensions_string_arb) = get_extensions_string_arb {
            let extensions = get_extensions_string_arb((libopengl32.wgl_get_current_dc)());
            if extensions.is_null() == false {
                let extensions_string =
                    unsafe { std::ffi::CStr::from_ptr(extensions) }.to_string_lossy();

                if extensions_string.contains(ext) {
                    return true;
                }
            }
        }
        return false;
    };

    let arb_create_context = wgl_ext_supported("WGL_ARB_create_context");
    let arb_create_context_profile = wgl_ext_supported("WGL_ARB_create_context_profile");
    let arb_pixel_format = wgl_ext_supported("WGL_ARB_pixel_format");

    if !arb_pixel_format {
        anyhow::bail!("WGL_ARB_pixel_format is required")
    };
    if !arb_create_context {
        anyhow::bail!("WGL_ARB_create_context is required")
    };
    if !arb_create_context_profile {
        anyhow::bail!("WGL_ARB_create_context_profile is required")
    };

    if unsafe { wglMakeCurrent(dummy_dc, 0) } == 0 {
        anyhow::bail!("failed to deactivate rendering context");
    }
    if (libopengl32.wgl_delete_context)(dummy_ctx) == 0 {
        anyhow::bail!("failed to delete rendering context");
    }
    if unsafe { ReleaseDC(dummy_hwnd, dummy_dc) } == 0 {
        anyhow::bail!("failed to release device context");
    }
    if unsafe { DestroyWindow(dummy_hwnd) } == 0 {
        anyhow::bail!("failed to destroy window");
    }
    if unsafe { UnregisterClassW(CLASS_NAME.as_ptr(), instance) } == 0 {
        anyhow::bail!("failed to deregister window class");
    }

    Ok(Wgl {
        create_context_attribs_arb,
        choose_pixel_format_arb,
    })
}

impl Wgl {
    fn create_context(&self, dc: isize, lib: &modules::LibOpengl32) -> anyhow::Result<isize> {
        #[rustfmt::skip]
        let pixel_format_attribs = &[
            WGL_DRAW_TO_WINDOW_ARB,     GL_TRUE,
            WGL_SUPPORT_OPENGL_ARB,     GL_TRUE,
            WGL_DOUBLE_BUFFER_ARB,      GL_TRUE,
            WGL_TRANSPARENT_ARB,        GL_TRUE,
            WGL_SWAP_METHOD_ARB,        WGL_SWAP_EXCHANGE_ARB,
            WGL_ACCELERATION_ARB,       WGL_FULL_ACCELERATION_ARB,
            WGL_PIXEL_TYPE_ARB,         WGL_TYPE_RGBA_ARB,
            WGL_RED_BITS_ARB,           8,
            WGL_GREEN_BITS_ARB,         8,
            WGL_BLUE_BITS_ARB,          8,
            WGL_ALPHA_BITS_ARB,         8,
            0
        ];

        let mut pixel_format = 0i32;
        let mut num_formats = 0u32;
        if (self.choose_pixel_format_arb.unwrap())(
            dc,
            pixel_format_attribs.as_ptr() as *const _,
            core::ptr::null(),
            1,
            &mut pixel_format,
            &mut num_formats,
        ) == 0
        {
            anyhow::bail!("failed to find suitable pixel format");
        }

        let mut pfd: PIXELFORMATDESCRIPTOR = unsafe { core::mem::zeroed() };
        if unsafe {
            DescribePixelFormat(
                dc,
                pixel_format,
                core::mem::size_of_val(&pfd) as _,
                &mut pfd,
            )
        } == 0
        {
            anyhow::bail!("failed to retrieve chosen pixel format descriptor");
        }

        if unsafe { SetPixelFormat(dc, pixel_format, &pfd) } == 0 {
            anyhow::bail!("failed to set pixel format");
        }

        #[rustfmt::skip]
        let gl_attribs = &[
            WGL_CONTEXT_MAJOR_VERSION_ARB, 3,
            WGL_CONTEXT_MINOR_VERSION_ARB, 3,
            WGL_CONTEXT_PROFILE_MASK_ARB, WGL_CONTEXT_CORE_PROFILE_BIT_ARB,
            0,
        ];

        let gl_ctx =
            (self.create_context_attribs_arb.unwrap())(dc, 0, gl_attribs.as_ptr() as *const _);
        if gl_ctx == 0 {
            anyhow::bail!(
                "failed to create opengl context, code {}",
                win32_last_error()
            );
        }

        if (lib.wgl_make_current)(dc, gl_ctx) == 0 {
            anyhow::bail!("failed to set opengl context");
        }

        Ok(gl_ctx)
    }
}

fn register_window_class(
    instance: isize,
    proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    name: &'static [u16],
    style: u32,
) -> anyhow::Result<u16> {
    unsafe {
        let window_class = WNDCLASSW {
            style,
            lpfnWndProc: Some(proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: 0 as HICON,
            hCursor: LoadCursorW(0, IDC_ARROW),
            hbrBackground: 0,
            lpszMenuName: core::ptr::null(),
            lpszClassName: name.as_ptr(),
        };

        let class = RegisterClassW(&window_class);
        if class == 0 {
            anyhow::bail!(
                "window class registration failed: error code {}",
                GetLastError()
            );
        }
        Ok(class)
    }
}

fn create_window(
    instance: isize,
    name: &'static [u16],
    flags: u32,
    ex_flags: u32,
    state: Option<WindowState>,
) -> anyhow::Result<isize> {
    let state = Box::leak(Box::new(state));
    let hwnd = unsafe {
        CreateWindowExW(
            ex_flags,
            name.as_ptr(),
            name.as_ptr(),
            flags,
            0,
            0,
            1920,
            1080,
            0,
            0,
            instance,
            if let Some(state) = state {
                state as *mut _ as *const _
            } else {
                core::ptr::null()
            },
        )
    };

    if hwnd == 0 {
        anyhow::bail!("create window failed; win32 {}", win32_last_error());
    }
    Ok(hwnd)
}

mod gl {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

struct Texture {
    id: u32,
    min_filter: u32,
    mag_filter: u32,
}

struct WindowState {
    tx: std::sync::mpsc::Sender<Event>,
}

pub trait App {
    fn update(&mut self, ctx: &egui::Context);
}

pub fn run<I, A>(init: I) -> anyhow::Result<()>
where
    I: FnOnce(&egui::Context) -> A + Send + 'static,
    A: App + Send + 'static,
{
    let mut lib = modules::LibOpengl32::try_load().context("failed to load opengl32.dll")?;
    let instance = get_process_handle()?;
    let wgl = setup_wgl(instance, &mut lib)?;

    let (tx, rx) = std::sync::mpsc::channel();

    let _window_class = register_window_class(instance, window_proc, CLASS_NAME, 0)?;
    let hwnd = create_window(
        instance,
        CLASS_NAME,
        WS_POPUP,
        WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_NOACTIVATE,
        Some(WindowState { tx: tx.clone() }),
    )?;

    unsafe {
        SetLayeredWindowAttributes(hwnd, 0x00000000, 255, LWA_COLORKEY);

        let bb = DWM_BLURBEHIND {
            dwFlags: DWM_BB_ENABLE,
            hRgnBlur: 0,
            fEnable: TRUE,
            fTransitionOnMaximized: 0,
        };
        DwmEnableBlurBehindWindow(hwnd, &bb);
        let margins = windows_sys::Win32::UI::Controls::MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        DwmExtendFrameIntoClientArea(hwnd, &margins);
        ShowWindow(hwnd, SW_SHOW);
    }

    std::thread::spawn(move || {
        render_thread(hwnd, wgl, lib, init, tx, rx).unwrap();
    });

    unsafe {
        let mut msg = core::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if msg.message == WM_QUIT {
                // send quit message?
                // done = true;
                // continue;
            } else {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    if unsafe { DestroyWindow(hwnd) } == 0 {
        anyhow::bail!("failed to destroy window");
    }
    if unsafe { UnregisterClassW(CLASS_NAME.as_ptr(), instance) } == 0 {
        anyhow::bail!("failed to deregister window class");
    }

    Ok(())
}

fn render_thread<I, A>(
    hwnd: HWND,
    wgl: Wgl,
    mut lib: modules::LibOpengl32,
    init: I,
    tx: std::sync::mpsc::Sender<Event>,
    rx: std::sync::mpsc::Receiver<Event>,
) -> anyhow::Result<()>
where
    I: FnOnce(&egui::Context) -> A + Send + 'static,
    A: App + Send + 'static,
{
    let dc = unsafe { GetDC(hwnd) };
    let gl_ctx = wgl.create_context(dc, &lib)?;

    let gl = gl::Gl::load_with(|s| unsafe {
        let s = std::ffi::CString::new(s).unwrap();
        get_wgl_proc_address(&mut lib, &s).unwrap()
    });

    let version = unsafe { std::ffi::CStr::from_ptr(gl.GetString(GL_VERSION) as *const _) };
    println!("loaded GL: {}", version.to_string_lossy());

    let vs_src = r###"
        #version 330 core

        uniform vec2 u_screen_size;
        layout (location = 0) in vec2 a_pos;
        layout (location = 1) in vec2 a_uv;
        layout (location = 2) in vec4 a_srgba;
        out vec2 v_uv;
        out vec4 v_rgba_gamma;

        void main() {
            gl_Position = vec4(
                2.0 * a_pos.x / u_screen_size.x - 1.0,
                1.0 - 2.0 * a_pos.y / u_screen_size.y,
                0.0,
                1.0
            );
            v_uv = a_uv;
            v_rgba_gamma = a_srgba / 255.0;
        }
    "###;

    let fs_src = r###"
        #version 330 core

        uniform sampler2D u_sampler;
        in vec2 v_uv;
        in vec4 v_rgba_gamma;
        out vec4 f_color;

        // 0-255 sRGB  from  0-1 linear
        vec3 srgb_from_linear(vec3 rgb) {
            bvec3 cutoff = lessThan(rgb, vec3(0.0031308));
            vec3 lower = rgb * vec3(3294.6);
            vec3 higher = vec3(269.025) * pow(rgb, vec3(1.0 / 2.4)) - vec3(14.025);
            return mix(higher, lower, vec3(cutoff));
        }

        // 0-255 sRGBA  from  0-1 linear
        vec4 srgba_from_linear(vec4 rgba) {
            return vec4(srgb_from_linear(rgba.rgb), 255.0 * rgba.a);
        }

        // 0-1 gamma  from  0-1 linear
        vec4 gamma_from_linear_rgba(vec4 linear_rgba) {
            return vec4(srgb_from_linear(linear_rgba.rgb) / 255.0, linear_rgba.a);
        }

        void main() {
            vec4 texture_in_gamma = gamma_from_linear_rgba(texture(u_sampler, v_uv));
            f_color = v_rgba_gamma * texture_in_gamma;
        } 
    "###;

    let (sp, vao, vbo, ebo) = unsafe {
        let vs = gl.CreateShader(gl::VERTEX_SHADER);
        let fs = gl.CreateShader(gl::FRAGMENT_SHADER);
        gl.ShaderSource(vs, 1, &(vs_src.as_ptr() as *const _), &(vs_src.len() as _));
        gl.ShaderSource(fs, 1, &(fs_src.as_ptr() as *const _), &(fs_src.len() as _));
        let mut success = 0;
        gl.CompileShader(vs);
        gl.GetShaderiv(vs, gl::COMPILE_STATUS, &mut success);
        if success == 0 {
            println!("vs died");
        }
        gl.CompileShader(fs);
        gl.GetShaderiv(fs, gl::COMPILE_STATUS, &mut success);
        if success == 0 {
            println!("fs died");
            let mut log = [0u8; 512];
            let mut len = 0;
            gl.GetShaderInfoLog(fs, 512, &mut len, log.as_mut_ptr() as *mut _);
            println!("{}", std::str::from_utf8(&log[0..len as usize]).unwrap());
        }
        let sp = gl.CreateProgram();
        gl.AttachShader(sp, vs);
        gl.AttachShader(sp, fs);
        gl.LinkProgram(sp);
        gl.UseProgram(sp);
        gl.DeleteShader(vs);
        gl.DeleteShader(fs);

        let mut vao = 0;
        let mut vbo = 0;
        let mut ebo = 0;
        gl.GenVertexArrays(1, &mut vao);
        gl.GenBuffers(1, &mut vbo);
        gl.GenBuffers(1, &mut ebo);

        gl.Enable(gl::BLEND);
        gl.BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);

        let u_screen_size = gl.GetUniformLocation(sp, cstr!("u_screen_size").as_ptr());
        gl.Uniform2f(u_screen_size, 1920.0, 1080.0);
        let u_sampler = gl.GetUniformLocation(sp, cstr!("u_sampler").as_ptr());
        gl.Uniform1i(u_sampler, 0);

        (sp, vao, vbo, ebo)
    };

    let egui = egui::Context::default();
    let mut app = init(&egui);

    let max_texture_side = unsafe {
        let mut ret = 0;
        glGetIntegerv(GL_MAX_TEXTURE_SIZE, &mut ret);
        ret as usize
    };

    let mut textures: HashMap<egui::TextureId, Texture> = HashMap::new();

    egui.set_request_repaint_callback(move |info| {
        tx.send(Event::RepaintAt(info.delay)).unwrap();
    });

    let mut timeout = std::time::Duration::new(0, 0);
    let mut inputs = Vec::new();
    let mut done = false;
    let mut last_cursor_pos = {
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut point);
        }
        egui::pos2(point.x as _, point.y as _)
    };
    let mut lmb_held = false;
    while !done {
        // clear the queue
        let mut next_timeout = std::time::Duration::MAX;
        let mut queued = false;
        let handle_input = |i: &egui::Event, lp: &mut egui::Pos2, h: &mut bool| match i {
            egui::Event::PointerMoved(pos) => *lp = *pos,
            egui::Event::PointerButton {
                pos,
                button,
                pressed,
                ..
            } => match button {
                egui::PointerButton::Primary => {
                    *lp = *pos;
                    *h = *pressed;
                }
                _ => {}
            },
            _ => {}
        };
        while let Ok(e) = rx.try_recv() {
            queued = true;
            match e {
                Event::RepaintAt(t) => next_timeout = t,
                Event::Input(i) => {
                    handle_input(&i, &mut last_cursor_pos, &mut lmb_held);
                    inputs.push(i);
                }
            }
        }

        // if there weren't any queued events then wait for more
        let mut timed_out = false;
        if !queued {
            match rx.recv_timeout(timeout) {
                Ok(Event::RepaintAt(t)) => next_timeout = t,
                Ok(Event::Input(i)) => {
                    handle_input(&i, &mut last_cursor_pos, &mut lmb_held);
                    inputs.push(i);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => timed_out = true,
                _ => unreachable!(),
            }
        }

        if lmb_held {
            let mut point = POINT { x: 0, y: 0 };
            unsafe {
                GetCursorPos(&mut point);
            }

            let pos = egui::pos2(point.x as f32, point.y as f32);
            if pos != last_cursor_pos {
                inputs.push(egui::Event::PointerMoved(pos));
            }

            if lmb_held {
                if !is_key_pressed(VK_LBUTTON) {
                    let shift = is_key_pressed(VK_SHIFT);
                    let ctrl = is_key_pressed(VK_CONTROL);
                    let alt = is_key_pressed(VK_MENU);
                    inputs.push(egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers {
                            alt,
                            ctrl,
                            shift,
                            mac_cmd: false,
                            command: ctrl,
                        },
                    });
                    lmb_held = false;
                    // key came back up. we can stop synthesis
                }
            }
            next_timeout = std::time::Duration::ZERO;
        }

        if !inputs.is_empty() || timed_out {
            // repaint
            let mut raw_input = egui::RawInput::default();
            raw_input.screen_rect = Some(egui::Rect::from_min_size(
                Default::default(),
                egui::vec2(1920.0, 1080.0),
            ));
            let pixels_per_point = 1.0;
            raw_input.max_texture_side = Some(max_texture_side);
            raw_input.events = std::mem::take(&mut inputs);
            let egui::FullOutput {
                textures_delta: egui::TexturesDelta { set, free },
                shapes,
                ..
            } = egui.run(raw_input, |ctx| app.update(ctx));
            let clipped_primitives = egui.tessellate(shapes, pixels_per_point);

            for id in free {
                let tex = textures.remove(&id).unwrap();
                unsafe {
                    gl.DeleteTextures(1, &tex.id);
                }
            }

            for (id, delta) in set {
                let mag_filter = match delta.options.magnification {
                    egui::TextureFilter::Nearest => gl::NEAREST,
                    egui::TextureFilter::Linear => gl::LINEAR,
                };
                let min_filter = match delta.options.minification {
                    egui::TextureFilter::Nearest => gl::NEAREST,
                    egui::TextureFilter::Linear => gl::LINEAR,
                };

                let [width, height] = delta.image.size();

                let data: std::borrow::Cow<Vec<egui::Color32>> = match &delta.image {
                    egui::ImageData::Color(image) => std::borrow::Cow::Borrowed(&image.pixels),
                    egui::ImageData::Font(image) => {
                        std::borrow::Cow::Owned(image.srgba_pixels(None).collect::<Vec<_>>())
                    }
                };

                if let Some([x, y]) = delta.pos {
                    let tex = textures.get(&id).unwrap();
                    unsafe {
                        glBindTexture(gl::TEXTURE_2D, tex.id);
                        gl.TexSubImage2D(
                            gl::TEXTURE_2D,
                            0,
                            x as _,
                            y as _,
                            width as _,
                            height as _,
                            gl::RGBA,
                            gl::UNSIGNED_BYTE,
                            data.as_ptr() as *const _,
                        );
                    }
                } else {
                    let mut tex = 0;
                    unsafe {
                        glGenTextures(1, &mut tex);
                        glBindTexture(gl::TEXTURE_2D, tex);
                        glTexImage2D(
                            gl::TEXTURE_2D,
                            0,
                            gl::SRGB8_ALPHA8 as _,
                            width as _,
                            height as _,
                            0,
                            gl::RGBA,
                            gl::UNSIGNED_BYTE,
                            data.as_ptr() as *const _,
                        );
                    };

                    textures.insert(
                        id,
                        Texture {
                            id: tex,
                            min_filter,
                            mag_filter,
                        },
                    );
                }
            }

            unsafe {
                glClearColor(0.0, 0.0, 0.0, 0.0);
                glClear(GL_COLOR_BUFFER_BIT);

                for clp in &clipped_primitives {
                    if let egui::epaint::Primitive::Mesh(mesh) = &clp.primitive {
                        gl.BindVertexArray(vao);
                        gl.BindBuffer(gl::ARRAY_BUFFER, vbo);
                        gl.BufferData(
                            gl::ARRAY_BUFFER,
                            (mesh.vertices.len() * core::mem::size_of::<egui::epaint::Vertex>())
                                as _,
                            mesh.vertices.as_ptr() as *const _,
                            gl::STATIC_DRAW,
                        );
                        gl.BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
                        gl.BufferData(
                            gl::ELEMENT_ARRAY_BUFFER,
                            (mesh.indices.len() * core::mem::size_of::<u32>()) as _,
                            mesh.indices.as_ptr() as *const _,
                            gl::STATIC_DRAW,
                        );

                        gl.VertexAttribPointer(
                            0,
                            2,
                            gl::FLOAT,
                            gl::FALSE,
                            core::mem::size_of::<egui::epaint::Vertex>() as _,
                            core::ptr::null(),
                        );
                        gl.EnableVertexAttribArray(0);

                        gl.VertexAttribPointer(
                            1,
                            2,
                            gl::FLOAT,
                            gl::FALSE,
                            core::mem::size_of::<egui::epaint::Vertex>() as _,
                            (core::mem::size_of::<egui::Pos2>()) as _,
                        );
                        gl.EnableVertexAttribArray(1);

                        gl.VertexAttribPointer(
                            2,
                            4,
                            gl::UNSIGNED_BYTE,
                            gl::FALSE,
                            core::mem::size_of::<egui::epaint::Vertex>() as _,
                            (2 * core::mem::size_of::<egui::Pos2>()) as _,
                        );
                        gl.EnableVertexAttribArray(2);

                        // draw
                        gl.UseProgram(sp);
                        if let Some(tex) = textures.get(&mesh.texture_id) {
                            gl.ActiveTexture(gl::TEXTURE0);
                            glTexParameteri(
                                gl::TEXTURE_2D,
                                gl::TEXTURE_WRAP_S,
                                gl::CLAMP_TO_EDGE as _,
                            );
                            glTexParameteri(
                                gl::TEXTURE_2D,
                                gl::TEXTURE_WRAP_T,
                                gl::CLAMP_TO_EDGE as _,
                            );
                            glTexParameteri(
                                gl::TEXTURE_2D,
                                gl::TEXTURE_MAG_FILTER,
                                tex.mag_filter as _,
                            );
                            glTexParameteri(
                                gl::TEXTURE_2D,
                                gl::TEXTURE_MIN_FILTER,
                                tex.min_filter as _,
                            );
                            gl.BindTexture(gl::TEXTURE_2D, tex.id);
                        }
                        gl.BindVertexArray(vao);
                        gl.DrawElements(
                            gl::TRIANGLES,
                            mesh.indices.len() as _,
                            gl::UNSIGNED_INT,
                            core::ptr::null(),
                        );
                        gl.BindVertexArray(0);
                    }
                }

                SwapBuffers(dc);
            }
        }

        timeout = next_timeout;
        inputs.clear();
    }

    if unsafe { wglMakeCurrent(dc, 0) } == 0 {
        anyhow::bail!("failed to deactivate rendering context");
    }
    if (lib.wgl_delete_context)(gl_ctx) == 0 {
        anyhow::bail!("failed to delete rendering context");
    }
    if unsafe { ReleaseDC(hwnd, dc) } == 0 {
        anyhow::bail!("failed to release device context");
    }
    Ok(())
}
