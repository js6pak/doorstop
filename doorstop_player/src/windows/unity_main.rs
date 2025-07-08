use windows::{
    Win32::{
        Foundation::{HINSTANCE, HMODULE},
        System::{
            Environment::GetCommandLineW,
            LibraryLoader::{GetModuleHandleW, GetProcAddress},
            Threading::{GetStartupInfoW, STARTUPINFOW},
        },
    },
    core::{PWSTR, s},
};

// extern "C" UNITY_API int UnityMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPWSTR lpCmdLine, int nShowCmd);
#[allow(non_snake_case)]
type FnUnityMain = unsafe extern "system" fn(hInstance: HINSTANCE, hPrevInstance: HMODULE, lpCmdLine: PWSTR, nShowCmd: i32) -> i32;

pub unsafe fn unity_main(unity_player_handle: HMODULE) {
    unsafe {
        let module_handle = GetModuleHandleW(None).unwrap();
        let command_line = GetCommandLineW();

        let mut startup_info = STARTUPINFOW::default();
        GetStartupInfoW(&raw mut startup_info);

        let unity_main: FnUnityMain =
            std::mem::transmute(GetProcAddress(unity_player_handle, s!("UnityMain")).expect("UnityPlayer.dll should contain UnityMain"));

        unity_main(
            module_handle.into(),
            HMODULE::default(),
            PWSTR(command_line.as_ptr().cast_mut()),
            i32::from(startup_info.wShowWindow),
        );
    }
}
