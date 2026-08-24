//! Verifies that the portable Windows executable has no developer-runtime DLL dependency.
//! plan_ref: docs/plan/11_testing_and_release.md#portable-windows-runtime

use std::fs;
use std::path::Path;

const PE32_PLUS_MAGIC: u16 = 0x20b;
const X86_64_MACHINE: u16 = 0x8664;
const IMPORT_DIRECTORY_INDEX: usize = 1;
const DELAY_IMPORT_DIRECTORY_INDEX: usize = 13;
const IMPORT_DESCRIPTOR_SIZE: u32 = 20;
const DELAY_IMPORT_DESCRIPTOR_SIZE: u32 = 32;
const MAX_IMPORT_DESCRIPTORS: usize = 4_096;
const MAX_DLL_NAME_BYTES: usize = 1_024;

#[derive(Clone, Copy, Debug)]
struct Section {
    virtual_address: u32,
    virtual_size: u32,
    raw_offset: u32,
    raw_size: u32,
}

#[derive(Debug)]
struct PeImage<'a> {
    bytes: &'a [u8],
    optional_header: usize,
    optional_header_size: usize,
    size_of_headers: u32,
    image_base: u64,
    sections: Vec<Section>,
}

/// Describes the DLL imports declared by one PE32+ executable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NativeDependencyReport {
    pub(crate) imports: Vec<String>,
}

/// Parses normal and delay-load imports and rejects redistributable/developer runtimes.
pub(crate) fn verify_portable_executable(path: &Path) -> Result<NativeDependencyReport, String> {
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "cannot read portable executable `{}`: {error}",
            path.display()
        )
    })?;
    let report = inspect_bytes(&bytes)?;
    let forbidden = report
        .imports
        .iter()
        .filter(|name| is_developer_runtime(name))
        .cloned()
        .collect::<Vec<_>>();
    if !forbidden.is_empty() {
        return Err(format!(
            "portable executable imports developer runtime DLL(s): {}",
            forbidden.join(", ")
        ));
    }
    let unsupported = report
        .imports
        .iter()
        .filter(|name| !is_windows_inbox_dependency(name))
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(format!(
            "portable executable imports DLL(s) outside the approved Windows inbox set: {}",
            unsupported.join(", ")
        ));
    }
    Ok(report)
}

fn inspect_bytes(bytes: &[u8]) -> Result<NativeDependencyReport, String> {
    let image = PeImage::parse(bytes)?;
    let mut imports = Vec::new();
    image.read_standard_imports(&mut imports)?;
    image.read_delay_imports(&mut imports)?;
    imports.sort_by_key(|name| name.to_ascii_lowercase());
    imports.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    Ok(NativeDependencyReport { imports })
}

