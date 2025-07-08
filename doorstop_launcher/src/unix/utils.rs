use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use anyhow::bail;

use crate::ProcessorArchitecture;

pub(super) fn get_executable_architectures(path: impl AsRef<Path>) -> anyhow::Result<Vec<ProcessorArchitecture>> {
    let mut file = File::open(path)?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;

    {
        const ELFMAG0: u8 = 0x7f;
        const ELFMAG1: u8 = b'E';
        const ELFMAG2: u8 = b'L';
        const ELFMAG3: u8 = b'F';

        const EM_X86_64: u16 = 62;
        const EM_386: u16 = 3;
        const EM_AARCH64: u16 = 183;
        const EM_ARM: u16 = 40;

        if magic == [ELFMAG0, ELFMAG1, ELFMAG2, ELFMAG3] {
            fn to_architecture(machine: u16) -> Option<ProcessorArchitecture> {
                match machine {
                    EM_X86_64 => Some(ProcessorArchitecture::X64),
                    EM_386 => Some(ProcessorArchitecture::X86),
                    EM_AARCH64 => Some(ProcessorArchitecture::Arm64),
                    EM_ARM => Some(ProcessorArchitecture::Arm),
                    _ => None,
                }
            }

            file.seek(SeekFrom::Start(18))?;
            let mut machine = [0u8; 2];
            file.read_exact(&mut machine)?;

            return match to_architecture(u16::from_le_bytes(machine)) {
                Some(architecture) => Ok(vec![architecture]),
                _ => bail!("Unknown architecture"),
            };
        }
    }

    {
        const CPU_ARCH_ABI64: u32 = 0x0100_0000;
        const CPU_TYPE_X86: u32 = 7;
        const CPU_TYPE_X86_64: u32 = CPU_TYPE_X86 | CPU_ARCH_ABI64;
        const CPU_TYPE_ARM: u32 = 12;
        const CPU_TYPE_ARM64: u32 = CPU_TYPE_ARM | CPU_ARCH_ABI64;

        fn to_architecture(cpu_type: u32) -> Option<ProcessorArchitecture> {
            match cpu_type {
                CPU_TYPE_X86_64 => Some(ProcessorArchitecture::X64),
                CPU_TYPE_X86 => Some(ProcessorArchitecture::X86),
                CPU_TYPE_ARM64 => Some(ProcessorArchitecture::Arm64),
                CPU_TYPE_ARM => Some(ProcessorArchitecture::Arm),
                _ => None,
            }
        }

        {
            const MH_MAGIC: u32 = 0xfeed_face;
            const MH_CIGAM: u32 = 0xcefa_edfe;
            const MH_MAGIC_64: u32 = 0xfeed_facf;
            const MH_CIGAM_64: u32 = 0xcffa_edfe;

            let magic = u32::from_ne_bytes(magic);

            if magic == MH_MAGIC || magic == MH_CIGAM || magic == MH_MAGIC_64 || magic == MH_CIGAM_64 {
                let mut cpu_type = [0u8; 4];
                file.read_exact(&mut cpu_type)?;

                let cpu_type = if magic == MH_CIGAM || magic == MH_CIGAM_64 {
                    u32::from_be_bytes(cpu_type)
                } else {
                    u32::from_le_bytes(cpu_type)
                };

                return match to_architecture(cpu_type) {
                    Some(architecture) => Ok(vec![architecture]),
                    _ => bail!("Unknown architecture"),
                };
            }
        }

        {
            const FAT_MAGIC: u32 = 0xcafe_babe;

            let magic = u32::from_be_bytes(magic);

            if magic == FAT_MAGIC {
                let mut number = [0u8; 4];
                file.read_exact(&mut number)?;

                let number = u32::from_be_bytes(number);

                let mut architectures = Vec::with_capacity(number as usize);

                for _ in 0..number {
                    let mut cpu_type = [0u8; 4];
                    file.read_exact(&mut cpu_type)?;
                    let cpu_type = u32::from_be_bytes(cpu_type);

                    match to_architecture(cpu_type) {
                        Some(architecture) => architectures.push(architecture),
                        _ => bail!("Unknown architecture"),
                    }

                    file.seek(SeekFrom::Current(16))?;
                }

                return Ok(architectures);
            }
        }
    }

    Ok(vec![])
}

pub(super) fn pick_architecture(architectures: &[ProcessorArchitecture]) -> Option<ProcessorArchitecture> {
    #[cfg(target_arch = "x86_64")]
    if architectures.contains(&ProcessorArchitecture::X64) {
        return Some(ProcessorArchitecture::X64);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if architectures.contains(&ProcessorArchitecture::X86) {
        return Some(ProcessorArchitecture::X86);
    }

    #[cfg(target_arch = "aarch64")]
    if architectures.contains(&ProcessorArchitecture::Arm64) {
        return Some(ProcessorArchitecture::Arm64);
    }

    None
}
