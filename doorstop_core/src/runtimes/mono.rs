use std::{
    env,
    ffi::{CStr, CString, c_char, c_void},
    fs,
    io::ErrorKind,
    mem::DropGuard,
    path::{Path, PathBuf},
    str,
    sync::{
        Once, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, bail};
use bitflags::bitflags;
use doorstop_shared::OsStrExt;
use log::{info, trace, warn};

use crate::{
    fatal, get_config, hook_fn,
    utils::bindings::{BindingsStruct, bindings},
};

unsafe extern "C" {
    pub type MonoDomain;
    pub type MonoAssembly;
    pub type MonoMethodDesc;
    pub type MonoImage;
    pub type MonoMethod;
    pub type MonoObject;
}

#[allow(non_camel_case_types)]
type gboolean = i32;

#[allow(non_camel_case_types)]
type mono_bool = i32;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MonoDebugFormat {
    None = 0,
    Mono = 1,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MonoImageOpenStatus {
    OK,
    ErrorErrno,
    MissingAssemblyRef,
    ImageInvalid,
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct MonoProfileFlags: u32 {
        const NONE = 0;
        const APPDOMAIN_EVENTS = 1 << 0;
        const ASSEMBLY_EVENTS = 1 << 1;
        const MODULE_EVENTS = 1 << 2;
        const CLASS_EVENTS = 1 << 3;
        const JIT_COMPILATION = 1 << 4;
        const INLINING = 1 << 5;
        const EXCEPTIONS = 1 << 6;
        const ALLOCATIONS = 1 << 7;
        const GC = 1 << 8;
        const THREADS = 1 << 9;
        const REMOTING = 1 << 10;
        const TRANSITIONS = 1 << 11;
        const ENTER_LEAVE = 1 << 12;
        const COVERAGE = 1 << 13;
        const INS_COVERAGE = 1 << 14;
        const STATISTICAL = 1 << 15;
        const METHOD_EVENTS = 1 << 16;
        const MONITOR_EVENTS = 1 << 17;
        const IOMAP_EVENTS = 1 << 18;
        const GC_MOVES = 1 << 19;
    }
}

bindings! {
    struct Mono {
        // void mono_free(void* ptr);
        g_free: Option<unsafe extern "C" fn(ptr: *const c_char)>,
        mono_free: Option<unsafe extern "C" fn(ptr: *const c_char)>,
        mono_unity_g_free: Option<unsafe extern "C" fn(ptr: *const c_char)>,

        // const char* mono_assembly_getrootdir()
        mono_assembly_getrootdir: unsafe extern "C" fn() -> *const c_char,

        // void mono_set_assemblies_path(const char* path)
        mono_set_assemblies_path: unsafe extern "C" fn(path: *const c_char),

        // void mono_jit_parse_options(int argc, char* argv[])
        mono_jit_parse_options: unsafe extern "C" fn(argc: i32, argv: *const *const c_char),

        // mono_bool mono_debug_enabled()
        mono_debug_enabled: Option<unsafe extern "C" fn() -> mono_bool>,

        // void mono_debug_init (MonoDebugFormat format)
        mono_debug_init: unsafe extern "C" fn(format: MonoDebugFormat),

        // MonoProfileFlags mono_profiler_get_events()
        mono_profiler_get_events: Option<unsafe extern "C" fn() -> MonoProfileFlags>,

        // MonoDomain* mono_domain_get()
        mono_domain_get: unsafe extern "C" fn() -> *const MonoDomain,

        // MonoAssembly* mono_domain_assembly_open(MonoDomain* domain, const char* name)
        mono_domain_assembly_open: unsafe extern "C" fn(domain: *const MonoDomain, name: *const c_char) -> *const MonoAssembly,

        // MonoImage* mono_assembly_get_image(MonoAssembly* assembly)
        mono_assembly_get_image: unsafe extern "C" fn(assembly: *const MonoAssembly) -> *const MonoImage,

        // MonoMethod* mono_method_desc_search_in_image(MonoMethodDesc* desc, MonoImage* image)
        mono_method_desc_search_in_image: unsafe extern "C" fn(desc: *const MonoMethodDesc, image: *const MonoImage) -> *const MonoMethod,

        // MonoMethodDesc* mono_method_desc_new(const char* name, gboolean include_namespace)
        mono_method_desc_new: unsafe extern "C" fn(name: *const c_char, include_namespace: gboolean) -> *const MonoMethodDesc,

        // void mono_method_desc_free(MonoMethodDesc* desc)
        mono_method_desc_free: unsafe extern "C" fn(desc: *const MonoMethodDesc),

        // MonoObject* mono_runtime_invoke(MonoMethod* method, void* obj, void** params, MonoObject** exc)
        mono_runtime_invoke: unsafe extern "C" fn(method: *const MonoMethod, obj: *mut c_void, params: *mut *mut c_void, exc: *mut *const MonoObject) -> *const MonoObject,

        // void mono_print_unhandled_exception(MonoObject* exc)
        mono_print_unhandled_exception: unsafe extern "C" fn(exc: *const MonoObject),

        // MonoString* mono_object_to_string(MonoObject* obj, MonoObject** exc);
        mono_object_to_string: Option<unsafe extern "C" fn(obj: *const MonoObject, exc: *mut *const MonoObject) -> *const MonoObject>,

        // char* mono_string_to_utf8(MonoString* s)
        mono_string_to_utf8: Option<unsafe extern "C" fn(s: *const MonoObject) -> *const c_char>,
    }
}

impl Mono {
    unsafe fn free(&self, ptr: *const c_char) {
        if let Some(free) = self.mono_unity_g_free.or(self.mono_free).or(self.g_free) {
            unsafe { free(ptr) };
        } else {
            panic!("Couldn't find mono_free");
        }
    }
}

static MONO: OnceLock<Mono> = OnceLock::new();
static IS_DEBUG_ENABLED: OnceLock<()> = OnceLock::new();
static BOOTSTRAP_ONCE: Once = Once::new();

static DURING_MONO_INIT: AtomicBool = AtomicBool::new(false);

pub fn try_hook(module: *mut c_void, name: &str, address: *const c_void) -> Option<*const c_void> {
    if name.starts_with("mono_") {
        MONO.get_or_init(|| {
            #[cfg(target_os = "linux")]
            {
                use std::ffi::c_int;

                use libc::{SO_REUSEADDR, SOL_SOCKET, setsockopt, sockaddr, socklen_t};
                use plthook::ObjectFile;

                use crate::plt_hook;

                // Workaround mono's debugger socket getting stuck in TIME_WAIT state
                if get_config().mono_debug_enabled {
                    let object = unsafe { ObjectFile::open_by_handle(module).unwrap() };
                    plt_hook!(
                        &object,
                        "bind",
                        extern "system" fn(orig, sockfd: c_int, addr: *const sockaddr, addrlen: socklen_t) -> c_int,
                        {
                            if DURING_MONO_INIT.load(Ordering::Relaxed) {
                                info!("Enabling SO_REUSEADDR on mono's debugger socket");

                                unsafe {
                                    let value: c_int = 1;
                                    setsockopt(
                                        sockfd,
                                        SOL_SOCKET,
                                        SO_REUSEADDR,
                                        (&raw const value).cast(),
                                        socklen_t::try_from(size_of_val(&value)).unwrap(),
                                    );
                                }
                            }

                            unsafe { orig(sockfd, addr, addrlen) }
                        }
                    )
                    .unwrap();
                }
            }

            unsafe { Mono::load_raw(module) }.unwrap()
        });
    }

    match name {
        "mono_debug_init" => Some(hook_fn!(address, extern "C" fn(orig, format: MonoDebugFormat), {
            trace!("mono_debug_init({format:?})");
            IS_DEBUG_ENABLED.set(()).expect("mono_debug_init should not be called more than once");
            unsafe { orig(format) }
        }) as *const _),

        "mono_jit_init_version" => Some(hook_fn!(
            address,
            extern "C" fn(orig, root_domain_name: *const c_char, runtime_version: *const c_char) -> *const c_void,
            {
                unsafe {
                    trace!(
                        "mono_jit_init_version({:?}, {:?})",
                        CStr::from_ptr(root_domain_name),
                        CStr::from_ptr(runtime_version)
                    );
                }

                DURING_MONO_INIT.store(true, Ordering::Relaxed);

                let is_net35 = unsafe { CStr::from_ptr(runtime_version) }.to_bytes().starts_with(b"v2.");

                let mono = MONO.get().unwrap();
                init(mono, is_net35);

                let result = unsafe { orig(root_domain_name, runtime_version) };
                DURING_MONO_INIT.store(false, Ordering::Relaxed);
                result
            }
        ) as *const _),

        "mono_assembly_load_from_full" => Some(hook_fn!(
            address,
            extern "C" fn(orig, image: *const MonoImage, fname: *const c_char, status: *const i32, refonly: gboolean) -> *const MonoAssembly,
            {
                BOOTSTRAP_ONCE.call_once(|| fatal(bootstrap().context("Failed to bootstrap")));

                unsafe { orig(image, fname, status, refonly) }
            }
        ) as *const _),

        // Legacy mono's debugger-agent relied on profiler events, but production UnityPlayer resets them
        // Hook mono_profiler_set_events to make it cumulative
        "mono_profiler_set_events" => Some(hook_fn!(address, extern "C" fn(orig, events: MonoProfileFlags), {
            trace!("mono_profiler_set_events({events:?})");

            let mono = MONO.get().unwrap();
            if let Some(mono_profiler_get_events) = mono.mono_profiler_get_events {
                let current_events = unsafe { mono_profiler_get_events() };
                let events = current_events | events;
                trace!("Overriding profiler events: {events:?}");
                return unsafe { orig(events) };
            }

            unsafe { orig(events) }
        }) as *const _),

        "mono_image_open_from_data_with_name" => Some(hook_fn!(
            address,
            extern "C" fn(
                orig,
                data: *const c_char,
                data_len: u32,
                need_copy: gboolean,
                status: *mut MonoImageOpenStatus,
                refonly: gboolean,
                name: *const c_char,
            ) -> *const MonoImage,
            {
                if let Some(search_path_override) = get_config().mono_dll_search_path_override.as_ref() {
                    let path = unsafe { CStr::from_ptr(name) };
                    let path = PathBuf::from(path.to_str().unwrap());
                    if let Some(file_name) = path.file_name() {
                        for search_path in search_path_override.split(PATH_SEPARATOR) {
                            let new_path = Path::new(search_path).join(file_name);
                            match fs::read(&new_path) {
                                Err(err) if err.kind() == ErrorKind::NotFound => (),
                                r => {
                                    trace!("Overriding {} to {}", path.display(), new_path.display());

                                    let new_data = r.unwrap();
                                    let new_name = new_path.to_cstr().unwrap();

                                    return unsafe {
                                        orig(
                                            new_data.as_ptr().cast(),
                                            u32::try_from(new_data.len()).unwrap(),
                                            need_copy,
                                            status,
                                            refonly,
                                            new_name.as_ptr(),
                                        )
                                    };
                                }
                            }
                        }
                    }
                }

                unsafe { orig(data, data_len, need_copy, status, refonly, name) }
            }
        ) as *const _),

        _ => None,
    }
}

#[cfg(windows)]
const PATH_SEPARATOR: char = ';';

#[cfg(unix)]
const PATH_SEPARATOR: char = ':';

fn init(mono: &Mono, is_net35: bool) {
    unsafe {
        let config = get_config();

        let root_dir = CStr::from_ptr((mono.mono_assembly_getrootdir)()).to_str().unwrap();

        env::set_var("DOORSTOP_MANAGED_FOLDER_DIR", root_dir);

        if let Some(search_path_override) = get_config().mono_dll_search_path_override.as_ref() {
            let mut new_search_path = search_path_override.clone();
            new_search_path.push(PATH_SEPARATOR);
            new_search_path.push_str(root_dir);

            env::set_var("DOORSTOP_DLL_SEARCH_DIRS", &new_search_path);

            info!("Overriding search path to {new_search_path}");
            let new_search_path = CString::new(new_search_path).unwrap();
            (mono.mono_set_assemblies_path)(new_search_path.as_ptr());
        } else {
            env::set_var("DOORSTOP_DLL_SEARCH_DIRS", root_dir);
        }

        if config.mono_debug_enabled {
            let arg = if let Ok(args) = env::var("MONO_ARGUMENTS") {
                if args.contains("server=n") {
                    sleep(Duration::from_millis(250));
                }

                args
            } else {
                let mut arg = String::from("--debugger-agent=transport=dt_socket,embedding=1");

                arg.push_str(",server=");
                arg.push_str(if config.mono_debug_connect { "n" } else { "y" });

                arg.push_str(",address=");
                arg.push_str(config.mono_debug_address.as_ref().unwrap());

                if !config.mono_debug_suspend {
                    arg.push_str(",suspend=n");
                    if is_net35 && !config.mono_debug_connect {
                        arg.push_str(",defer=y");
                    }
                }

                arg
            };

            let arg = CString::new(arg).unwrap();
            let args = [arg.as_ptr()];
            (mono.mono_jit_parse_options)(i32::try_from(args.len()).unwrap(), args.as_ptr());
        }

        // Regardless of whether we enable debugging, mono_debug_init is needed for symbolized stacktrace on older Unity versions
        let mut is_debug_enabled = IS_DEBUG_ENABLED.get().is_some();
        if let Some(mono_debug_enabled) = mono.mono_debug_enabled {
            is_debug_enabled |= mono_debug_enabled() != 0;
        }

        if !is_debug_enabled {
            (mono.mono_debug_init)(MonoDebugFormat::Mono);
        }
    }
}

fn bootstrap() -> anyhow::Result<()> {
    unsafe {
        let mono = MONO.get().unwrap();
        let config = get_config();

        if let Some(target_assembly) = config.target_assembly.as_ref() {
            let domain = (mono.mono_domain_get)();
            assert!(!domain.is_null());

            let target_assembly_path = target_assembly.to_cstr().unwrap();

            let assembly = (mono.mono_domain_assembly_open)(domain, target_assembly_path.as_ptr());
            if assembly.is_null() {
                bail!("Failed to load target assembly");
            }

            let image = (mono.mono_assembly_get_image)(assembly);
            assert!(!image.is_null());

            let desc = (mono.mono_method_desc_new)(c"Doorstop.Entrypoint:Start".as_ptr(), 1);
            let desc = DropGuard::new(desc, |desc| (mono.mono_method_desc_free)(desc));
            assert!(!desc.is_null());

            let method = (mono.mono_method_desc_search_in_image)(*desc, image);
            if method.is_null() {
                bail!("Failed to find entrypoint method in target assembly");
            }

            let mut exc: *const MonoObject = std::ptr::null();
            (mono.mono_runtime_invoke)(method, std::ptr::null_mut(), std::ptr::null_mut(), &raw mut exc);

            if !exc.is_null() {
                if let Some(mono_object_to_string) = mono.mono_object_to_string
                    && let Some(mono_string_to_utf8) = mono.mono_string_to_utf8
                {
                    let string_object = mono_object_to_string(exc, std::ptr::null_mut());
                    let str = DropGuard::new(mono_string_to_utf8(string_object), |str| mono.free(str));
                    let str = CStr::from_ptr(*str);
                    bail!("Failed to invoke entrypoint method: {}", str.display());
                }

                (mono.mono_print_unhandled_exception)(exc);
                bail!("Failed to invoke entrypoint method");
            }
        } else {
            warn!("No target assembly specified, skipping bootstrap");
        }

        Ok(())
    }
}