fn is_developer_runtime(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".dll").unwrap_or(&lower);
    stem.starts_with("vcruntime")
        || stem.starts_with("msvcp")
        || stem.starts_with("concrt")
        || stem == "ucrtbased"
        || stem.starts_with("libgcc_s")
        || stem.starts_with("libstdc++")
        || stem.starts_with("libwinpthread")
        || stem.strip_prefix("msvcr").is_some_and(|version| {
            !version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn is_windows_inbox_dependency(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("api-ms-win-")
        || lower.starts_with("ext-ms-win-")
        || matches!(
            lower.as_str(),
            "advapi32.dll"
                | "bcryptprimitives.dll"
                | "combase.dll"
                | "comctl32.dll"
                | "dwmapi.dll"
                | "gdi32.dll"
                | "imm32.dll"
                | "kernel32.dll"
                | "msvcrt.dll"
                | "ntdll.dll"
                | "ole32.dll"
                | "oleaut32.dll"
                | "shell32.dll"
                | "user32.dll"
                | "uxtheme.dll"
        )
}

impl<'a> PeImage<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.get(..2) != Some(b"MZ") {
            return Err("portable executable has no DOS MZ signature".to_owned());
        }
        let pe_offset = read_u32(bytes, 0x3c)? as usize;
        if bytes.get(pe_offset..pe_offset.saturating_add(4)) != Some(b"PE\0\0") {
            return Err("portable executable has no PE signature".to_owned());
        }
        let coff = pe_offset
            .checked_add(4)
            .ok_or_else(|| "PE header offset overflow".to_owned())?;
        if read_u16(bytes, coff)? != X86_64_MACHINE {
            return Err("portable executable is not x86_64".to_owned());
        }
        let section_count = usize::from(read_u16(bytes, coff + 2)?);
        let optional_header_size = usize::from(read_u16(bytes, coff + 16)?);
        let optional_header = coff
            .checked_add(20)
            .ok_or_else(|| "optional-header offset overflow".to_owned())?;
        let optional_end = optional_header
            .checked_add(optional_header_size)
            .ok_or_else(|| "optional-header size overflow".to_owned())?;
        if optional_end > bytes.len() || read_u16(bytes, optional_header)? != PE32_PLUS_MAGIC {
            return Err("portable executable is not a complete PE32+ image".to_owned());
        }
        let image_base = read_u64(bytes, optional_header + 24)?;
        let size_of_headers = read_u32(bytes, optional_header + 60)?;
        let section_table = optional_end;
        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let offset = section_table
                .checked_add(index.saturating_mul(40))
                .ok_or_else(|| "section-table offset overflow".to_owned())?;
            if offset.checked_add(40).is_none_or(|end| end > bytes.len()) {
                return Err("portable executable has a truncated section table".to_owned());
            }
            sections.push(Section {
                virtual_size: read_u32(bytes, offset + 8)?,
                virtual_address: read_u32(bytes, offset + 12)?,
                raw_size: read_u32(bytes, offset + 16)?,
                raw_offset: read_u32(bytes, offset + 20)?,
            });
        }
        Ok(Self {
            bytes,
            optional_header,
            optional_header_size,
            size_of_headers,
            image_base,
            sections,
        })
    }

    fn read_standard_imports(&self, imports: &mut Vec<String>) -> Result<(), String> {
        let Some((rva, size)) = self.data_directory(IMPORT_DIRECTORY_INDEX)? else {
            return Ok(());
        };
        self.read_descriptors(rva, size, IMPORT_DESCRIPTOR_SIZE, |descriptor| {
            let name_rva = read_u32(self.bytes, descriptor + 12)?;
            imports.push(self.read_dll_name(name_rva)?);
            Ok(())
        })
    }

    fn read_delay_imports(&self, imports: &mut Vec<String>) -> Result<(), String> {
        let Some((rva, size)) = self.data_directory(DELAY_IMPORT_DIRECTORY_INDEX)? else {
            return Ok(());
        };
        self.read_descriptors(rva, size, DELAY_IMPORT_DESCRIPTOR_SIZE, |descriptor| {
            let attributes = read_u32(self.bytes, descriptor)?;
            let name = u64::from(read_u32(self.bytes, descriptor + 4)?);
            let name_rva = if attributes & 1 != 0 {
                u32::try_from(name).map_err(|_| "delay-import name RVA overflow".to_owned())?
            } else {
                let rva = name
                    .checked_sub(self.image_base)
                    .ok_or_else(|| "delay-import name VA precedes the PE image base".to_owned())?;
                u32::try_from(rva).map_err(|_| "delay-import name RVA overflow".to_owned())?
            };
            imports.push(self.read_dll_name(name_rva)?);
            Ok(())
        })
    }

    fn read_descriptors(
        &self,
        table_rva: u32,
        table_size: u32,
        descriptor_size: u32,
        mut read_descriptor: impl FnMut(usize) -> Result<(), String>,
    ) -> Result<(), String> {
        let declared = if table_size == 0 {
            MAX_IMPORT_DESCRIPTORS
        } else {
            usize::try_from(table_size / descriptor_size)
                .unwrap_or(MAX_IMPORT_DESCRIPTORS)
                .saturating_add(1)
                .min(MAX_IMPORT_DESCRIPTORS)
        };
        for index in 0..declared {
            let index = u32::try_from(index).map_err(|_| "import index overflow".to_owned())?;
            let descriptor_rva = table_rva
                .checked_add(index.saturating_mul(descriptor_size))
                .ok_or_else(|| "import descriptor RVA overflow".to_owned())?;
            let descriptor = self.rva_to_offset(descriptor_rva)?;
            let end = descriptor
                .checked_add(descriptor_size as usize)
                .ok_or_else(|| "import descriptor offset overflow".to_owned())?;
            let descriptor_bytes = self.bytes.get(descriptor..end).ok_or_else(|| {
                "portable executable has a truncated import descriptor".to_owned()
            })?;
            if descriptor_bytes.iter().all(|byte| *byte == 0) {
                return Ok(());
            }
            read_descriptor(descriptor)?;
        }
        Err("portable executable import table has no bounded terminator".to_owned())
    }

    fn data_directory(&self, index: usize) -> Result<Option<(u32, u32)>, String> {
        let directory_count = usize::try_from(read_u32(self.bytes, self.optional_header + 108)?)
            .map_err(|_| "PE data-directory count overflow".to_owned())?;
        if index >= directory_count {
            return Ok(None);
        }
        let offset = self
            .optional_header
            .checked_add(112)
            .and_then(|offset| offset.checked_add(index.saturating_mul(8)))
            .ok_or_else(|| "PE data-directory offset overflow".to_owned())?;
        let end = offset
            .checked_add(8)
            .ok_or_else(|| "PE data-directory end overflow".to_owned())?;
        if end > self.optional_header + self.optional_header_size {
            return Err("PE data directory extends beyond the optional header".to_owned());
        }
        let rva = read_u32(self.bytes, offset)?;
        let size = read_u32(self.bytes, offset + 4)?;
        Ok((rva != 0).then_some((rva, size)))
    }

    fn read_dll_name(&self, rva: u32) -> Result<String, String> {
        let offset = self.rva_to_offset(rva)?;
        let tail = self
            .bytes
            .get(offset..)
            .ok_or_else(|| "DLL name offset is outside the PE image".to_owned())?;
        let length = tail
            .iter()
            .take(MAX_DLL_NAME_BYTES)
            .position(|byte| *byte == 0)
            .ok_or_else(|| "DLL name has no bounded NUL terminator".to_owned())?;
        let name = std::str::from_utf8(&tail[..length])
            .map_err(|_| "DLL name is not ASCII-compatible UTF-8".to_owned())?;
        if name.is_empty() || !name.is_ascii() {
            return Err("DLL import name is empty or non-ASCII".to_owned());
        }
        Ok(name.to_owned())
    }

    fn rva_to_offset(&self, rva: u32) -> Result<usize, String> {
        if rva < self.size_of_headers {
            let offset = rva as usize;
            if offset < self.bytes.len() {
                return Ok(offset);
            }
        }
        for section in &self.sections {
            let span = section.virtual_size.max(section.raw_size);
            let Some(end) = section.virtual_address.checked_add(span) else {
                continue;
            };
            if (section.virtual_address..end).contains(&rva) {
                let delta = rva - section.virtual_address;
                if delta >= section.raw_size {
                    return Err("PE RVA points into an uninitialized section tail".to_owned());
                }
                let raw = section
                    .raw_offset
                    .checked_add(delta)
                    .ok_or_else(|| "PE raw-file offset overflow".to_owned())?
                    as usize;
                if raw < self.bytes.len() {
                    return Ok(raw);
                }
            }
        }
        Err(format!("PE RVA 0x{rva:08x} is not backed by file data"))
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset.saturating_add(2))
        .ok_or_else(|| "portable executable is truncated".to_owned())?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset.saturating_add(4))
        .ok_or_else(|| "portable executable is truncated".to_owned())?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let value = bytes
        .get(offset..offset.saturating_add(8))
        .ok_or_else(|| "portable executable is truncated".to_owned())?;
    Ok(u64::from_le_bytes([
        value[0], value[1], value[2], value[3], value[4], value[5], value[6], value[7],
    ]))
}

