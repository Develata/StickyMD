//! Bounded streaming of shared inputs into independent v1 fingerprint digests.
//! plan_ref: docs/plan/11_testing_and_release.md#shared-headless-prerequisite

use super::normalize::VersionNormalizer;
use super::{ModuleId, domains, path_domains, temporary_path};
use crate::qualification::receipt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

pub(in crate::qualification) struct Batch {
    pub(in crate::qualification) digests: Vec<String>,
    pub(in crate::qualification) input_files: usize,
    pub(in crate::qualification) input_bytes: u64,
}

pub(super) fn calculate(
    root: &Path,
    modules: &[Option<ModuleId>],
    tracked: &[String],
    extra: &[u8],
) -> Result<Batch, String> {
    let mut streams = modules
        .iter()
        .map(|module| Stream::create(temporary_path()?, *module))
        .collect::<Result<Vec<_>, _>>()?;
    let (input_files, input_bytes) = write_inputs(root, &mut streams, tracked, extra)?;
    // No digest leaves this call until every stream and hash has succeeded.
    let digests = streams
        .iter_mut()
        .map(Stream::finish)
        .collect::<Result<_, _>>()?;
    Ok(Batch {
        digests,
        input_files,
        input_bytes,
    })
}

struct Stream {
    path: PathBuf,
    module: Option<ModuleId>,
    writer: Option<BufWriter<File>>,
}

impl Stream {
    fn create(path: PathBuf, module: Option<ModuleId>) -> Result<Self, String> {
        // Establish ownership only after exclusive creation; never remove a conflicting file.
        let file = File::create_new(&path)
            .map_err(|e| format!("cannot create module fingerprint stream: {e}"))?;
        let mut stream = Self {
            path,
            module,
            writer: Some(BufWriter::with_capacity(64 * 1024, file)),
        };
        let writer = stream.writer();
        // v2 module digests apply release-version normalization; the workspace-tests
        // identity keeps the v1 raw-byte serialization.
        let header: &[u8] = if module.is_some() {
            b"StickyMD qualification module fingerprint v2\0"
        } else {
            b"StickyMD qualification module fingerprint v1\0"
        };
        writer.write_all(header).map_err(io_error)?;
        writer
            .write_all(
                module
                    .map_or("workspace-tests", ModuleId::as_str)
                    .as_bytes(),
            )
            .map_err(io_error)?;
        writer.write_all(&[0]).map_err(io_error)?;
        Ok(stream)
    }

    fn includes(&self, input_domains: u64) -> bool {
        self.module
            .is_none_or(|module| domains(module) & input_domains != 0)
    }

    fn writer(&mut self) -> &mut BufWriter<File> {
        self.writer.as_mut().expect("unfinished fingerprint stream")
    }

    fn finish(&mut self) -> Result<String, String> {
        let mut writer = self.writer.take().expect("unfinished fingerprint stream");
        // A private scratch stream needs completed writes, not durable publication.
        writer.flush().map_err(io_error)?;
        drop(writer);
        receipt::sha256(&self.path)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // Close the handle before cleanup on Windows, including unwinds and I/O failures.
        drop(self.writer.take());
        let _ = fs::remove_file(&self.path);
    }
}

fn write_inputs(
    root: &Path,
    streams: &mut [Stream],
    tracked: &[String],
    extra: &[u8],
) -> Result<(usize, u64), String> {
    let mut input_files = 0;
    let mut input_bytes = 0;
    let normalizer = streams
        .iter()
        .any(|stream| stream.module.is_some())
        .then(|| VersionNormalizer::read(root, tracked))
        .flatten();
    for relative in tracked {
        let input_domains = path_domains(relative);
        if !streams.iter().any(|s| s.includes(input_domains)) {
            continue;
        }
        let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(normalizer) = normalizer.as_ref()
            && VersionNormalizer::applies_to(relative)
        {
            // Two small manifests: read once, give module streams the normalized form.
            let raw =
                fs::read(&path).map_err(|e| format!("cannot read module input {relative}: {e}"))?;
            let normalized = normalizer.normalize(relative, &raw);
            for stream in streams.iter_mut().filter(|s| s.includes(input_domains)) {
                let content: &[u8] = if stream.module.is_some() {
                    &normalized
                } else {
                    &raw
                };
                write_entry(stream.writer(), relative, content).map_err(io_error)?;
            }
            input_files += 1;
            input_bytes += raw.len() as u64;
            continue;
        }
        let mut input =
            File::open(&path).map_err(|e| format!("cannot open module input {relative}: {e}"))?;
        let length = input
            .metadata()
            .map_err(|e| format!("cannot stat module input {relative}: {e}"))?
            .len();
        let mut output = Fanout {
            streams,
            input_domains,
        };
        let name = relative.as_bytes();
        output
            .write_all(&(name.len() as u64).to_le_bytes())
            .and_then(|()| output.write_all(name))
            .and_then(|()| output.write_all(&length.to_le_bytes()))
            .map_err(io_error)?;
        let copied = io::copy(&mut input, &mut output)
            .map_err(|e| format!("cannot hash module input {relative}: {e}"))?;
        if copied != length {
            return Err(format!(
                "module input {relative} changed length while reading"
            ));
        }
        input_files += 1;
        input_bytes += copied;
    }
    for stream in streams.iter_mut().filter(|s| s.module.is_none()) {
        stream
            .writer()
            .write_all(&(extra.len() as u64).to_le_bytes())
            .and_then(|()| stream.writer().write_all(extra))
            .map_err(io_error)?;
    }
    Ok((input_files, input_bytes))
}

/// The per-file record shared with the streaming path: name length, name, length, bytes.
fn write_entry(writer: &mut impl Write, relative: &str, content: &[u8]) -> io::Result<()> {
    let name = relative.as_bytes();
    writer.write_all(&(name.len() as u64).to_le_bytes())?;
    writer.write_all(name)?;
    writer.write_all(&(content.len() as u64).to_le_bytes())?;
    writer.write_all(content)
}

struct Fanout<'a> {
    streams: &'a mut [Stream],
    input_domains: u64,
}

impl Write for Fanout<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        for stream in self.streams.iter_mut() {
            if stream.includes(self.input_domains) {
                stream.writer().write_all(bytes)?;
            }
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        for stream in self.streams.iter_mut() {
            stream.writer().flush()?;
        }
        Ok(())
    }
}

fn io_error(error: io::Error) -> String {
    format!("module fingerprint stream I/O failed: {error}")
}

#[cfg(test)]
mod tests;
