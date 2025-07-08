use std::{
    env,
    ffi::{CStr, CString, c_char, c_void},
    fs, mem, ptr,
    str::FromStr,
};

use anyhow::{Context, bail};
use const_format::{concatcp, formatc};
use doorstop_shared::OsStrExt;
use log::{error, warn};

use crate::{
    fatal, get_config, hook_fn,
    utils::bindings::{BindingsStruct, bindings},
};

// TODO is system stdcall here correct? (check if its stdcall on win-x86)
#[allow(non_camel_case_types)]
type coreclr_error_writer_callback_fn = unsafe extern "system" fn(message: *const c_char);

bindings! {
    struct CoreCLR {
        coreclr_set_error_writer: Option<unsafe extern "system" fn(error_writer: coreclr_error_writer_callback_fn)>,
        coreclr_initialize: unsafe extern "system" fn(
            exe_path: *const c_char,
            app_domain_friendly_name: *const c_char,
            property_count: i32,
            property_keys: *const *const c_char,
            property_values: *const *const c_char,
            host_handle: *mut *const c_void,
            domain_id: *mut u32,
        ) -> i32,
        coreclr_create_delegate: unsafe extern "system" fn(
            host_handle: *const c_void,
            domain_id: u32,
            entry_point_assembly_name: *const c_char,
            entry_point_type_name: *const c_char,
            entry_point_method_name: *const c_char,
            delegate: *mut *const c_void,
        ) -> i32,
    }
}

pub fn try_hook(_module: *mut c_void, name: &str, address: *const c_void) -> Option<*const c_void> {
    match name {
        "il2cpp_init" => Some(hook_fn!(address, extern "C" fn(orig, domain_name: *const c_char) -> i32, {
            let result = unsafe { orig(domain_name) };
            fatal(bootstrap().context("Failed to bootstrap CoreCLR"));
            result
        }) as *const _),

        _ => None,
    }
}

extern "system" fn error_writer_callback(message: *const c_char) {
    let message = unsafe { CStr::from_ptr(message) };
    error!("coreclr: {}", message.display());
}

fn bootstrap() -> anyhow::Result<()> {
    unsafe {
        let config = get_config();

        let Some(target_assembly) = config.target_assembly.as_ref() else {
            warn!("No target assembly specified, skipping bootstrap");
            return Ok(());
        };

        let coreclr_path = config
            .clr_corlib_dir
            .as_ref()
            .map(|x| x.join(format!("{}coreclr{}", env::consts::DLL_PREFIX, env::consts::DLL_SUFFIX)));
        let Some(coreclr_path) = config.clr_runtime_coreclr_path.as_ref().or(coreclr_path.as_ref()) else {
            warn!("No coreclr path specified, skipping bootstrap");
            return Ok(());
        };

        let clr_corlib_dir = config.clr_corlib_dir.as_ref().unwrap();

        let lib = libloading::Library::new(coreclr_path)?;
        let coreclr = CoreCLR::load(&lib)?;

        if let Some(coreclr_set_error_writer) = coreclr.coreclr_set_error_writer {
            coreclr_set_error_writer(error_writer_callback);
        }

        let target_dir = target_assembly.parent().unwrap();
        let app_paths = env::join_paths([clr_corlib_dir, target_dir])?;
        let app_paths_cstr = app_paths.to_cstr().unwrap();

        let native_paths = env::join_paths([target_dir.join(formatc!("runtimes/{DOTNET_RID}/native"))])?;
        let native_paths_cstr = native_paths.to_cstr().unwrap();

        let mut property_keys: Vec<*const c_char> = vec![];
        let mut property_values: Vec<*const c_char> = vec![];

        property_keys.push(c"APP_PATHS".as_ptr());
        property_values.push(app_paths_cstr.as_ptr());

        property_keys.push(c"NATIVE_DLL_SEARCH_DIRECTORIES".as_ptr());
        property_values.push(native_paths_cstr.as_ptr());

        assert_eq!(property_keys.len(), property_values.len());

        let mut host: *const c_void = ptr::null();
        let mut domain_id: u32 = 0;
        let result = (coreclr.coreclr_initialize)(
            env::current_exe()?.to_cstr().unwrap().as_ptr(),
            c"Doorstop Domain".as_ptr(),
            i32::try_from(property_keys.len()).unwrap(),
            property_keys.as_mut_ptr(),
            property_values.as_mut_ptr(),
            &raw mut host,
            &raw mut domain_id,
        );
        if result != 0 {
            bail!("Failed to initialize CoreCLR ({result:X})");
        }

        let target_assembly_name = CString::from_str(target_assembly.file_stem().unwrap().to_str().unwrap())?;
        if !fs::exists(target_assembly)? {
            bail!("Failed to load target assembly");
        }

        let mut startup: Option<unsafe extern "system" fn()> = None;
        let result = (coreclr.coreclr_create_delegate)(
            host,
            domain_id,
            target_assembly_name.as_ptr(),
            c"Doorstop.Entrypoint".as_ptr(),
            c"Start".as_ptr(),
            (&raw mut startup).cast(),
        );
        if result != 0 {
            bail!("Failed to find entrypoint method in target assembly ({result:X})");
        }

        env::set_var("DOORSTOP_MANAGED_FOLDER_DIR", clr_corlib_dir);
        env::set_var("DOORSTOP_DLL_SEARCH_DIRS", app_paths);

        startup.unwrap()();

        mem::forget(lib);
    }

    Ok(())
}

const DOTNET_RID: &str = concatcp!(
    {
        #[cfg(windows)]
        {
            "win"
        }

        #[cfg(target_os = "macos")]
        {
            "osx"
        }

        #[cfg(target_os = "linux")]
        {
            "linux"
        }
    },
    "-",
    {
        #[cfg(target_arch = "x86_64")]
        {
            "x64"
        }

        #[cfg(target_arch = "x86")]
        {
            "x86"
        }

        #[cfg(target_arch = "aarch64")]
        {
            "arm64"
        }
    }
);