#[cfg(test)]
mod tests {
    use super::{inspect_bytes, is_developer_runtime, is_windows_inbox_dependency};

    #[test]
    fn parses_standard_and_delay_load_dependency_tables() {
        let image = synthetic_pe(Some("KERNEL32.dll"), Some("SHELL32.dll"));
        let report = inspect_bytes(&image).expect("valid synthetic PE");
        assert_eq!(report.imports, ["KERNEL32.dll", "SHELL32.dll"]);
    }

    #[test]
    fn rejects_dynamic_developer_runtimes_but_allows_windows_api_sets() {
        for forbidden in [
            "VCRUNTIME140.dll",
            "VCRUNTIME140_1.dll",
            "MSVCP140.dll",
            "MSVCR120.dll",
            "CONCRT140.dll",
            "ucrtbased.dll",
            "libgcc_s_seh-1.dll",
            "libstdc++-6.dll",
            "libwinpthread-1.dll",
        ] {
            assert!(is_developer_runtime(forbidden), "{forbidden}");
        }
        for windows_inbox in [
            "KERNEL32.dll",
            "msvcrt.dll",
            "api-ms-win-crt-runtime-l1-1-0.dll",
        ] {
            assert!(!is_developer_runtime(windows_inbox), "{windows_inbox}");
            assert!(
                is_windows_inbox_dependency(windows_inbox),
                "{windows_inbox}"
            );
        }
        assert!(!is_windows_inbox_dependency("third-party-helper.dll"));
    }

