use std::ffi::c_void;

use eager2::eager_macro;

pub trait BindingsStruct
where
    Self: Sized,
{
    unsafe fn load(library: &libloading::Library) -> anyhow::Result<Self>;
    unsafe fn load_raw(module: *mut c_void) -> anyhow::Result<Self>;
}

#[eager_macro]
macro_rules! bindings {
    (
        // extern $default_abi:literal struct $struct_name:ident {
        $vis:vis struct $struct_name:ident {
            $(
                $field_name:ident: $field_type:ty
            ),* $(,)?
        }
    ) => {
        use eager2::eager;


        #[allow(clippy::struct_field_names)]
        #[derive(Debug)]
        $vis struct $struct_name {
            $(
                // TODO rust-rover analysis breaks when eager! is used, so we can't use it in signatures, report upstream
                // pub $field_name: eager! { bindings!(@rewrite_type $default_abi, $field_type) },
                pub $field_name: $field_type,
            )*
        }

        impl BindingsStruct for $struct_name {
            unsafe fn load(library: &libloading::Library) -> anyhow::Result<Self> {
                unsafe {
                    Ok(Self {
                        $(
                            // $field_name: eager! { bindings!(@load library, $field_name, bindings!(@rewrite_type $default_abi, $field_type) ) },
                            $field_name: eager! { bindings!(@load library, $field_name, $field_type) },
                        )*
                    })
                }
            }

            unsafe fn load_raw(module: *mut c_void) -> anyhow::Result<Self> {
                let library = libloading::Library::from(unsafe {
                    #[cfg(windows)]
                    {
                        libloading::os::windows::Library::from_raw(module as _)
                    }

                    #[cfg(unix)]
                    {
                        libloading::os::unix::Library::from_raw(module)
                    }
                });
                let result = unsafe { Self::load(&library) }?;
                std::mem::forget(library);
                Ok(result)
            }
        }
    };

    // (@rewrite_type $default_abi:literal, $(unsafe)? fn($($params:tt)*) $(-> $return_type:ty)?) => {
    //     bindings!(@rewrite_type $default_abi, unsafe extern $default_abi fn($($params)*) $(-> $return_type)?)
    // };
    // (@rewrite_type $default_abi:literal, $(unsafe)? extern $abi:literal fn($($param:ident: $param_type:ty),* $(,)?) $(-> $return_type:ty)?) => {
    //     unsafe extern $abi fn($($param: $param_type),*) $(-> $return_type)?
    // };
    // (@rewrite_type $default_abi:literal, Option<$inner:ty>) => {
    //     Option<bindings!(@rewrite_type $default_abi, $inner)>
    // };

    (@load $library:ident, $symbol_name:ident, Option<$signature:ty>) => {
        $library.get::<$signature>(concat!(stringify!($symbol_name), "\0").as_bytes()).map_or(None, |s| Some(*s))
    };

    (@load $library:ident, $symbol_name:ident, $signature:ty) => {
        *$library.get::<$signature>(concat!(stringify!($symbol_name), "\0").as_bytes()).context(format!("failed to get symbol `{}`", stringify!($symbol_name)))?
    };
}

pub(crate) use bindings;
