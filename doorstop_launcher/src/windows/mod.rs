use std::{
    os::windows::{io::FromRawHandle, process::ExitCodeExt},
    process::ExitCode,
    slice,
};

use anyhow::Context;
use mini_syringe::{Syringe, process::OwnedProcess};
use windows::{
    Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, HANDLE, WAIT_FAILED},
        System::{
            Environment::GetCommandLineW,
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JobObjectExtendedLimitInformation, SetInformationJobObject,
            },
            Threading::{CREATE_SUSPENDED, CreateProcessW, GetExitCodeProcess, INFINITE, PROCESS_INFORMATION, ResumeThread, STARTUPINFOW, WaitForSingleObject},
        },
    },
    core::{Error, PCWSTR, PWSTR},
};

use crate::{ProcessorArchitecture, get_doorstop_path, windows::utils::strip_first_arg};

mod utils;

#[must_use]
pub fn main() -> ExitCode {
    try_main().unwrap_or_else(|err| {
        eprintln!("[doorstop_launcher] {err:#}");
        ExitCode::from(1)
    })
}

/// Assigns the process to a job that takes all descendants down with it.
unsafe fn assign_to_job(process: HANDLE) -> anyhow::Result<()> {
    unsafe {
        let job = CreateJobObjectW(None, PCWSTR::null()).context("CreateJobObjectW failed")?;

        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            u32::try_from(size_of_val(&info)).unwrap(),
        )
        .context("SetInformationJobObject failed")?;

        AssignProcessToJobObject(job, process).context("AssignProcessToJobObject failed")?;

        Ok(())
    }
}

fn try_main() -> anyhow::Result<ExitCode> {
    unsafe {
        let command_line = GetCommandLineW();
        let command_line = strip_first_arg(slice::from_raw_parts(command_line.0, command_line.len() + 1));

        if command_line.is_empty() || command_line[0] == 0 {
            eprintln!("usage: doorstop_launcher COMMAND [ARGS]");
            return Ok(ExitCode::from(1));
        }

        let mut startup_info = STARTUPINFOW::default();
        startup_info.cb = u32::try_from(size_of_val(&startup_info)).unwrap();

        let mut process_information = PROCESS_INFORMATION::default();

        let mut create_process = |inherit_handles: bool| {
            CreateProcessW(
                PCWSTR::null(),
                Some(PWSTR(command_line.as_ptr().cast_mut())),
                None,
                None,
                inherit_handles,
                CREATE_SUSPENDED,
                None,
                None,
                &raw const startup_info,
                &raw mut process_information,
            )
        };

        // Sometimes creating a process with bInheritHandles=true fails with INVALID_PARAMETER, not sure why
        // TODO investigate
        match create_process(true) {
            Err(e) if e.code() == ERROR_INVALID_PARAMETER.to_hresult() => create_process(false),
            r => r,
        }
        .context("CreateProcess failed")?;

        let process = process_information.hProcess;

        assign_to_job(process)?;

        let owned_process = OwnedProcess::from_raw_handle(process.0);

        let architecture = {
            #[cfg(target_arch = "x86_64")]
            {
                use mini_syringe::process::Process;

                if owned_process.is_x86().context("Failed to determine whether the process is 32-bit")? {
                    ProcessorArchitecture::X86
                } else {
                    ProcessorArchitecture::X64
                }
            }

            #[cfg(target_arch = "x86")]
            {
                ProcessorArchitecture::X86
            }

            #[cfg(target_arch = "aarch64")]
            {
                ProcessorArchitecture::Arm64
            }
        };

        let doorstop_path = get_doorstop_path(Some(architecture))?;

        let syringe = Syringe::for_suspended_process(owned_process).context("Failed to initialize the suspended process")?;
        syringe.inject(doorstop_path).context("Failed to inject doorstop")?;

        if ResumeThread(process_information.hThread) == u32::MAX {
            Err(Error::from_thread()).context("ResumeThread failed")?;
        }

        if WaitForSingleObject(process, INFINITE) == WAIT_FAILED {
            Err(Error::from_thread()).context("WaitForSingleObject failed")?;
        }

        let mut exit_code = 1;
        GetExitCodeProcess(process, &raw mut exit_code).context("GetExitCodeProcess failed")?;
        Ok(ExitCode::from_raw(exit_code))
    }
}
