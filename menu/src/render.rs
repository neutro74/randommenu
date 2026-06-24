// creates and updates Unity GameObjects for the in-world menu panel
// uses Mono embedding API directly since we need UnityEngine, not Assembly-CSharp

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::OnceLock;
use crate::mods::ALL_MODS;

extern "system" {
    fn GetModuleHandleA(name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
}

// function pointer types we use here
type FnAssemblyForeach  = unsafe extern "C" fn(unsafe extern "C" fn(*mut c_void, *mut c_void), *mut c_void);
type FnAssemblyGetImage = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type FnImageGetName     = unsafe extern "C" fn(*mut c_void) -> *const c_char;
type FnClassFromName    = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_void;
type FnClassGetMethod   = unsafe extern "C" fn(*mut c_void, *const c_char, i32) -> *mut c_void;
type FnClassGetField    = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type FnObjectNew        = unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void;
type FnRuntimeInvoke    = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void, *mut *mut c_void) -> *mut c_void;
type FnGetRootDomain    = unsafe extern "C" fn() -> *mut c_void;
type FnStringNew        = unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void;
type FnFieldSetValue    = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);
type FnFieldGetValue    = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);
type FnClassGetType     = unsafe extern "C" fn(*mut c_void) -> *mut c_void;

struct MonoSyms {
    assembly_foreach:   FnAssemblyForeach,
    assembly_get_image: FnAssemblyGetImage,
    image_get_name:     FnImageGetName,
    class_from_name:    FnClassFromName,
    class_get_method:   FnClassGetMethod,
    class_get_field:    FnClassGetField,
    object_new:         FnObjectNew,
    runtime_invoke:     FnRuntimeInvoke,
    get_root_domain:    FnGetRootDomain,
    string_new:         FnStringNew,
    field_set_value:    FnFieldSetValue,
    field_get_value:    FnFieldGetValue,
    class_get_type:     FnClassGetType,
}

struct RenderState {
    syms: MonoSyms,
    domain: *mut c_void,
    ue_image: *mut c_void,         // UnityEngine.CoreModule image
    go_class: *mut c_void,
    go_ctor_name: *mut c_void,     // GameObject(string) ctor
    go_set_active: *mut c_void,    // SetActive(bool)
    go_add_comp: *mut c_void,      // AddComponent(Type)
    transform_class: *mut c_void,
    set_position: *mut c_void,     // set_position setter
    textmesh_class: *mut c_void,
    textmesh_type: *mut c_void,    // System.Type instance for TextMesh
    text_field: *mut c_void,       // TextMesh.text field
    color_field: *mut c_void,      // TextMesh.color field
    fontsize_field: *mut c_void,   // TextMesh.fontSize field
    // created objects per mod button + one header
    button_objects: Vec<*mut c_void>,
    header_object: *mut c_void,
    created: bool,
}

unsafe impl Send for RenderState {}
unsafe impl Sync for RenderState {}

static RENDER: OnceLock<std::sync::Mutex<RenderState>> = OnceLock::new();

unsafe fn get_sym(mono: *mut c_void, name: &[u8]) -> *mut c_void {
    GetProcAddress(mono, name.as_ptr() as *const c_char)
}

unsafe fn load_syms() -> Option<MonoSyms> {
    let mono = GetModuleHandleA(b"mono-2.0-bdwgc.dll\0".as_ptr() as *const c_char);
    if mono.is_null() { return None; }
    macro_rules! sym {
        ($n:literal) => {{
            let p = get_sym(mono, concat!($n, "\0").as_bytes());
            if p.is_null() { return None; }
            std::mem::transmute(p)
        }};
    }
    Some(MonoSyms {
        assembly_foreach:   sym!("mono_assembly_foreach"),
        assembly_get_image: sym!("mono_assembly_get_image"),
        image_get_name:     sym!("mono_image_get_name"),
        class_from_name:    sym!("mono_class_from_name"),
        class_get_method:   sym!("mono_class_get_method_from_name"),
        class_get_field:    sym!("mono_class_get_field_from_name"),
        object_new:         sym!("mono_object_new"),
        runtime_invoke:     sym!("mono_runtime_invoke"),
        get_root_domain:    sym!("mono_get_root_domain"),
        string_new:         sym!("mono_string_new"),
        field_set_value:    sym!("mono_field_set_value"),
        field_get_value:    sym!("mono_field_get_value"),
        class_get_type:     sym!("mono_class_get_type"),
    })
}

// finds an assembly image by its name
struct FindImg<'a> { name: &'a str, image: *mut c_void, get_image: FnAssemblyGetImage, get_name: FnImageGetName }
unsafe extern "C" fn find_image_cb(asm: *mut c_void, data: *mut c_void) {
    let s = &mut *(data as *mut FindImg);
    if !s.image.is_null() { return; }
    let img = (s.get_image)(asm);
    if img.is_null() { return; }
    let np = (s.get_name)(img);
    if np.is_null() { return; }
    if CStr::from_ptr(np).to_str().unwrap_or("") == s.name { s.image = img; }
}