    #[test]
    fn malformed_pe_fails_closed() {
        let error = inspect_bytes(b"not a PE image").expect_err("malformed image must fail");
        assert!(error.contains("MZ"));
    }

    fn synthetic_pe(standard: Option<&str>, delayed: Option<&str>) -> Vec<u8> {
        let mut bytes = vec![0_u8; 0x500];
        bytes[0..2].copy_from_slice(b"MZ");
        put_u32(&mut bytes, 0x3c, 0x80);
        bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
        put_u16(&mut bytes, 0x84, 0x8664);
        put_u16(&mut bytes, 0x86, 1);
        put_u16(&mut bytes, 0x94, 0xf0);
        let optional = 0x98;
        put_u16(&mut bytes, optional, 0x20b);
        put_u64(&mut bytes, optional + 24, 0x0000_0001_4000_0000);
        put_u32(&mut bytes, optional + 60, 0x200);
        put_u32(&mut bytes, optional + 108, 16);
        let section = optional + 0xf0;
        bytes[section..section + 6].copy_from_slice(b".rdata");
        put_u32(&mut bytes, section + 8, 0x300);
        put_u32(&mut bytes, section + 12, 0x1000);
        put_u32(&mut bytes, section + 16, 0x300);
        put_u32(&mut bytes, section + 20, 0x200);

        if let Some(name) = standard {
            put_u32(&mut bytes, optional + 120, 0x1000);
            put_u32(&mut bytes, optional + 124, 40);
            put_u32(&mut bytes, 0x200 + 12, 0x1080);
            put_c_string(&mut bytes, 0x280, name);
        }
        if let Some(name) = delayed {
            let directory = optional + 112 + 13 * 8;
            put_u32(&mut bytes, directory, 0x1040);
            put_u32(&mut bytes, directory + 4, 64);
            put_u32(&mut bytes, 0x240, 1);
            put_u32(&mut bytes, 0x244, 0x10a0);
            put_c_string(&mut bytes, 0x2a0, name);
        }
        bytes
    }

    fn put_c_string(bytes: &mut [u8], offset: usize, value: &str) {
        bytes[offset..offset + value.len()].copy_from_slice(value.as_bytes());
        bytes[offset + value.len()] = 0;
    }

    fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }
}
