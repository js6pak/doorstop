use std::{
    ffi::{OsStr, c_void},
    io,
    path::PathBuf,
    slice,
};

use windows::{
    Win32::{
        Foundation::{HMODULE, TRUE},
        System::{
            LibraryLoader::{GetModuleFileNameW, GetProcAddress, LoadLibraryW},
            SystemInformation::GetSystemDirectoryW,
        },
    },
    core::{BOOL, HSTRING, PCSTR},
};

use crate::windows::utils::{fill_utf16_buf, os2path};

const VALID_PROXY_DLL_NAMES: &[&str] = &["winhttp.dll", "version.dll"];

#[allow(non_snake_case)]
#[cfg_attr(not(test), unsafe(no_mangle))]
#[cfg_attr(test, allow(unused))]
extern "system" fn DllMain(handle: HMODULE, reason: u32, _: *const c_void) -> BOOL {
    fn GetModuleFileName(hmodule: Option<HMODULE>) -> io::Result<PathBuf> {
        fill_utf16_buf(|buf, sz| unsafe { GetModuleFileNameW(hmodule, slice::from_raw_parts_mut(buf, sz)) }, os2path)
    }

    fn GetSystemDirectory() -> io::Result<PathBuf> {
        fill_utf16_buf(|buf, sz| unsafe { GetSystemDirectoryW(Some(slice::from_raw_parts_mut(buf, sz))) }, os2path)
    }

    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason != DLL_PROCESS_ATTACH {
        return TRUE;
    }

    let module_file_path = GetModuleFileName(Some(handle)).unwrap();
    if let Some(module_file_name) = module_file_path.file_name().and_then(OsStr::to_str)
        && VALID_PROXY_DLL_NAMES.iter().any(|&name| name.eq_ignore_ascii_case(module_file_name))
    {
        let mut path = GetSystemDirectory().unwrap();
        path.push(module_file_name);

        let path = HSTRING::from(path.as_os_str());
        let handle = unsafe { LoadLibraryW(&path).unwrap() };
        load_proxy_functions(handle);
    }

    TRUE
}

macro_rules! jump {
    ($address:expr) => {
        use std::arch::naked_asm;

        #[cfg(target_arch = "x86_64")]
        naked_asm!(
            "jmp qword ptr [rip + {}]",
            sym $address
        );

        #[cfg(target_arch = "x86")]
        naked_asm!(
            "jmp dword ptr [{}]",
            sym $address
        );

        #[cfg(target_arch = "aarch64")]
        naked_asm!(
            "adrp x16, {0}",
            "ldr x16, [x16, :lo12:{0}]",
            "br x16",
            sym $address
        );
    };
}

macro_rules! proxy {
    ($($name:ident),* $(,)?) => {
        $(
            #[allow(non_upper_case_globals)]
            static mut ${ concat(g_, $name) }: *const c_void = std::ptr::null();

            #[unsafe(naked)]
            #[unsafe(no_mangle)]
            extern "C" fn $name() {
                jump!(${ concat(g_, $name) });
            }
        )*

        fn load_proxy_functions(handle: HMODULE) {
            $(
                unsafe {
                    let name = PCSTR::from_raw(concat!(stringify!($name), '\0').as_ptr());
                    if let Some(address) = GetProcAddress(handle, name) {
                        ${ concat(g_, $name) } = address as *const _;
                    }
                }
            )*
        }
    };
}

proxy![
    GetFileVersionInfoA,
    GetFileVersionInfoByHandle,
    GetFileVersionInfoExA,
    GetFileVersionInfoExW,
    GetFileVersionInfoSizeA,
    GetFileVersionInfoSizeExA,
    GetFileVersionInfoSizeExW,
    GetFileVersionInfoSizeW,
    GetFileVersionInfoW,
    VerFindFileA,
    VerFindFileW,
    VerInstallFileA,
    VerInstallFileW,
    VerLanguageNameA,
    VerLanguageNameW,
    VerQueryValueA,
    VerQueryValueW,
    Private1,
    SvchostPushServiceGlobals,
    WinHttpAddRequestHeaders,
    WinHttpAutoProxySvcMain,
    WinHttpCheckPlatform,
    WinHttpCloseHandle,
    WinHttpConnect,
    WinHttpConnectionDeletePolicyEntries,
    WinHttpConnectionDeleteProxyInfo,
    WinHttpConnectionFreeNameList,
    WinHttpConnectionFreeProxyInfo,
    WinHttpConnectionFreeProxyList,
    WinHttpConnectionGetNameList,
    WinHttpConnectionGetProxyInfo,
    WinHttpConnectionGetProxyList,
    WinHttpConnectionSetPolicyEntries,
    WinHttpConnectionSetProxyInfo,
    WinHttpConnectionUpdateIfIndexTable,
    WinHttpCrackUrl,
    WinHttpCreateProxyResolver,
    WinHttpCreateUrl,
    WinHttpDetectAutoProxyConfigUrl,
    WinHttpFreeProxyResult,
    WinHttpFreeProxyResultEx,
    WinHttpFreeProxySettings,
    WinHttpGetDefaultProxyConfiguration,
    WinHttpGetIEProxyConfigForCurrentUser,
    WinHttpGetProxyForUrl,
    WinHttpGetProxyForUrlEx,
    WinHttpGetProxyForUrlEx2,
    WinHttpGetProxyForUrlHvsi,
    WinHttpGetProxyResult,
    WinHttpGetProxyResultEx,
    WinHttpGetProxySettingsVersion,
    WinHttpGetTunnelSocket,
    WinHttpOpen,
    WinHttpOpenRequest,
    WinHttpPacJsWorkerMain,
    WinHttpProbeConnectivity,
    WinHttpQueryAuthSchemes,
    WinHttpQueryDataAvailable,
    WinHttpQueryHeaders,
    WinHttpQueryOption,
    WinHttpReadData,
    WinHttpReadProxySettings,
    WinHttpReadProxySettingsHvsi,
    WinHttpReceiveResponse,
    WinHttpResetAutoProxy,
    WinHttpSaveProxyCredentials,
    WinHttpSendRequest,
    WinHttpSetCredentials,
    WinHttpSetDefaultProxyConfiguration,
    WinHttpSetOption,
    WinHttpSetStatusCallback,
    WinHttpSetTimeouts,
    WinHttpTimeFromSystemTime,
    WinHttpTimeToSystemTime,
    WinHttpWebSocketClose,
    WinHttpWebSocketCompleteUpgrade,
    WinHttpWebSocketQueryCloseStatus,
    WinHttpWebSocketReceive,
    WinHttpWebSocketSend,
    WinHttpWebSocketShutdown,
    WinHttpWriteData,
    WinHttpWriteProxySettings,
];