unsafe fn find_ue_image(s: &MonoSyms) -> Option<*mut c_void> {
    // Unity 2021+ splits into UnityEngine.CoreModule, older may be just UnityEngine
    for name in &["UnityEngine.CoreModule", "UnityEngine"] {
        let mut state = FindImg { name, image: std::ptr::null_mut(), get_image: s.assembly_get_image, get_name: s.image_get_name };
        (s.assembly_foreach)(find_image_cb, &mut state as *mut _ as *mut c_void);
        if !state.image.is_null() { return Some(state.image); }
    }
    None
}

// calls a void method with no arguments
unsafe fn invoke0(s: &MonoSyms, method: *mut c_void, obj: *mut c_void) -> *mut c_void {
    let mut exc: *mut c_void = std::ptr::null_mut();
    (s.runtime_invoke)(method, obj, std::ptr::null_mut(), &mut exc)
}

// calls a method with args array
unsafe fn invoke(s: &MonoSyms, method: *mut c_void, obj: *mut c_void, args: &mut [*mut c_void]) -> *mut c_void {
    let mut exc: *mut c_void = std::ptr::null_mut();
    (s.runtime_invoke)(method, obj, args.as_mut_ptr(), &mut exc)
}

// creates a new named GameObject and returns its object pointer
unsafe fn new_gameobject(r: &RenderState, name: &str) -> *mut c_void {
    let obj = (r.syms.object_new)(r.domain, r.go_class);
    if obj.is_null() { return std::ptr::null_mut(); }
    let cname = CString::new(name).unwrap();
    let mono_str = (r.syms.string_new)(r.domain, cname.as_ptr());
    let mut args = [mono_str];
    invoke(&r.syms, r.go_ctor_name, obj, &mut args);
    obj
}

// sets a GameObject's world position by accessing transform.set_position
unsafe fn set_object_position(r: &RenderState, go: *mut c_void, x: f32, y: f32, z: f32) {
    // get transform via AddComponent trick — we cached set_position on Transform
    // position is a struct so we pass a pointer to [f32; 3]
    let pos: [f32; 3] = [x, y, z];
    let mut args = [pos.as_ptr() as *mut c_void];
    invoke(&r.syms, r.set_position, go, &mut args);
}

// creates a TextMesh child on go and returns the TextMesh object
unsafe fn add_textmesh(r: &RenderState, go: *mut c_void) -> *mut c_void {
    let mut args = [r.textmesh_type];
    invoke(&r.syms, r.go_add_comp, go, &mut args)
}

// sets the .text string field on a TextMesh object
unsafe fn set_text(r: &RenderState, tm: *mut c_void, text: &str) {
    let cs = CString::new(text).unwrap();
    let mono_str = (r.syms.string_new)(r.domain, cs.as_ptr());
    (r.syms.field_set_value)(tm, r.text_field, mono_str as *mut c_void);
}

// sets fontSize
unsafe fn set_fontsize(r: &RenderState, tm: *mut c_void, size: i32) {
    (r.syms.field_set_value)(tm, r.fontsize_field, &size as *const i32 as *mut c_void);
}

// RGBA color packed as four bytes; TextMesh.color is a UnityEngine.Color (4 floats)
unsafe fn set_color(r: &RenderState, tm: *mut c_void, red: f32, green: f32, blue: f32, alpha: f32) {
    let color = [red, green, blue, alpha];
    (r.syms.field_set_value)(tm, r.color_field, color.as_ptr() as *mut c_void);
}

// shows/hides a GameObject
unsafe fn set_active(r: &RenderState, go: *mut c_void, active: bool) {
    let val: u8 = active as u8;
    let mut args = [&val as *const u8 as *mut c_void];
    invoke(&r.syms, r.go_set_active, go, &mut args);
}

