#[macro_export]
macro_rules! hook_fn {
    (extern $abi:literal fn($orig:ident, $($param:ident: $param_type:ty),* $(,)?) $(-> $return_type:ty)?, $body:block) => {{
        use std::mem::MaybeUninit;

        static mut ORIGINAL_FN: MaybeUninit<unsafe extern $abi fn($($param: $param_type),*) $(-> $return_type)?> = MaybeUninit::uninit();

        unsafe extern $abi fn hook($($param: $param_type),*) $(-> $return_type)? {
            let $orig = unsafe { ORIGINAL_FN.assume_init() };
            $body
        }

        #[allow(static_mut_refs)]
        (hook, unsafe { &mut ORIGINAL_FN })
    }};

    ($original_address:expr, extern $abi:literal fn($orig:ident, $($param:ident: $param_type:ty),* $(,)?) $(-> $return_type:ty)?, $body:block) => {{
        use std::mem::MaybeUninit;

        let (hook, original_fn) = hook_fn!(extern $abi fn($orig, $($param: $param_type),*) $(-> $return_type)?, $body);

        let original_address: *const c_void = $original_address;
        #[allow(clippy::missing_transmute_annotations)]
        unsafe { *original_fn = MaybeUninit::new(std::mem::transmute(original_address)) };
        hook
    }};
}

#[macro_export]
macro_rules! plt_hook {
    ($object:expr, $symbol_name:expr, extern $abi:literal fn($orig:ident, $($param:ident: $param_type:ty),* $(,)?) $(-> $return_type:ty)?, $body:block) => {{
        (|| -> plthook::Result<()> {
            use std::mem::MaybeUninit;

            let (hook, original_fn) = $crate::hook_fn!(extern $abi fn($orig, $($param: $param_type),*) $(-> $return_type)?, $body);

            let object: &plthook::ObjectFile = $object;
            let symbol_name: &str = $symbol_name;
            let mut entry = unsafe { object.replace(symbol_name, hook as *const _)? };

            let original_address = {
                #[cfg(windows)]
                {
                    entry.original_address()
                }

                #[cfg(unix)]
                {
                    if std::env::var("LD_BIND_NOW").map(|val| val == "1").unwrap_or(false) {
                        entry.original_address()
                    } else {
                        // PLT's lazy binding would overwrite our hook on the first call, so resolve the symbol ourselves
                        let symbol_name = std::ffi::CString::new(symbol_name).unwrap();
                        unsafe { libc::dlsym(libc::RTLD_NEXT, symbol_name.as_ptr()) }
                    }
                }
            };

            #[allow(clippy::missing_transmute_annotations)]
            unsafe { *original_fn = MaybeUninit::new(std::mem::transmute(original_address)) };

            entry.discard();

            Ok(())
        })()
    }};
}