// initialises all cached classes/fields and creates the button GameObjects
pub unsafe fn init_render() -> bool {
    let syms = match load_syms() {
        Some(s) => s,
        None => return false,
    };
    let domain = (syms.get_root_domain)();
    let ue_image = match find_ue_image(&syms) {
        Some(i) => i,
        None => return false,
    };

    let ns  = b"UnityEngine\0".as_ptr() as *const c_char;

    macro_rules! class {
        ($name:literal) => {{
            let c = (syms.class_from_name)(ue_image, ns, concat!($name, "\0").as_ptr() as *const c_char);
            if c.is_null() { return false; }
            c
        }};
    }
    macro_rules! method {
        ($class:expr, $name:literal, $nargs:expr) => {{
            let m = (syms.class_get_method)($class, concat!($name, "\0").as_ptr() as *const c_char, $nargs);
            if m.is_null() { return false; }
            m
        }};
    }
    macro_rules! field {
        ($class:expr, $name:literal) => {{
            let f = (syms.class_get_field)($class, concat!($name, "\0").as_ptr() as *const c_char);
            if f.is_null() { return false; }
            f
        }};
    }

    let go_class       = class!("GameObject");
    let transform_class = class!("Transform");
    let textmesh_class = class!("TextMesh");

    let go_ctor_name   = method!(go_class, ".ctor", 1);   // GameObject(string name)
    let go_set_active  = method!(go_class, "SetActive", 1);
    let go_add_comp    = method!(go_class, "AddComponent", 1); // AddComponent(Type)
    let set_position   = method!(transform_class, "set_position", 1);

    let text_field     = field!(textmesh_class, "m_Text");
    let color_field    = field!(textmesh_class, "m_Color");
    let fontsize_field = field!(textmesh_class, "m_FontSize");

    // get a System.Type instance for TextMesh so we can pass it to AddComponent
    let textmesh_type  = (syms.class_get_type)(textmesh_class);

    let mut r = RenderState {
        syms,
        domain,
        ue_image,
        go_class,
        go_ctor_name,
        go_set_active,
        go_add_comp,
        transform_class,
        set_position,
        textmesh_class,
        textmesh_type,
        text_field,
        color_field,
        fontsize_field,
        button_objects: Vec::new(),
        header_object: std::ptr::null_mut(),
        created: false,
    };

    // create the header object (title label)
    let header_go = new_gameobject(&r, "rm_header");
    if !header_go.is_null() {
        let tm = add_textmesh(&r, header_go);
        if !tm.is_null() {
            set_text(&r, tm, "randommenu");
            set_fontsize(&r, tm, 24);
            set_color(&r, tm, 1.0, 1.0, 1.0, 1.0);
        }
        set_active(&r, header_go, false);
        r.header_object = header_go;
    }

    // create one GameObject per mod
    for m in ALL_MODS {
        let go = new_gameobject(&r, &format!("rm_btn_{}", m.id));
        if !go.is_null() {
            let tm = add_textmesh(&r, go);
            if !tm.is_null() {
                set_text(&r, tm, m.name);
                set_fontsize(&r, tm, 16);
                set_color(&r, tm, 0.8, 0.8, 0.8, 1.0);
            }
            set_active(&r, go, false);
        }
        r.button_objects.push(go);
    }

    r.created = true;
    RENDER.get_or_init(|| std::sync::Mutex::new(r));
    true
}

// called every frame from ui::tick — updates label text/colors and positions
pub unsafe fn update(
    open: bool,
    head_x: f32, head_y: f32, head_z: f32,
    enabled_ids: &[String],
    selected_index: usize,
) {
    let lock = match RENDER.get() {
        Some(m) => m,
        None => return,
    };
    let r = match lock.try_lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if !r.created { return; }

    // show/hide header
    if !r.header_object.is_null() {
        set_active(&r, r.header_object, open);
        if open {
            // position title 0.15 above the top button
            let title_y = head_y + (ALL_MODS.len() as f32 / 2.0) * 0.07 + 0.12;
            set_object_position(&r, r.header_object, head_x, title_y, head_z + 0.5);
        }
    }

    const BTN_SPACING: f32 = 0.07;
    const PANEL_Z: f32 = 0.5;

    for (i, go) in r.button_objects.iter().enumerate() {
        if go.is_null() { continue; }
        set_active(&r, *go, open);
        if !open { continue; }

        // position this button
        let offset = (i as f32 - ALL_MODS.len() as f32 / 2.0) * BTN_SPACING;
        set_object_position(&r, *go, head_x, head_y + offset, head_z + PANEL_Z);

        // get the TextMesh child to update label color
        // enabled = green, selected = yellow, default = light grey
        let mod_id = ALL_MODS[i].id;
        let is_enabled = enabled_ids.iter().any(|s| s == mod_id);
        let is_selected = i == selected_index;

        // re-get the TextMesh via add_textmesh (returns existing if already added)
        let tm = add_textmesh(&r, *go);
        if tm.is_null() { continue; }

        let label = if is_enabled {
            format!("[ON]  {}", ALL_MODS[i].name)
        } else {
            format!("[OFF] {}", ALL_MODS[i].name)
        };
        set_text(&r, tm, &label);

        let (red, green, blue) = if is_selected {
            (1.0f32, 1.0f32, 0.0f32)   // yellow = cursor
        } else if is_enabled {
            (0.3f32, 1.0f32, 0.3f32)   // green = enabled
        } else {
            (0.8f32, 0.8f32, 0.8f32)   // grey = off
        };
        set_color(&r, tm, red, green, blue, 1.0);
    }
}
